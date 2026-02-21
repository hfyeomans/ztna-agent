//! Connectivity checks for P2P hole punching
//!
//! Implements ICE-style connectivity checks using binding requests/responses
//! to verify reachability between candidate pairs.
//!
//! # Protocol Overview
//!
//! ```text
//! Agent                                              Connector
//!   │                                                    │
//!   │─── BindingRequest (txn_id, priority) ────────────►│
//!   │                                                    │
//!   │◄─── BindingResponse (txn_id, success, mapped) ────│
//!   │                                                    │
//!   │   (if success, direct path is viable)              │
//! ```
//!
//! # Candidate Pair Priority (RFC 8445 Section 6.1.2.3)
//!
//! ```text
//! pair_priority = 2^32 * MIN(G,D) + 2 * MAX(G,D) + (G > D ? 1 : 0)
//! ```
//! where G = controlling agent priority, D = controlled agent priority

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::candidate::Candidate;

// ============================================================================
// Constants
// ============================================================================

/// Size of transaction ID in bytes
pub const TRANSACTION_ID_LEN: usize = 12;

/// Initial retransmit interval
pub const INITIAL_RTO: Duration = Duration::from_millis(100);

/// Maximum retransmit interval
pub const MAX_RTO: Duration = Duration::from_millis(1600);

/// Maximum number of retransmissions per check
pub const MAX_RETRANSMITS: u32 = 5;

/// Total timeout for all connectivity checks
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Interval between sending binding requests to different pairs
pub const PACE_INTERVAL: Duration = Duration::from_millis(20);

// ============================================================================
// Binding Messages
// ============================================================================

/// Binding request sent to verify connectivity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingRequest {
    /// Unique transaction identifier
    pub transaction_id: [u8; TRANSACTION_ID_LEN],
    /// Priority of the candidate pair
    pub priority: u64,
    /// Whether sender is the controlling agent
    pub use_candidate: bool,
}

/// Binding response confirming connectivity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingResponse {
    /// Transaction ID from the request
    pub transaction_id: [u8; TRANSACTION_ID_LEN],
    /// Whether the check succeeded
    pub success: bool,
    /// Mapped address (reflexive discovery)
    pub mapped_address: Option<SocketAddr>,
}

/// Binding message (request or response)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingMessage {
    Request(BindingRequest),
    Response(BindingResponse),
}

impl BindingRequest {
    /// Create a new binding request with random transaction ID
    pub fn new(priority: u64, use_candidate: bool) -> Self {
        Self {
            transaction_id: generate_transaction_id(),
            priority,
            use_candidate,
        }
    }

    /// Create a binding request with specific transaction ID
    pub fn with_transaction_id(
        transaction_id: [u8; TRANSACTION_ID_LEN],
        priority: u64,
        use_candidate: bool,
    ) -> Self {
        Self {
            transaction_id,
            priority,
            use_candidate,
        }
    }
}

impl BindingResponse {
    /// Create a success response
    pub fn success(transaction_id: [u8; TRANSACTION_ID_LEN], mapped_address: SocketAddr) -> Self {
        Self {
            transaction_id,
            success: true,
            mapped_address: Some(mapped_address),
        }
    }

    /// Create a failure response
    pub fn failure(transaction_id: [u8; TRANSACTION_ID_LEN]) -> Self {
        Self {
            transaction_id,
            success: false,
            mapped_address: None,
        }
    }
}

/// Generate a random transaction ID
fn generate_transaction_id() -> [u8; TRANSACTION_ID_LEN] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut id = [0u8; TRANSACTION_ID_LEN];

    // Use timestamp + counter for uniqueness
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    id[0..8].copy_from_slice(&nanos.to_le_bytes());

    // Add process ID for additional uniqueness
    let pid = std::process::id();
    id[8..12].copy_from_slice(&pid.to_le_bytes());

    id
}

// ============================================================================
// Candidate Pairs
// ============================================================================

/// A pair of local and remote candidates for connectivity checking
#[derive(Debug, Clone)]
pub struct CandidatePair {
    /// Local candidate
    pub local: Candidate,
    /// Remote candidate
    pub remote: Candidate,
    /// Pair priority (higher = try first)
    pub priority: u64,
    /// Foundation string (for frozen/unfrozen logic)
    pub foundation: String,
    /// Current state of the check
    pub state: CheckState,
    /// Number of times request has been sent
    pub transmit_count: u32,
    /// When the last request was sent
    pub last_sent: Option<Instant>,
    /// Transaction ID of outstanding request
    pub transaction_id: Option<[u8; TRANSACTION_ID_LEN]>,
    /// Whether this is the nominated pair
    pub nominated: bool,
}

/// State of a connectivity check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// Waiting to be scheduled
    Frozen,
    /// Ready to send binding request
    Waiting,
    /// Request sent, awaiting response
    InProgress,
    /// Check succeeded
    Succeeded,
    /// Check failed
    Failed,
}

impl CandidatePair {
    /// Create a new candidate pair
    ///
    /// # Arguments
    /// * `local` - Local candidate
    /// * `remote` - Remote candidate
    /// * `is_controlling` - Whether this agent is the controlling agent
    pub fn new(local: Candidate, remote: Candidate, is_controlling: bool) -> Self {
        let priority = calculate_pair_priority(local.priority, remote.priority, is_controlling);
        let foundation = format!("{}:{}", local.foundation, remote.foundation);

        Self {
            local,
            remote,
            priority,
            foundation,
            state: CheckState::Frozen,
            transmit_count: 0,
            last_sent: None,
            transaction_id: None,
            nominated: false,
        }
    }

    /// Check if this pair needs retransmission
    pub fn needs_retransmit(&self) -> bool {
        if self.state != CheckState::InProgress {
            return false;
        }

        if self.transmit_count >= MAX_RETRANSMITS {
            return false;
        }

        match self.last_sent {
            Some(sent) => {
                let rto = self.current_rto();
                sent.elapsed() >= rto
            }
            None => true,
        }
    }

    /// Get current retransmit timeout (exponential backoff)
    pub fn current_rto(&self) -> Duration {
        let multiplier = 1u32 << self.transmit_count.min(4);
        let rto = INITIAL_RTO * multiplier;
        rto.min(MAX_RTO)
    }

    /// Check if this pair has timed out
    pub fn is_timed_out(&self, start_time: Instant) -> bool {
        start_time.elapsed() >= CHECK_TIMEOUT && self.state == CheckState::InProgress
    }

    /// Mark as in progress with new transaction ID
    pub fn start_check(&mut self) -> BindingRequest {
        let request = BindingRequest::new(self.priority, self.nominated);
        self.transaction_id = Some(request.transaction_id);
        self.state = CheckState::InProgress;
        self.transmit_count = 1;
        self.last_sent = Some(Instant::now());
        request
    }

    /// Record a retransmission
    pub fn record_retransmit(&mut self) {
        self.transmit_count += 1;
        self.last_sent = Some(Instant::now());
    }

    /// Handle a binding response
    pub fn handle_response(&mut self, response: &BindingResponse) -> bool {
        // Check transaction ID matches
        if self.transaction_id != Some(response.transaction_id) {
            return false;
        }

        if response.success {
            self.state = CheckState::Succeeded;
        } else {
            self.state = CheckState::Failed;
        }

        true
    }

    /// Mark check as failed
    pub fn mark_failed(&mut self) {
        self.state = CheckState::Failed;
    }
}

/// Calculate pair priority per RFC 8445 Section 6.1.2.3
///
/// Formula: 2^32 * MIN(G,D) + 2 * MAX(G,D) + (G > D ? 1 : 0)
/// where G = controlling priority, D = controlled priority
pub fn calculate_pair_priority(
    local_priority: u32,
    remote_priority: u32,
    is_controlling: bool,
) -> u64 {
    let (g, d) = if is_controlling {
        (local_priority as u64, remote_priority as u64)
    } else {
        (remote_priority as u64, local_priority as u64)
    };

    let min = g.min(d);
    let max = g.max(d);
    let tie_breaker = if g > d { 1u64 } else { 0u64 };

    (1u64 << 32) * min + 2 * max + tie_breaker
}

// ============================================================================
// Check List
// ============================================================================

/// Manages all candidate pairs and their connectivity checks
#[derive(Debug)]
pub struct CheckList {
    /// All candidate pairs, sorted by priority
    pairs: Vec<CandidatePair>,
    /// When checking started
    start_time: Option<Instant>,
    /// Whether we are the controlling agent
    is_controlling: bool,
    /// Index of next pair to check
    next_check_index: usize,
    /// When last check was triggered
    last_check_time: Option<Instant>,
}

impl CheckList {
    /// Create a new check list
    pub fn new(is_controlling: bool) -> Self {
        Self {
            pairs: Vec::new(),
            start_time: None,
            is_controlling,
            next_check_index: 0,
            last_check_time: None,
        }
    }

    /// Add candidate pairs from local and remote candidates
    pub fn add_pairs(&mut self, local_candidates: &[Candidate], remote_candidates: &[Candidate]) {
        for local in local_candidates {
            for remote in remote_candidates {
                // Only pair candidates of same IP family
                if local.address.is_ipv4() != remote.address.is_ipv4() {
                    continue;
                }

                let pair = CandidatePair::new(local.clone(), remote.clone(), self.is_controlling);
                self.pairs.push(pair);
            }
        }

        // Sort by priority (highest first)
        self.pairs.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Unfreeze first pair of each foundation
        self.unfreeze_initial_pairs();
    }

    /// Unfreeze the first pair with each unique foundation
    fn unfreeze_initial_pairs(&mut self) {
        let mut seen_foundations = std::collections::HashSet::new();

        for pair in &mut self.pairs {
            if !seen_foundations.contains(&pair.foundation) {
                pair.state = CheckState::Waiting;
                seen_foundations.insert(pair.foundation.clone());
            }
        }
    }

    /// Start the checking process
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.last_check_time = None;
        self.next_check_index = 0;
    }

    /// Get the next binding request to send (if any)
    ///
    /// Returns the pair index and binding request
    pub fn next_request(&mut self) -> Option<(usize, BindingRequest, SocketAddr)> {
        // Check pacing
        if let Some(last) = self.last_check_time {
            if last.elapsed() < PACE_INTERVAL {
                return None;
            }
        }

        // First check for retransmissions
        for (idx, pair) in self.pairs.iter_mut().enumerate() {
            if pair.needs_retransmit() {
                pair.record_retransmit();
                if let Some(txn_id) = pair.transaction_id {
                    let request =
                        BindingRequest::with_transaction_id(txn_id, pair.priority, pair.nominated);
                    self.last_check_time = Some(Instant::now());
                    return Some((idx, request, pair.remote.address));
                }
            }
        }

        // Then check for new pairs to test
        while self.next_check_index < self.pairs.len() {
            let idx = self.next_check_index;
            self.next_check_index += 1;

            if self.pairs[idx].state == CheckState::Waiting {
                let request = self.pairs[idx].start_check();
                let remote_addr = self.pairs[idx].remote.address;
                self.last_check_time = Some(Instant::now());
                return Some((idx, request, remote_addr));
            }
        }

        None
    }

    /// Handle a binding response
    ///
    /// Returns the index of the pair that matched (if any)
    pub fn handle_response(&mut self, response: &BindingResponse) -> Option<usize> {
        // First find the matching pair
        let mut matched_idx = None;
        let mut foundation_to_unfreeze = None;

        for (idx, pair) in self.pairs.iter_mut().enumerate() {
            if pair.handle_response(response) {
                matched_idx = Some(idx);
                if pair.state == CheckState::Succeeded {
                    foundation_to_unfreeze = Some(pair.foundation.clone());
                }
                break;
            }
        }

        // Then unfreeze if needed (separate borrow)
        if let Some(foundation) = foundation_to_unfreeze {
            self.unfreeze_by_foundation(&foundation);
        }

        matched_idx
    }

    /// Unfreeze all frozen pairs with the given foundation
    fn unfreeze_by_foundation(&mut self, foundation: &str) {
        for pair in &mut self.pairs {
            if pair.state == CheckState::Frozen && pair.foundation == foundation {
                pair.state = CheckState::Waiting;
            }
        }
    }

    /// Handle timeout for all in-progress pairs
    pub fn handle_timeouts(&mut self) {
        let start = match self.start_time {
            Some(s) => s,
            None => return,
        };

        for pair in &mut self.pairs {
            if pair.state == CheckState::InProgress
                && (pair.transmit_count >= MAX_RETRANSMITS || pair.is_timed_out(start))
            {
                pair.mark_failed();
            }
        }
    }

    /// Get the best succeeded pair (if any)
    pub fn get_best_succeeded(&self) -> Option<&CandidatePair> {
        self.pairs
            .iter()
            .filter(|p| p.state == CheckState::Succeeded)
            .max_by_key(|p| p.priority)
    }

    /// Check if all checks are complete
    pub fn is_complete(&self) -> bool {
        self.pairs
            .iter()
            .all(|p| matches!(p.state, CheckState::Succeeded | CheckState::Failed))
    }

    /// Check if any check succeeded
    pub fn has_succeeded(&self) -> bool {
        self.pairs.iter().any(|p| p.state == CheckState::Succeeded)
    }

    /// Check if checking has timed out overall
    pub fn is_timed_out(&self) -> bool {
        match self.start_time {
            Some(start) => start.elapsed() >= CHECK_TIMEOUT,
            None => false,
        }
    }

    /// Get number of pairs
    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    /// Get pairs by state
    pub fn pairs_by_state(&self, state: CheckState) -> impl Iterator<Item = &CandidatePair> {
        self.pairs.iter().filter(move |p| p.state == state)
    }

    /// Get a pair by index
    pub fn get_pair(&self, index: usize) -> Option<&CandidatePair> {
        self.pairs.get(index)
    }

    /// Nominate a successful pair
    pub fn nominate(&mut self, index: usize) -> bool {
        if let Some(pair) = self.pairs.get_mut(index) {
            if pair.state == CheckState::Succeeded {
                pair.nominated = true;
                return true;
            }
        }
        false
    }
}

// ============================================================================
// Message Encoding
// ============================================================================

/// Encode a binding message
pub fn encode_binding(msg: &BindingMessage) -> Result<Vec<u8>, String> {
    bincode::serialize(msg).map_err(|e| e.to_string())
}

/// Decode a binding message
pub fn decode_binding(data: &[u8]) -> Result<BindingMessage, String> {
    bincode::deserialize(data).map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::candidate::CandidateType;

    fn host_candidate(addr: &str) -> Candidate {
        Candidate::host(addr.parse().unwrap())
    }

    fn srflx_candidate(public: &str, base: &str) -> Candidate {
        Candidate::server_reflexive(public.parse().unwrap(), base.parse().unwrap())
    }

    #[test]
    fn test_binding_request_creation() {
        let req = BindingRequest::new(1000, false);
        assert_eq!(req.priority, 1000);
        assert!(!req.use_candidate);
        assert_ne!(req.transaction_id, [0u8; 12]);
    }

    #[test]
    fn test_binding_response_success() {
        let txn_id = [1u8; 12];
        let addr: SocketAddr = "192.168.1.1:5000".parse().unwrap();
        let resp = BindingResponse::success(txn_id, addr);

        assert!(resp.success);
        assert_eq!(resp.transaction_id, txn_id);
        assert_eq!(resp.mapped_address, Some(addr));
    }

    #[test]
    fn test_binding_response_failure() {
        let txn_id = [2u8; 12];
        let resp = BindingResponse::failure(txn_id);

        assert!(!resp.success);
        assert_eq!(resp.transaction_id, txn_id);
        assert!(resp.mapped_address.is_none());
    }

    #[test]
    fn test_binding_message_roundtrip() {
        let req = BindingRequest::new(5000, true);
        let msg = BindingMessage::Request(req.clone());

        let encoded = encode_binding(&msg).unwrap();
        let decoded = decode_binding(&encoded).unwrap();

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_pair_priority_controlling() {
        // Controlling agent: local is G, remote is D
        let priority = calculate_pair_priority(100, 50, true);

        // 2^32 * min(100,50) + 2 * max(100,50) + (100 > 50 ? 1 : 0)
        // = 2^32 * 50 + 2 * 100 + 1
        let expected = (1u64 << 32) * 50 + 2 * 100 + 1;
        assert_eq!(priority, expected);
    }

    #[test]
    fn test_pair_priority_controlled() {
        // Controlled agent: remote is G, local is D
        let priority = calculate_pair_priority(50, 100, false);

        // G=100, D=50
        // 2^32 * min(100,50) + 2 * max(100,50) + (100 > 50 ? 1 : 0)
        let expected = (1u64 << 32) * 50 + 2 * 100 + 1;
        assert_eq!(priority, expected);
    }

    #[test]
    fn test_candidate_pair_creation() {
        let local = host_candidate("192.168.1.100:5000");
        let remote = host_candidate("192.168.1.200:5000");

        let pair = CandidatePair::new(local.clone(), remote.clone(), true);

        assert_eq!(pair.local, local);
        assert_eq!(pair.remote, remote);
        assert_eq!(pair.state, CheckState::Frozen);
        assert!(!pair.nominated);
    }

    #[test]
    fn test_candidate_pair_check_flow() {
        let local = host_candidate("192.168.1.100:5000");
        let remote = host_candidate("192.168.1.200:5000");
        let mut pair = CandidatePair::new(local, remote, true);

        // Start check
        pair.state = CheckState::Waiting;
        let request = pair.start_check();

        assert_eq!(pair.state, CheckState::InProgress);
        assert_eq!(pair.transmit_count, 1);
        assert!(pair.transaction_id.is_some());

        // Handle success response
        let response =
            BindingResponse::success(request.transaction_id, "203.0.113.50:5000".parse().unwrap());
        assert!(pair.handle_response(&response));
        assert_eq!(pair.state, CheckState::Succeeded);
    }

    #[test]
    fn test_check_list_pair_formation() {
        let mut list = CheckList::new(true);

        let local = vec![
            host_candidate("192.168.1.100:5000"),
            host_candidate("10.0.0.1:5000"),
        ];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);

        // Should have 2 pairs (2 local × 1 remote, all IPv4)
        assert_eq!(list.pair_count(), 2);
    }

    #[test]
    fn test_check_list_priority_sorting() {
        let mut list = CheckList::new(true);

        // Host candidates have higher priority than srflx
        let local = vec![
            host_candidate("192.168.1.100:5000"),
            srflx_candidate("203.0.113.1:5000", "192.168.1.100:5000"),
        ];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);

        // First pair should be host-host (higher priority)
        let first = list.get_pair(0).unwrap();
        assert_eq!(first.local.candidate_type, CandidateType::Host);
    }

    #[test]
    fn test_check_list_initial_unfreeze() {
        let mut list = CheckList::new(true);

        let local = vec![host_candidate("192.168.1.100:5000")];
        let remote = vec![
            host_candidate("192.168.1.200:5000"),
            host_candidate("192.168.1.201:5000"),
        ];

        list.add_pairs(&local, &remote);

        // At least one pair should be unfrozen
        let waiting_count = list.pairs_by_state(CheckState::Waiting).count();
        assert!(waiting_count >= 1);
    }

    #[test]
    fn test_check_list_next_request() {
        let mut list = CheckList::new(true);

        let local = vec![host_candidate("192.168.1.100:5000")];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);
        list.start();

        // Should get a request for the first pair
        let result = list.next_request();
        assert!(result.is_some());

        let (idx, _request, addr) = result.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(addr, "192.168.1.200:5000".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_check_list_handle_response() {
        let mut list = CheckList::new(true);

        let local = vec![host_candidate("192.168.1.100:5000")];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);
        list.start();

        // Get and send request
        let (_, request, _) = list.next_request().unwrap();

        // Handle response
        let response = BindingResponse::success(
            request.transaction_id,
            "192.168.1.200:5000".parse().unwrap(),
        );
        let matched = list.handle_response(&response);

        assert_eq!(matched, Some(0));
        assert!(list.has_succeeded());
    }

    #[test]
    fn test_check_list_completion() {
        let mut list = CheckList::new(true);

        let local = vec![host_candidate("192.168.1.100:5000")];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);
        list.start();

        // Initially not complete
        assert!(!list.is_complete());

        // Get request and mark succeeded
        let (_, request, _) = list.next_request().unwrap();
        let response =
            BindingResponse::success(request.transaction_id, "1.2.3.4:5000".parse().unwrap());
        list.handle_response(&response);

        // Now complete
        assert!(list.is_complete());
    }

    #[test]
    fn test_check_list_nomination() {
        let mut list = CheckList::new(true);

        let local = vec![host_candidate("192.168.1.100:5000")];
        let remote = vec![host_candidate("192.168.1.200:5000")];

        list.add_pairs(&local, &remote);
        list.start();

        // Get request and succeed
        let (idx, request, _) = list.next_request().unwrap();
        let response =
            BindingResponse::success(request.transaction_id, "1.2.3.4:5000".parse().unwrap());
        list.handle_response(&response);

        // Nominate
        assert!(list.nominate(idx));
        assert!(list.get_pair(idx).unwrap().nominated);
    }

    #[test]
    fn test_exponential_backoff() {
        let local = host_candidate("192.168.1.100:5000");
        let remote = host_candidate("192.168.1.200:5000");
        let mut pair = CandidatePair::new(local, remote, true);

        pair.state = CheckState::Waiting;
        pair.start_check();

        // After start_check: transmit_count = 1, RTO = 100ms * 2^1 = 200ms
        assert_eq!(pair.current_rto(), Duration::from_millis(200));

        pair.record_retransmit();
        // transmit_count = 2, RTO = 100ms * 2^2 = 400ms
        assert_eq!(pair.current_rto(), Duration::from_millis(400));

        pair.record_retransmit();
        // transmit_count = 3, RTO = 100ms * 2^3 = 800ms
        assert_eq!(pair.current_rto(), Duration::from_millis(800));

        pair.record_retransmit();
        // transmit_count = 4, RTO = 100ms * 2^4 = 1600ms (max)
        assert_eq!(pair.current_rto(), Duration::from_millis(1600));

        pair.record_retransmit();
        // transmit_count = 5, but capped at max
        assert_eq!(pair.current_rto(), Duration::from_millis(1600));
    }

    #[test]
    fn test_ipv4_ipv6_separation() {
        let mut list = CheckList::new(true);

        let local = vec![
            host_candidate("192.168.1.100:5000"), // IPv4
            host_candidate("[::1]:5000"),         // IPv6
        ];
        let remote = vec![
            host_candidate("192.168.1.200:5000"), // IPv4
        ];

        list.add_pairs(&local, &remote);

        // Should only have 1 pair (IPv4-IPv4)
        // IPv6 local can't pair with IPv4 remote
        assert_eq!(list.pair_count(), 1);
    }
}
