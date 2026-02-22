//! P2P module for direct peer-to-peer connectivity
//!
//! This module implements ICE-style NAT traversal for establishing
//! direct connections between Agent and Connector, bypassing the
//! Intermediate server when possible.
//!
//! # Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────┐
//! │                      P2P Module Structure                      │
//! ├───────────────────────────────────────────────────────────────┤
//! │                                                                │
//! │  candidate.rs    - ICE candidate types and gathering          │
//! │  signaling.rs    - Candidate exchange via Intermediate        │
//! │  connectivity.rs - Binding request/response protocol          │
//! │  hole_punch.rs   - Hole punching coordination                 │
//! │  resilience.rs   - Keepalive and path fallback                │
//! │                                                                │
//! └───────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Current Status
//!
//! - [x] `candidate.rs` - Phase 1 (Candidate Gathering)
//! - [x] `signaling.rs` - Phase 2 (Signaling Infrastructure)
//! - [x] `connectivity.rs` - Phase 3 (Direct Path Establishment)
//! - [x] `hole_punch.rs` - Phase 4 (Hole Punching Coordination)
//! - [x] `resilience.rs` - Phase 5 (Path Resilience)

pub mod candidate;
pub mod connectivity;
pub mod hole_punch;
pub mod resilience;
pub mod signaling;

// ============================================================================
// Protocol Constants (shared across all P2P modules and app-connector)
// ============================================================================

/// Magic byte prefix for all P2P control messages (ASCII 'Z' for ZTNA).
/// Distinguishes P2P control traffic from QUIC packets unambiguously:
/// QUIC long headers have bit 7 set (0x80), short headers have bit 6 set (0x40).
/// 0x5A (0b01011010) has bit 6 set, but we check for the exact byte, not a bit pattern.
pub const ZTNA_MAGIC: u8 = 0x5A;

// Re-export commonly used types
pub use candidate::{
    calculate_priority, enumerate_local_addresses, gather_host_candidates,
    gather_reflexive_candidate, gather_relay_candidate, sort_candidates_by_priority, Candidate,
    CandidateType,
};

pub use signaling::{
    decode_message, decode_messages, encode_message, generate_session_id, SignalingError,
    SignalingMessage, SIGNALING_TIMEOUT_MS,
};

pub use connectivity::{
    calculate_pair_priority, decode_binding, encode_binding, BindingMessage, BindingRequest,
    BindingResponse, CandidatePair, CheckList, CheckState,
};

pub use hole_punch::{
    select_path, should_switch_to_direct, should_switch_to_relay, HolePunchCoordinator,
    HolePunchResult, HolePunchState, PathSelection, DEFAULT_START_DELAY_MS, HOLE_PUNCH_TIMEOUT,
    SIGNALING_TIMEOUT,
};

pub use resilience::{
    decode_keepalive, encode_keepalive_request, encode_keepalive_response, ActivePath, PathInfo,
    PathManager, PathState, PathStats, FALLBACK_COOLDOWN, KEEPALIVE_INTERVAL, KEEPALIVE_REQUEST,
    KEEPALIVE_RESPONSE, KEEPALIVE_SIZE, KEEPALIVE_TIMEOUT, MISSED_KEEPALIVES_THRESHOLD,
};
