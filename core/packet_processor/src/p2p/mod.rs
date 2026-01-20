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
//! │  path_select.rs  - Path selection logic                       │
//! │                                                                │
//! └───────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Current Status
//!
//! - [x] `candidate.rs` - Phase 1 (Candidate Gathering)
//! - [x] `signaling.rs` - Phase 2 (Signaling Infrastructure)
//! - [ ] `connectivity.rs` - Phase 3 (Direct Path Establishment)
//! - [ ] `hole_punch.rs` - Phase 3 (Hole Punching)
//! - [ ] `path_select.rs` - Phase 4 (Path Selection)

pub mod candidate;
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
