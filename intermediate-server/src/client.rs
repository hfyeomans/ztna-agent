//! Client management for the ZTNA Intermediate Server

use std::collections::HashMap;
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
    /// Buffer for accumulating signaling stream data (per stream ID)
    pub signaling_buffers: HashMap<u64, Vec<u8>>,
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
            signaling_buffers: HashMap::new(),
        }
    }

    /// Get or create a signaling buffer for a stream
    pub fn get_signaling_buffer(&mut self, stream_id: u64) -> &mut Vec<u8> {
        self.signaling_buffers.entry(stream_id).or_default()
    }

    /// Remove a signaling buffer for a stream
    pub fn remove_signaling_buffer(&mut self, stream_id: u64) {
        self.signaling_buffers.remove(&stream_id);
    }
}
