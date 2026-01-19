//! Client registry for routing between Agents and Connectors
//!
//! The registry maintains mappings between:
//! - Service IDs and their Connectors
//! - Agents and their target services
//!
//! This enables bidirectional routing of DATAGRAMs between
//! Agent-Connector pairs for the same service.

use std::collections::HashMap;

use crate::client::ClientType;

// ============================================================================
// Registry Structure
// ============================================================================

/// Registry for managing client routing
pub struct Registry {
    /// Map from service_id to Connector connection ID
    connectors: HashMap<String, quiche::ConnectionId<'static>>,

    /// Map from Agent connection ID to target service_id
    agent_targets: HashMap<quiche::ConnectionId<'static>, String>,

    /// Reverse map: Connector connection ID to service_id (for cleanup)
    connector_services: HashMap<quiche::ConnectionId<'static>, String>,
}

impl Registry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Registry {
            connectors: HashMap::new(),
            agent_targets: HashMap::new(),
            connector_services: HashMap::new(),
        }
    }

    /// Register a client with its type and service/destination ID
    ///
    /// For Connectors: service_id is the service they provide
    /// For Agents: service_id is the service they want to reach
    pub fn register(
        &mut self,
        conn_id: quiche::ConnectionId<'static>,
        client_type: ClientType,
        service_id: String,
    ) {
        match client_type {
            ClientType::Connector => {
                log::info!(
                    "Registering Connector for service '{}' (conn={:?})",
                    service_id,
                    conn_id
                );
                self.connectors.insert(service_id.clone(), conn_id.clone());
                self.connector_services.insert(conn_id, service_id);
            }
            ClientType::Agent => {
                log::info!(
                    "Registering Agent targeting service '{}' (conn={:?})",
                    service_id,
                    conn_id
                );
                self.agent_targets.insert(conn_id, service_id);
            }
        }
    }

    /// Unregister a client when their connection closes
    pub fn unregister(&mut self, conn_id: &quiche::ConnectionId<'static>) {
        // Check if it was an Agent
        if let Some(service_id) = self.agent_targets.remove(conn_id) {
            log::info!(
                "Unregistered Agent for service '{}' (conn={:?})",
                service_id,
                conn_id
            );
        }

        // Check if it was a Connector
        if let Some(service_id) = self.connector_services.remove(conn_id) {
            self.connectors.remove(&service_id);
            log::info!(
                "Unregistered Connector for service '{}' (conn={:?})",
                service_id,
                conn_id
            );
        }
    }

    /// Find the destination connection for a given source connection
    ///
    /// If source is an Agent → returns Connector for their target service
    /// If source is a Connector → returns Agent(s) targeting their service
    ///
    /// Returns None if no matching destination found
    pub fn find_destination(
        &self,
        from_conn_id: &quiche::ConnectionId<'static>,
    ) -> Option<quiche::ConnectionId<'static>> {
        // Check if sender is an Agent
        if let Some(target_service) = self.agent_targets.get(from_conn_id) {
            // Find the Connector for this service
            return self.connectors.get(target_service).cloned();
        }

        // Check if sender is a Connector
        if let Some(service_id) = self.connector_services.get(from_conn_id) {
            // Find an Agent targeting this service
            // Note: For MVP, we only support one Agent per service
            // Future: could return all Agents for load balancing
            for (agent_conn_id, agent_target) in &self.agent_targets {
                if agent_target == service_id {
                    return Some(agent_conn_id.clone());
                }
            }
        }

        None
    }

    /// Get the number of registered Connectors
    pub fn connector_count(&self) -> usize {
        self.connectors.len()
    }

    /// Get the number of registered Agents
    pub fn agent_count(&self) -> usize {
        self.agent_targets.len()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn_id(id: u8) -> quiche::ConnectionId<'static> {
        quiche::ConnectionId::from_vec(vec![id])
    }

    #[test]
    fn test_agent_to_connector_routing() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        // Register Connector for "web-app"
        registry.register(connector_id.clone(), ClientType::Connector, "web-app".to_string());

        // Register Agent targeting "web-app"
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        // Agent → Connector
        assert_eq!(
            registry.find_destination(&agent_id),
            Some(connector_id.clone())
        );

        // Connector → Agent
        assert_eq!(
            registry.find_destination(&connector_id),
            Some(agent_id.clone())
        );
    }

    #[test]
    fn test_no_matching_service() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        // Register Connector for "web-app"
        registry.register(connector_id.clone(), ClientType::Connector, "web-app".to_string());

        // Register Agent targeting different service "database"
        registry.register(agent_id.clone(), ClientType::Agent, "database".to_string());

        // No route should be found
        assert_eq!(registry.find_destination(&agent_id), None);
    }

    #[test]
    fn test_unregister_connector() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        registry.register(connector_id.clone(), ClientType::Connector, "web-app".to_string());
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        // Unregister Connector
        registry.unregister(&connector_id);

        // Agent should no longer find destination
        assert_eq!(registry.find_destination(&agent_id), None);
        assert_eq!(registry.connector_count(), 0);
    }

    #[test]
    fn test_unregister_agent() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        registry.register(connector_id.clone(), ClientType::Connector, "web-app".to_string());
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        // Unregister Agent
        registry.unregister(&agent_id);

        // Connector should no longer find destination
        assert_eq!(registry.find_destination(&connector_id), None);
        assert_eq!(registry.agent_count(), 0);
    }
}
