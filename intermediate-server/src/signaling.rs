//! P2P signaling relay for Intermediate Server
//!
//! This module handles signaling message relay between Agents and Connectors
//! for P2P hole punching coordination.
//!
//! # Protocol Flow
//!
//! ```text
//! Agent                 Intermediate               Connector
//!   │                        │                          │
//!   │─── CandidateOffer ────►│                          │
//!   │                        │──── CandidateOffer ─────►│
//!   │                        │                          │
//!   │                        │◄─── CandidateAnswer ─────│
//!   │◄─── CandidateAnswer ───│                          │
//!   │                        │                          │
//!   │◄─── StartPunching ─────│──── StartPunching ──────►│
//! ```
//!
//! # Session Management
//!
//! The Intermediate tracks active P2P sessions and routes messages between
//! the correct Agent-Connector pairs based on session ID.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

// ============================================================================
// Constants
// ============================================================================

/// Maximum signaling message size (64 KB)
pub const MAX_MESSAGE_SIZE: u32 = 65536;

/// Signaling timeout
pub const SIGNALING_TIMEOUT: Duration = Duration::from_secs(5);

/// Delay before starting hole punching (ms)
pub const PUNCH_START_DELAY_MS: u64 = 100;

/// Length of message header (4 bytes for length)
pub const HEADER_LEN: usize = 4;

// ============================================================================
// Candidate Types (mirrors packet_processor::p2p::candidate)
// ============================================================================

/// Type of ICE candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub enum CandidateType {
    Host = 0,
    ServerReflexive = 1,
    PeerReflexive = 2,
    Relay = 3,
}

/// An ICE candidate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub candidate_type: CandidateType,
    pub address: SocketAddr,
    pub priority: u32,
    pub foundation: String,
    pub related_address: Option<SocketAddr>,
}

// ============================================================================
// Signaling Messages (mirrors packet_processor::p2p::signaling)
// ============================================================================

/// Signaling message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalingMessage {
    /// Agent sends its candidates to Connector
    CandidateOffer {
        session_id: u64,
        service_id: String,
        candidates: Vec<Candidate>,
    },

    /// Connector responds with its candidates
    CandidateAnswer {
        session_id: u64,
        candidates: Vec<Candidate>,
    },

    /// Signal to start hole punching
    StartPunching {
        session_id: u64,
        start_delay_ms: u64,
        peer_candidates: Vec<Candidate>,
    },

    /// Report hole punching result
    PunchingResult {
        session_id: u64,
        success: bool,
        working_address: Option<SocketAddr>,
    },

    /// Error response
    Error {
        session_id: Option<u64>,
        code: SignalingError,
        message: String,
    },
}

/// Signaling error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SignalingError {
    Unknown = 0,
    ServiceNotFound = 1,
    NoConnectorAvailable = 2,
    SessionNotFound = 3,
    SessionTimeout = 4,
    InvalidMessage = 5,
    PeerRejected = 6,
}

impl SignalingMessage {
    /// Create an error message
    #[cfg(test)]
    pub fn error(
        session_id: Option<u64>,
        code: SignalingError,
        message: impl Into<String>,
    ) -> Self {
        SignalingMessage::Error {
            session_id,
            code,
            message: message.into(),
        }
    }
}

// ============================================================================
// Message Encoding/Decoding
// ============================================================================

/// Encode a signaling message with 4-byte length prefix
pub fn encode_message(msg: &SignalingMessage) -> Result<Vec<u8>, String> {
    let payload = bincode::serialize(msg).map_err(|e| e.to_string())?;

    if payload.len() > MAX_MESSAGE_SIZE as usize {
        return Err(format!("message too large: {} bytes", payload.len()));
    }

    let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&payload);

    Ok(buf)
}

/// Decode a signaling message from length-prefixed buffer
/// Returns (message, bytes_consumed) or error
pub fn decode_message(buf: &[u8]) -> Result<(SignalingMessage, usize), DecodeError> {
    if buf.len() < HEADER_LEN {
        return Err(DecodeError::Incomplete(HEADER_LEN - buf.len()));
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

    if length > MAX_MESSAGE_SIZE as usize {
        return Err(DecodeError::TooLarge(length));
    }

    let total = HEADER_LEN + length;
    if buf.len() < total {
        return Err(DecodeError::Incomplete(total - buf.len()));
    }

    let msg = bincode::deserialize(&buf[HEADER_LEN..total])
        .map_err(|e| DecodeError::Invalid(e.to_string()))?;

    Ok((msg, total))
}

/// Decode error types
#[derive(Debug)]
pub enum DecodeError {
    /// Need more bytes
    #[allow(dead_code)]
    Incomplete(usize),
    /// Message too large
    TooLarge(usize),
    /// Invalid format
    Invalid(String),
}

// ============================================================================
// Session State
// ============================================================================

/// State of a P2P signaling session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Waiting for Connector's answer
    AwaitingAnswer,
    /// Candidates exchanged, ready to start punching
    ReadyToPunch,
    /// Hole punching in progress
    Punching,
}

/// A P2P signaling session tracked by the Intermediate
#[derive(Debug)]
pub struct SignalingSession {
    /// Service ID being connected to
    pub service_id: String,
    /// Agent's QUIC connection ID
    pub agent_conn_id: quiche::ConnectionId<'static>,
    /// Connector's QUIC connection ID (set when answer received)
    pub connector_conn_id: Option<quiche::ConnectionId<'static>>,
    /// Agent's candidates
    pub agent_candidates: Vec<Candidate>,
    /// Connector's candidates (set when answer received)
    pub connector_candidates: Option<Vec<Candidate>>,
    /// Current session state
    pub state: SessionState,
    /// When session was created
    pub created_at: Instant,
    /// Stream ID used for signaling (Connector side)
    pub connector_stream_id: Option<u64>,
}

impl SignalingSession {
    /// Create a new session from a CandidateOffer
    pub fn new(
        service_id: String,
        agent_conn_id: quiche::ConnectionId<'static>,
        agent_candidates: Vec<Candidate>,
    ) -> Self {
        Self {
            service_id,
            agent_conn_id,
            connector_conn_id: None,
            agent_candidates,
            connector_candidates: None,
            state: SessionState::AwaitingAnswer,
            created_at: Instant::now(),
            connector_stream_id: None,
        }
    }

    /// Check if session has timed out
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > SIGNALING_TIMEOUT
    }

    /// Set connector response
    pub fn set_connector_answer(
        &mut self,
        connector_conn_id: quiche::ConnectionId<'static>,
        candidates: Vec<Candidate>,
        stream_id: u64,
    ) {
        self.connector_conn_id = Some(connector_conn_id);
        self.connector_candidates = Some(candidates);
        self.connector_stream_id = Some(stream_id);
        self.state = SessionState::ReadyToPunch;
    }
}

// ============================================================================
// Session Manager
// ============================================================================

/// Manages active P2P signaling sessions
pub struct SessionManager {
    /// Active sessions by session ID
    sessions: HashMap<u64, SignalingSession>,
    /// Map from service_id to pending session (waiting for connector)
    pending_by_service: HashMap<String, u64>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            pending_by_service: HashMap::new(),
        }
    }

    /// Create a new session from a CandidateOffer
    pub fn create_session(
        &mut self,
        session_id: u64,
        service_id: String,
        agent_conn_id: quiche::ConnectionId<'static>,
        candidates: Vec<Candidate>,
    ) {
        let session = SignalingSession::new(service_id.clone(), agent_conn_id, candidates);
        self.sessions.insert(session_id, session);
        self.pending_by_service.insert(service_id, session_id);
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: u64) -> Option<&SignalingSession> {
        self.sessions.get(&session_id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, session_id: u64) -> Option<&mut SignalingSession> {
        self.sessions.get_mut(&session_id)
    }

    /// Get pending session for a service
    #[cfg(test)]
    pub fn get_pending_for_service(&self, service_id: &str) -> Option<u64> {
        self.pending_by_service.get(service_id).copied()
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: u64) -> Option<SignalingSession> {
        if let Some(session) = self.sessions.remove(&session_id) {
            self.pending_by_service.remove(&session.service_id);
            Some(session)
        } else {
            None
        }
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&mut self) -> Vec<u64> {
        let expired: Vec<u64> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_expired())
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            self.remove_session(*id);
        }

        expired
    }

    /// Get number of active sessions
    #[cfg(test)]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Iterate over all sessions
    pub fn sessions_iter(&self) -> impl Iterator<Item = (&u64, &SignalingSession)> {
        self.sessions.iter()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_candidate() -> Candidate {
        Candidate {
            candidate_type: CandidateType::Host,
            address: "192.168.1.100:50000".parse().unwrap(),
            priority: 2130706431,
            foundation: "host_192.168.1.100".to_string(),
            related_address: None,
        }
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 12345,
            service_id: "test-service".to_string(),
            candidates: vec![sample_candidate()],
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_decode_incomplete() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 1,
            service_id: "test".to_string(),
            candidates: vec![],
        };

        let encoded = encode_message(&msg).unwrap();

        // Partial buffer
        match decode_message(&encoded[..3]) {
            Err(DecodeError::Incomplete(_)) => {}
            other => panic!("Expected Incomplete, got {:?}", other),
        }
    }

    #[test]
    fn test_session_manager_create_and_get() {
        let mut manager = SessionManager::new();
        let conn_id = quiche::ConnectionId::from_ref(&[1, 2, 3, 4]);

        manager.create_session(
            100,
            "my-service".to_string(),
            conn_id.into_owned(),
            vec![sample_candidate()],
        );

        assert!(manager.get_session(100).is_some());
        assert_eq!(manager.session_count(), 1);
        assert_eq!(manager.get_pending_for_service("my-service"), Some(100));
    }

    #[test]
    fn test_session_manager_remove() {
        let mut manager = SessionManager::new();
        let conn_id = quiche::ConnectionId::from_ref(&[1, 2, 3, 4]);

        manager.create_session(100, "my-service".to_string(), conn_id.into_owned(), vec![]);

        let removed = manager.remove_session(100);
        assert!(removed.is_some());
        assert!(manager.get_session(100).is_none());
        assert!(manager.get_pending_for_service("my-service").is_none());
    }

    #[test]
    fn test_session_state_transitions() {
        let conn_id = quiche::ConnectionId::from_ref(&[1, 2, 3, 4]);
        let mut session = SignalingSession::new("test".to_string(), conn_id.into_owned(), vec![]);

        assert_eq!(session.state, SessionState::AwaitingAnswer);

        let connector_id = quiche::ConnectionId::from_ref(&[5, 6, 7, 8]);
        session.set_connector_answer(connector_id.into_owned(), vec![sample_candidate()], 2);

        assert_eq!(session.state, SessionState::ReadyToPunch);
        assert!(session.connector_candidates.is_some());
    }

    #[test]
    fn test_signaling_error_message() {
        let msg = SignalingMessage::error(
            Some(42),
            SignalingError::ServiceNotFound,
            "Service 'foo' not found",
        );

        match msg {
            SignalingMessage::Error {
                session_id,
                code,
                message,
            } => {
                assert_eq!(session_id, Some(42));
                assert_eq!(code, SignalingError::ServiceNotFound);
                assert_eq!(message, "Service 'foo' not found");
            }
            _ => panic!("Expected Error variant"),
        }
    }
}
