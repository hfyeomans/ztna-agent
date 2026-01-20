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

pub mod candidate;
pub mod connectivity;
pub mod hole_punch;
pub mod signaling;

// Re-export commonly used types
pub use candidate::{
    Candidate,
    CandidateType,
    calculate_priority,
    enumerate_local_addresses,
    gather_host_candidates,
    gather_reflexive_candidate,
    gather_relay_candidate,
    sort_candidates_by_priority,
};

pub use signaling::{
    SignalingMessage,
    SignalingError,
    encode_message,
    decode_message,
    decode_messages,
    generate_session_id,
    SIGNALING_TIMEOUT_MS,
};

pub use connectivity::{
    BindingRequest,
    BindingResponse,
    BindingMessage,
    CandidatePair,
    CheckState,
    CheckList,
    calculate_pair_priority,
    encode_binding,
    decode_binding,
};

pub use hole_punch::{
    HolePunchCoordinator,
    HolePunchState,
    HolePunchResult,
    PathSelection,
    select_path,
    should_switch_to_direct,
    should_switch_to_relay,
    HOLE_PUNCH_TIMEOUT,
    SIGNALING_TIMEOUT,
    DEFAULT_START_DELAY_MS,
};
