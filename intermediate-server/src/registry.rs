//! Client registry for routing between Agents and Connectors
//!
//! The registry maintains mappings between:
//! - Service IDs and their Connectors
//! - Agents and their target services (supports multiple services per Agent)
//!
//! This enables bidirectional routing of DATAGRAMs between
//! Agent-Connector pairs for the same service.

use std::collections::{HashMap, HashSet};

use crate::client::ClientType;

// ============================================================================
// Registry Structure
// ============================================================================

/// Registry for managing client routing
pub struct Registry {
    /// Map from service_id to Connector connection ID
    connectors: HashMap<String, quiche::ConnectionId<'static>>,

    /// Map from Agent connection ID to set of target service_ids
    agent_targets: HashMap<quiche::ConnectionId<'static>, HashSet<String>>,

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
    /// For Agents: service_id is added to their set of target services
    pub fn register(
        &mut self,
        conn_id: quiche::ConnectionId<'static>,
        client_type: ClientType,
        service_id: String,
    ) {
        match client_type {
            ClientType::Connector => {
                // Clean up stale connector_services entry if another Connector
                // was previously registered for this service
                if let Some(old_conn_id) = self.connectors.get(&service_id) {
                    if old_conn_id != &conn_id {
                        log::warn!(
                            "Connector replacement: service '{}' was {:?}, now {:?}",
                            service_id,
                            old_conn_id,
                            conn_id
                        );
                        self.connector_services.remove(old_conn_id);
                    }
                }
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
                self.agent_targets
                    .entry(conn_id)
                    .or_default()
                    .insert(service_id);
            }
        }
    }

    /// Unregister a client when their connection closes
    pub fn unregister(&mut self, conn_id: &quiche::ConnectionId<'static>) {
        // Check if it was an Agent
        if let Some(services) = self.agent_targets.remove(conn_id) {
            log::info!(
                "Unregistered Agent for services {:?} (conn={:?})",
                services,
                conn_id
            );
        }

        // Check if it was a Connector
        if let Some(service_id) = self.connector_services.remove(conn_id) {
            // Only remove from connectors if this connection is still the active one.
            // A newer Connector may have already taken over the service.
            if self
                .connectors
                .get(&service_id)
                .map(|c| c == conn_id)
                .unwrap_or(false)
            {
                self.connectors.remove(&service_id);
            }
            log::info!(
                "Unregistered Connector for service '{}' (conn={:?})",
                service_id,
                conn_id
            );
        }
    }

    /// Find the destination connection for a given source connection (implicit routing).
    ///
    /// If source is an Agent → returns Connector for their first registered service
    /// If source is a Connector → returns Agent targeting their service
    ///
    /// For multi-service Agents, use find_connector_for_service() with explicit service_id.
    pub fn find_destination(
        &self,
        from_conn_id: &quiche::ConnectionId<'static>,
    ) -> Option<quiche::ConnectionId<'static>> {
        // Check if sender is an Agent (use first service for backward compat)
        if let Some(services) = self.agent_targets.get(from_conn_id) {
            if let Some(target_service) = services.iter().next() {
                return self.connectors.get(target_service).cloned();
            }
        }

        // Check if sender is a Connector
        if let Some(service_id) = self.connector_services.get(from_conn_id) {
            return self.find_agent_for_service(service_id);
        }

        None
    }

    /// Find an Agent connection that targets the given service
    pub fn find_agent_for_service(
        &self,
        service_id: &str,
    ) -> Option<quiche::ConnectionId<'static>> {
        for (agent_conn_id, services) in &self.agent_targets {
            if services.contains(service_id) {
                return Some(agent_conn_id.clone());
            }
        }
        None
    }

    /// Get the number of registered Connectors
    #[cfg(test)]
    pub fn connector_count(&self) -> usize {
        self.connectors.len()
    }

    /// Get the number of registered Agents
    #[cfg(test)]
    pub fn agent_count(&self) -> usize {
        self.agent_targets.len()
    }

    /// Check if a connection is a registered Agent targeting the given service.
    /// Used for sender authorization on service-routed datagrams.
    pub fn is_agent_for_service(
        &self,
        conn_id: &quiche::ConnectionId<'static>,
        service_id: &str,
    ) -> bool {
        self.agent_targets
            .get(conn_id)
            .map(|services| services.contains(service_id))
            .unwrap_or(false)
    }

    /// Find the Connector connection ID for a given service
    pub fn find_connector_for_service(
        &self,
        service_id: &str,
    ) -> Option<quiche::ConnectionId<'static>> {
        self.connectors.get(service_id).cloned()
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

        registry.register(
            connector_id.clone(),
            ClientType::Connector,
            "web-app".to_string(),
        );
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

        registry.register(
            connector_id.clone(),
            ClientType::Connector,
            "web-app".to_string(),
        );
        registry.register(agent_id.clone(), ClientType::Agent, "database".to_string());

        assert_eq!(registry.find_destination(&agent_id), None);
    }

    #[test]
    fn test_unregister_connector() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        registry.register(
            connector_id.clone(),
            ClientType::Connector,
            "web-app".to_string(),
        );
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        registry.unregister(&connector_id);

        assert_eq!(registry.find_destination(&agent_id), None);
        assert_eq!(registry.connector_count(), 0);
    }

    #[test]
    fn test_unregister_agent() {
        let mut registry = Registry::new();

        let connector_id = make_conn_id(1);
        let agent_id = make_conn_id(2);

        registry.register(
            connector_id.clone(),
            ClientType::Connector,
            "web-app".to_string(),
        );
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        registry.unregister(&agent_id);

        assert_eq!(registry.find_destination(&connector_id), None);
        assert_eq!(registry.agent_count(), 0);
    }

    #[test]
    fn test_multi_service_agent() {
        let mut registry = Registry::new();

        let echo_connector_id = make_conn_id(1);
        let web_connector_id = make_conn_id(2);
        let agent_id = make_conn_id(3);

        // Register two Connectors for different services
        registry.register(
            echo_connector_id.clone(),
            ClientType::Connector,
            "echo-service".to_string(),
        );
        registry.register(
            web_connector_id.clone(),
            ClientType::Connector,
            "web-app".to_string(),
        );

        // Register Agent for both services
        registry.register(
            agent_id.clone(),
            ClientType::Agent,
            "echo-service".to_string(),
        );
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        // Explicit service routing
        assert_eq!(
            registry.find_connector_for_service("echo-service"),
            Some(echo_connector_id.clone())
        );
        assert_eq!(
            registry.find_connector_for_service("web-app"),
            Some(web_connector_id.clone())
        );

        // Reverse: both Connectors should find the Agent
        assert_eq!(
            registry.find_destination(&echo_connector_id),
            Some(agent_id.clone())
        );
        assert_eq!(
            registry.find_destination(&web_connector_id),
            Some(agent_id.clone())
        );

        // Agent count should be 1 (one agent, multiple services)
        assert_eq!(registry.agent_count(), 1);
    }

    #[test]
    fn test_find_agent_for_service() {
        let mut registry = Registry::new();

        let agent_id = make_conn_id(1);
        registry.register(
            agent_id.clone(),
            ClientType::Agent,
            "echo-service".to_string(),
        );
        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        assert_eq!(
            registry.find_agent_for_service("echo-service"),
            Some(agent_id.clone())
        );
        assert_eq!(
            registry.find_agent_for_service("web-app"),
            Some(agent_id.clone())
        );
        assert_eq!(registry.find_agent_for_service("nonexistent"), None);
    }

    #[test]
    fn test_is_agent_for_service() {
        let mut registry = Registry::new();

        let agent_id = make_conn_id(1);
        let non_agent_id = make_conn_id(2);

        registry.register(agent_id.clone(), ClientType::Agent, "web-app".to_string());

        // Agent is registered for web-app
        assert!(registry.is_agent_for_service(&agent_id, "web-app"));
        // Agent is NOT registered for other services
        assert!(!registry.is_agent_for_service(&agent_id, "database"));
        // Non-agent is NOT registered
        assert!(!registry.is_agent_for_service(&non_agent_id, "web-app"));
    }

    #[test]
    fn test_connector_replacement_warns() {
        let mut registry = Registry::new();

        let old_connector = make_conn_id(1);
        let new_connector = make_conn_id(2);

        registry.register(
            old_connector.clone(),
            ClientType::Connector,
            "echo-service".to_string(),
        );

        // Replacement should succeed (warning logged but not asserted here)
        registry.register(
            new_connector.clone(),
            ClientType::Connector,
            "echo-service".to_string(),
        );

        assert_eq!(
            registry.find_connector_for_service("echo-service"),
            Some(new_connector)
        );
    }

    #[test]
    fn test_connector_replacement_preserves_registration() {
        let mut registry = Registry::new();

        let old_connector = make_conn_id(1);
        let new_connector = make_conn_id(2);
        let agent_id = make_conn_id(3);

        // Old Connector registers for echo-service
        registry.register(
            old_connector.clone(),
            ClientType::Connector,
            "echo-service".to_string(),
        );
        registry.register(
            agent_id.clone(),
            ClientType::Agent,
            "echo-service".to_string(),
        );

        // Verify routing works
        assert_eq!(
            registry.find_connector_for_service("echo-service"),
            Some(old_connector.clone())
        );

        // New Connector takes over echo-service (e.g., reconnect)
        registry.register(
            new_connector.clone(),
            ClientType::Connector,
            "echo-service".to_string(),
        );

        // Verify routing points to new Connector
        assert_eq!(
            registry.find_connector_for_service("echo-service"),
            Some(new_connector.clone())
        );

        // Old Connector's connection is cleaned up — must NOT remove new registration
        registry.unregister(&old_connector);

        // New Connector should still be registered
        assert_eq!(
            registry.find_connector_for_service("echo-service"),
            Some(new_connector.clone())
        );
        assert_eq!(registry.connector_count(), 1);

        // Agent should still route to new Connector
        assert_eq!(
            registry.find_destination(&agent_id),
            Some(new_connector.clone())
        );
    }
}
