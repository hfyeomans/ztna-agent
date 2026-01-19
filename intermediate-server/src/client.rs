//! Client management for the ZTNA Intermediate Server

use std::net::SocketAddr;

// ============================================================================
// Client Type
// ============================================================================

/// Type of connected client
#[derive(Debug, Clone, PartialEq)]
pub enum ClientType {
    /// Agent connecting to reach a service
    Agent,
    /// Connector providing access to a service
    Connector,
}

// ============================================================================
// Client Structure
// ============================================================================

/// Represents a connected QUIC client
pub struct Client {
    /// The QUIC connection
    pub conn: quiche::Connection,
    /// Observed source address (for QAD)
    pub observed_addr: SocketAddr,
    /// Type of client (Agent or Connector)
    pub client_type: Option<ClientType>,
    /// Registered service/destination ID
    pub registered_id: Option<String>,
    /// Whether QAD has been sent to this client
    pub qad_sent: bool,
}

impl Client {
    /// Create a new client from an accepted connection
    pub fn new(conn: quiche::Connection, observed_addr: SocketAddr) -> Self {
        Client {
            conn,
            observed_addr,
            client_type: None,
            registered_id: None,
            qad_sent: false,
        }
    }
}
