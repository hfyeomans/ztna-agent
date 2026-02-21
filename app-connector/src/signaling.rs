//! P2P signaling for App Connector
//!
//! Handles signaling message exchange with the Intermediate Server
//! for P2P hole punching coordination.

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

/// Length of message header (4 bytes for length)
pub const HEADER_LEN: usize = 4;

// ============================================================================
// Candidate Types (matches Intermediate Server)
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

impl Candidate {
    /// Create a new host candidate
    pub fn host(address: SocketAddr) -> Self {
        Self {
            candidate_type: CandidateType::Host,
            address,
            priority: calculate_priority(CandidateType::Host, 0, 1),
            foundation: format!("host_{}", address.ip()),
            related_address: None,
        }
    }

    /// Create a new server-reflexive candidate
    pub fn server_reflexive(address: SocketAddr, base: SocketAddr) -> Self {
        Self {
            candidate_type: CandidateType::ServerReflexive,
            address,
            priority: calculate_priority(CandidateType::ServerReflexive, 0, 1),
            foundation: format!("srflx_{}", address.ip()),
            related_address: Some(base),
        }
    }

    /// Create a new relay candidate
    pub fn relay(address: SocketAddr, base: SocketAddr) -> Self {
        Self {
            candidate_type: CandidateType::Relay,
            address,
            priority: calculate_priority(CandidateType::Relay, 0, 1),
            foundation: format!("relay_{}", address.ip()),
            related_address: Some(base),
        }
    }
}

/// Calculate ICE priority based on RFC 5245
fn calculate_priority(ctype: CandidateType, component: u8, local_pref: u16) -> u32 {
    let type_pref: u32 = match ctype {
        CandidateType::Host => 126,
        CandidateType::ServerReflexive => 100,
        CandidateType::PeerReflexive => 110,
        CandidateType::Relay => 0,
    };

    (type_pref << 24) | ((local_pref as u32) << 8) | ((256 - component as u32) & 0xFF)
}

// ============================================================================
// Signaling Messages (matches Intermediate Server)
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
    /// Need more bytes (value = bytes still needed)
    Incomplete(#[allow(dead_code)] usize),
    /// Message too large
    TooLarge(usize),
    /// Invalid format
    Invalid(String),
}

// ============================================================================
// P2P Session State
// ============================================================================

/// State of a P2P session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2PSessionState {
    /// Received offer, preparing answer
    AwaitingAnswer,
    /// Answer sent, waiting for StartPunching
    AwaitingStart,
    /// Hole punching in progress
    Punching,
    /// Direct connection established
    Connected,
    /// All checks failed, using relay
    FallbackRelay,
}

/// A P2P signaling session from the Connector's perspective
#[derive(Debug)]
pub struct P2PSession {
    /// Session ID (used for session tracking in P2P hole punching)
    #[allow(dead_code)]
    pub session_id: u64,
    /// Agent's candidates
    pub agent_candidates: Vec<Candidate>,
    /// Our candidates
    pub local_candidates: Vec<Candidate>,
    /// Current state
    pub state: P2PSessionState,
    /// When session was created
    pub created_at: Instant,
    /// Working address (if connection succeeded)
    pub working_address: Option<SocketAddr>,
    /// Start punching time (after delay)
    pub punch_start_time: Option<Instant>,
}

impl P2PSession {
    /// Create a new session from a received CandidateOffer
    pub fn new(session_id: u64, agent_candidates: Vec<Candidate>) -> Self {
        Self {
            session_id,
            agent_candidates,
            local_candidates: Vec::new(),
            state: P2PSessionState::AwaitingAnswer,
            created_at: Instant::now(),
            working_address: None,
            punch_start_time: None,
        }
    }

    /// Set local candidates and transition to AwaitingStart
    pub fn set_local_candidates(&mut self, candidates: Vec<Candidate>) {
        self.local_candidates = candidates;
        self.state = P2PSessionState::AwaitingStart;
    }

    /// Set punch start time
    pub fn set_punch_start(&mut self, delay_ms: u64) {
        self.punch_start_time = Some(Instant::now() + Duration::from_millis(delay_ms));
        self.state = P2PSessionState::Punching;
    }

    /// Check if punching should start now (used during P2P connectivity checks)
    #[allow(dead_code)]
    pub fn should_start_punching(&self) -> bool {
        match (self.state, self.punch_start_time) {
            (P2PSessionState::Punching, Some(start)) => Instant::now() >= start,
            _ => false,
        }
    }

    /// Check if session has timed out
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > SIGNALING_TIMEOUT
    }

    /// Mark as connected
    pub fn set_connected(&mut self, addr: SocketAddr) {
        self.working_address = Some(addr);
        self.state = P2PSessionState::Connected;
    }

    /// Mark as fallback relay
    pub fn set_fallback(&mut self) {
        self.state = P2PSessionState::FallbackRelay;
    }
}

/// Manager for P2P sessions
pub struct P2PSessionManager {
    /// Active sessions by session ID
    sessions: HashMap<u64, P2PSession>,
}

impl P2PSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new session from a CandidateOffer
    pub fn create_session(&mut self, session_id: u64, agent_candidates: Vec<Candidate>) {
        let session = P2PSession::new(session_id, agent_candidates);
        self.sessions.insert(session_id, session);
    }

    /// Get a session by ID (used during P2P connectivity checks)
    #[allow(dead_code)]
    pub fn get_session(&self, session_id: u64) -> Option<&P2PSession> {
        self.sessions.get(&session_id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, session_id: u64) -> Option<&mut P2PSession> {
        self.sessions.get_mut(&session_id)
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: u64) -> Option<P2PSession> {
        self.sessions.remove(&session_id)
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
            self.sessions.remove(id);
        }

        expired
    }
}

impl Default for P2PSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Candidate Gathering Helpers
// ============================================================================

/// Gather candidates including observed (server-reflexive) address
pub fn gather_candidates_with_observed(
    bind_addr: SocketAddr,
    observed_addr: Option<SocketAddr>,
    intermediate_addr: Option<SocketAddr>,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    // Add host candidate
    candidates.push(Candidate::host(bind_addr));

    // Add server-reflexive candidate if observed address differs
    if let Some(observed) = observed_addr {
        if observed != bind_addr {
            candidates.push(Candidate::server_reflexive(observed, bind_addr));
        }
    }

    // Add relay candidate (intermediate server)
    if let Some(intermediate) = intermediate_addr {
        candidates.push(Candidate::relay(intermediate, bind_addr));
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 12345,
            service_id: "test-service".to_string(),
            candidates: vec![Candidate::host("192.168.1.100:5000".parse().unwrap())],
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_session_lifecycle() {
        let mut manager = P2PSessionManager::new();

        // Create session
        manager.create_session(100, vec![]);
        assert!(manager.get_session(100).is_some());

        // Update session
        if let Some(session) = manager.get_session_mut(100) {
            session
                .set_local_candidates(vec![Candidate::host("192.168.1.1:5000".parse().unwrap())]);
            assert_eq!(session.state, P2PSessionState::AwaitingStart);
        }

        // Remove session
        manager.remove_session(100);
        assert!(manager.get_session(100).is_none());
    }
}
