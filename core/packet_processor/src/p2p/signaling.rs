//! P2P signaling protocol for candidate exchange
//!
//! This module implements the signaling protocol used to exchange ICE candidates
//! between Agent and Connector via the Intermediate server.
//!
//! # Protocol Overview
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
//! # Message Framing
//!
//! Messages are length-prefixed with a 4-byte big-endian length header:
//! ```text
//! ┌─────────────┬─────────────────────────────────┐
//! │ Length (4B) │ Payload (bincode-encoded)       │
//! └─────────────┴─────────────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

use super::candidate::Candidate;

// ============================================================================
// Constants
// ============================================================================

/// Maximum signaling message size (64 KB should be plenty for candidates)
pub const MAX_MESSAGE_SIZE: u32 = 65536;

/// Signaling timeout in milliseconds
pub const SIGNALING_TIMEOUT_MS: u64 = 5000;

/// Length of the message header (4 bytes for length)
pub const HEADER_LEN: usize = 4;

// ============================================================================
// Signaling Messages
// ============================================================================

/// Signaling message types for P2P coordination
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalingMessage {
    /// Agent sends its candidates to Connector (via Intermediate)
    CandidateOffer {
        /// Unique session identifier for this P2P attempt
        session_id: u64,
        /// Service ID the Agent wants to connect to
        service_id: String,
        /// Agent's local candidates
        candidates: Vec<Candidate>,
    },

    /// Connector responds with its candidates
    CandidateAnswer {
        /// Session ID from the offer
        session_id: u64,
        /// Connector's local candidates
        candidates: Vec<Candidate>,
    },

    /// Intermediate signals both parties to start hole punching
    StartPunching {
        /// Session ID
        session_id: u64,
        /// Relative time: start punching in N milliseconds
        start_delay_ms: u64,
        /// Peer's candidates to try
        peer_candidates: Vec<Candidate>,
    },

    /// Report hole punching result
    PunchingResult {
        /// Session ID
        session_id: u64,
        /// Whether direct connection was established
        success: bool,
        /// Address that worked (if successful)
        working_address: Option<std::net::SocketAddr>,
    },

    /// Error response
    Error {
        /// Session ID (if known)
        session_id: Option<u64>,
        /// Error code
        code: SignalingError,
        /// Human-readable message
        message: String,
    },
}

/// Signaling error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SignalingError {
    /// Unknown or internal error
    Unknown = 0,
    /// Service not found
    ServiceNotFound = 1,
    /// No connector available for service
    NoConnectorAvailable = 2,
    /// Session not found
    SessionNotFound = 3,
    /// Session timed out
    SessionTimeout = 4,
    /// Invalid message format
    InvalidMessage = 5,
    /// Peer rejected the offer
    PeerRejected = 6,
}

impl std::fmt::Display for SignalingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalingError::Unknown => write!(f, "unknown error"),
            SignalingError::ServiceNotFound => write!(f, "service not found"),
            SignalingError::NoConnectorAvailable => write!(f, "no connector available"),
            SignalingError::SessionNotFound => write!(f, "session not found"),
            SignalingError::SessionTimeout => write!(f, "session timed out"),
            SignalingError::InvalidMessage => write!(f, "invalid message"),
            SignalingError::PeerRejected => write!(f, "peer rejected"),
        }
    }
}

// ============================================================================
// Message Encoding/Decoding
// ============================================================================

/// Encode a signaling message with length prefix
///
/// Returns a Vec containing: [4-byte BE length][bincode payload]
pub fn encode_message(msg: &SignalingMessage) -> Result<Vec<u8>, EncodeError> {
    let payload = bincode::serialize(msg).map_err(|e| EncodeError::Serialization(e.to_string()))?;

    if payload.len() > MAX_MESSAGE_SIZE as usize {
        return Err(EncodeError::MessageTooLarge(payload.len()));
    }

    let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&payload);

    Ok(buf)
}

/// Decode a signaling message from a length-prefixed buffer
///
/// Returns the message and the number of bytes consumed
pub fn decode_message(buf: &[u8]) -> Result<(SignalingMessage, usize), DecodeError> {
    if buf.len() < HEADER_LEN {
        return Err(DecodeError::Incomplete(HEADER_LEN - buf.len()));
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

    if length > MAX_MESSAGE_SIZE as usize {
        return Err(DecodeError::MessageTooLarge(length));
    }

    let total_len = HEADER_LEN + length;
    if buf.len() < total_len {
        return Err(DecodeError::Incomplete(total_len - buf.len()));
    }

    let payload = &buf[HEADER_LEN..total_len];
    let msg =
        bincode::deserialize(payload).map_err(|e| DecodeError::Deserialization(e.to_string()))?;

    Ok((msg, total_len))
}

/// Try to decode multiple messages from a buffer
///
/// Returns decoded messages and remaining bytes
pub fn decode_messages(mut buf: &[u8]) -> (Vec<SignalingMessage>, Vec<u8>) {
    let mut messages = Vec::new();

    while !buf.is_empty() {
        match decode_message(buf) {
            Ok((msg, consumed)) => {
                messages.push(msg);
                buf = &buf[consumed..];
            }
            Err(DecodeError::Incomplete(_)) => {
                // Not enough data for next message
                break;
            }
            Err(_) => {
                // Error decoding - stop processing
                break;
            }
        }
    }

    (messages, buf.to_vec())
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during message encoding
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// Serialization failed
    Serialization(String),
    /// Message exceeds maximum size
    MessageTooLarge(usize),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Serialization(e) => write!(f, "serialization error: {}", e),
            EncodeError::MessageTooLarge(size) => {
                write!(
                    f,
                    "message too large: {} bytes (max {})",
                    size, MAX_MESSAGE_SIZE
                )
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Errors that can occur during message decoding
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not enough data (need N more bytes)
    Incomplete(usize),
    /// Deserialization failed
    Deserialization(String),
    /// Message exceeds maximum size
    MessageTooLarge(usize),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Incomplete(needed) => {
                write!(f, "incomplete message, need {} more bytes", needed)
            }
            DecodeError::Deserialization(e) => write!(f, "deserialization error: {}", e),
            DecodeError::MessageTooLarge(size) => {
                write!(
                    f,
                    "message too large: {} bytes (max {})",
                    size, MAX_MESSAGE_SIZE
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

// ============================================================================
// Stream-based I/O (for QUIC streams)
// ============================================================================

/// Write a signaling message to a writer with length prefix
pub fn write_message<W: Write>(writer: &mut W, msg: &SignalingMessage) -> io::Result<()> {
    let encoded = encode_message(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    writer.write_all(&encoded)?;
    Ok(())
}

/// Read a signaling message from a reader
///
/// Blocks until a complete message is received
pub fn read_message<R: Read>(reader: &mut R) -> io::Result<SignalingMessage> {
    // Read length header
    let mut header = [0u8; HEADER_LEN];
    reader.read_exact(&mut header)?;

    let length = u32::from_be_bytes(header) as usize;
    if length > MAX_MESSAGE_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} bytes", length),
        ));
    }

    // Read payload
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;

    bincode::deserialize(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

// ============================================================================
// Session Management
// ============================================================================

/// Generate a random session ID using CSPRNG
pub fn generate_session_id() -> u64 {
    use ring::rand::{SecureRandom, SystemRandom};
    let rng = SystemRandom::new();
    let mut buf = [0u8; 8];
    rng.fill(&mut buf).expect("SystemRandom failed");
    u64::from_ne_bytes(buf)
}

// ============================================================================
// Helper Methods
// ============================================================================

impl SignalingMessage {
    /// Get the session ID from a message (if applicable)
    pub fn session_id(&self) -> Option<u64> {
        match self {
            SignalingMessage::CandidateOffer { session_id, .. } => Some(*session_id),
            SignalingMessage::CandidateAnswer { session_id, .. } => Some(*session_id),
            SignalingMessage::StartPunching { session_id, .. } => Some(*session_id),
            SignalingMessage::PunchingResult { session_id, .. } => Some(*session_id),
            SignalingMessage::Error { session_id, .. } => *session_id,
        }
    }

    /// Check if this is an error message
    pub fn is_error(&self) -> bool {
        matches!(self, SignalingMessage::Error { .. })
    }

    /// Create an error response for a given session
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::candidate::Candidate;

    fn sample_candidates() -> Vec<Candidate> {
        vec![
            Candidate::host("192.168.1.100:50000".parse().unwrap()),
            Candidate::server_reflexive(
                "203.0.113.50:50000".parse().unwrap(),
                "192.168.1.100:50000".parse().unwrap(),
            ),
        ]
    }

    #[test]
    fn test_encode_decode_candidate_offer() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 12345,
            service_id: "test-service".to_string(),
            candidates: sample_candidates(),
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_encode_decode_candidate_answer() {
        let msg = SignalingMessage::CandidateAnswer {
            session_id: 12345,
            candidates: sample_candidates(),
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_encode_decode_start_punching() {
        let msg = SignalingMessage::StartPunching {
            session_id: 12345,
            start_delay_ms: 100,
            peer_candidates: sample_candidates(),
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_encode_decode_punching_result() {
        let msg = SignalingMessage::PunchingResult {
            session_id: 12345,
            success: true,
            working_address: Some("203.0.113.50:50000".parse().unwrap()),
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_encode_decode_error() {
        let msg = SignalingMessage::Error {
            session_id: Some(12345),
            code: SignalingError::ServiceNotFound,
            message: "Service 'foo' not found".to_string(),
        };

        let encoded = encode_message(&msg).unwrap();
        let (decoded, _) = decode_message(&encoded).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_incomplete() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 1,
            service_id: "test".to_string(),
            candidates: vec![],
        };

        let encoded = encode_message(&msg).unwrap();

        // Test with partial header
        assert!(matches!(
            decode_message(&encoded[..2]),
            Err(DecodeError::Incomplete(_))
        ));

        // Test with partial payload
        assert!(matches!(
            decode_message(&encoded[..encoded.len() - 1]),
            Err(DecodeError::Incomplete(_))
        ));
    }

    #[test]
    fn test_decode_multiple_messages() {
        let msg1 = SignalingMessage::CandidateOffer {
            session_id: 1,
            service_id: "svc1".to_string(),
            candidates: vec![],
        };
        let msg2 = SignalingMessage::CandidateAnswer {
            session_id: 1,
            candidates: vec![],
        };

        let mut buf = encode_message(&msg1).unwrap();
        buf.extend(encode_message(&msg2).unwrap());

        let (messages, remaining) = decode_messages(&buf);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], msg1);
        assert_eq!(messages[1], msg2);
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_decode_multiple_with_remainder() {
        let msg = SignalingMessage::CandidateOffer {
            session_id: 1,
            service_id: "test".to_string(),
            candidates: vec![],
        };

        let mut buf = encode_message(&msg).unwrap();
        buf.extend(&[0x00, 0x00, 0x00, 0x10]); // Partial header for next message

        let (messages, remaining) = decode_messages(&buf);

        assert_eq!(messages.len(), 1);
        assert_eq!(remaining.len(), 4);
    }

    #[test]
    fn test_message_too_large() {
        // Create a header claiming huge size
        let fake_header = (MAX_MESSAGE_SIZE + 1).to_be_bytes();

        assert!(matches!(
            decode_message(&fake_header),
            Err(DecodeError::MessageTooLarge(_))
        ));
    }

    #[test]
    fn test_session_id_extraction() {
        let offer = SignalingMessage::CandidateOffer {
            session_id: 42,
            service_id: "test".to_string(),
            candidates: vec![],
        };
        assert_eq!(offer.session_id(), Some(42));

        let error = SignalingMessage::Error {
            session_id: None,
            code: SignalingError::Unknown,
            message: "test".to_string(),
        };
        assert_eq!(error.session_id(), None);
    }

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();

        // CSPRNG should produce different values
        assert_ne!(id1, id2);
        assert_ne!(id1, 0);
        assert_ne!(id2, 0);
    }

    #[test]
    fn test_session_id_uniqueness() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();
        assert_ne!(id1, id2, "Consecutive CSPRNG session IDs should differ");
    }

    #[test]
    fn test_stream_io() {
        use std::io::Cursor;

        let msg = SignalingMessage::CandidateOffer {
            session_id: 999,
            service_id: "stream-test".to_string(),
            candidates: sample_candidates(),
        };

        // Write to buffer
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        // Read back
        let mut cursor = Cursor::new(buf);
        let decoded = read_message(&mut cursor).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_signaling_error_display() {
        assert_eq!(
            format!("{}", SignalingError::ServiceNotFound),
            "service not found"
        );
        assert_eq!(
            format!("{}", SignalingError::SessionTimeout),
            "session timed out"
        );
    }
}
