//! Hole punching coordination for P2P connectivity
//!
//! This module orchestrates the complete hole punching flow:
//! 1. Gather local candidates
//! 2. Exchange candidates via Intermediate (signaling)
//! 3. Perform connectivity checks on candidate pairs
//! 4. Establish QUIC connection on successful path
//! 5. Select best path (direct vs relay)
//!
//! # State Machine
//!
//! ```text
//! Idle → Gathering → Signaling → Checking → Connected
//!                                    ↓
//!                                 Failed → FallbackRelay
//! ```

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::candidate::{Candidate, gather_host_candidates, gather_reflexive_candidate, gather_relay_candidate};
use super::connectivity::{BindingResponse, BindingMessage, CheckList, encode_binding, decode_binding};
use super::signaling::{SignalingMessage, encode_message, decode_message};

// ============================================================================
// Constants
// ============================================================================

/// Total timeout for hole punching process
pub const HOLE_PUNCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for signaling exchange
pub const SIGNALING_TIMEOUT: Duration = Duration::from_secs(5);

/// Delay before starting hole punching (coordination)
pub const DEFAULT_START_DELAY_MS: u64 = 100;

// ============================================================================
// Coordinator State
// ============================================================================

/// State of the hole punching process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolePunchState {
    /// Not started
    Idle,
    /// Gathering local candidates
    Gathering,
    /// Exchanging candidates via signaling
    Signaling,
    /// Waiting for coordinated start
    WaitingToStart,
    /// Performing connectivity checks
    Checking,
    /// Direct connection established
    Connected,
    /// All checks failed
    Failed,
    /// Falling back to relay
    FallbackRelay,
}

/// Result of a hole punch attempt
#[derive(Debug, Clone)]
pub enum HolePunchResult {
    /// Direct path established
    DirectPath {
        /// Remote address for direct connection
        remote_addr: SocketAddr,
        /// Round-trip time measurement
        rtt: Option<Duration>,
    },
    /// Must use relay (direct failed or not attempted)
    UseRelay {
        /// Reason for using relay
        reason: String,
    },
}

/// Coordinator for hole punching process
#[derive(Debug)]
pub struct HolePunchCoordinator {
    /// Current state
    state: HolePunchState,
    /// Session ID for this hole punch attempt
    session_id: u64,
    /// Service ID we're connecting to
    service_id: String,
    /// Whether this is the controlling agent (Agent = true, Connector = false)
    is_controlling: bool,
    /// Local candidates gathered
    local_candidates: Vec<Candidate>,
    /// Remote candidates received
    remote_candidates: Vec<Candidate>,
    /// Connectivity check list
    check_list: CheckList,
    /// When the process started
    start_time: Option<Instant>,
    /// When checking should start (after signaling)
    check_start_time: Option<Instant>,
    /// Intermediate server address (for relay candidate)
    intermediate_addr: Option<SocketAddr>,
    /// Observed address from QAD
    observed_addr: Option<SocketAddr>,
    /// Best working address (if found)
    working_addr: Option<SocketAddr>,
    /// Outgoing messages to send
    outgoing_messages: Vec<Vec<u8>>,
    /// Binding requests to send (addr, encoded message)
    outgoing_bindings: Vec<(SocketAddr, Vec<u8>)>,
}

impl HolePunchCoordinator {
    /// Create a new hole punch coordinator
    pub fn new(session_id: u64, service_id: String, is_controlling: bool) -> Self {
        Self {
            state: HolePunchState::Idle,
            session_id,
            service_id,
            is_controlling,
            local_candidates: Vec::new(),
            remote_candidates: Vec::new(),
            check_list: CheckList::new(is_controlling),
            start_time: None,
            check_start_time: None,
            intermediate_addr: None,
            observed_addr: None,
            working_addr: None,
            outgoing_messages: Vec::new(),
            outgoing_bindings: Vec::new(),
        }
    }

    /// Get current state
    pub fn state(&self) -> HolePunchState {
        self.state
    }

    /// Get session ID
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Get working address (if hole punching succeeded)
    pub fn working_address(&self) -> Option<SocketAddr> {
        self.working_addr
    }

    /// Set the intermediate server address (for relay candidate)
    pub fn set_intermediate_addr(&mut self, addr: SocketAddr) {
        self.intermediate_addr = Some(addr);
    }

    /// Set the observed address from QAD
    pub fn set_observed_addr(&mut self, addr: SocketAddr) {
        self.observed_addr = Some(addr);
    }

    /// Start gathering candidates
    pub fn start_gathering(&mut self, local_addresses: &[SocketAddr]) {
        self.state = HolePunchState::Gathering;
        self.start_time = Some(Instant::now());

        // Gather host candidates from local addresses
        self.local_candidates = gather_host_candidates(local_addresses, false);

        // Add server-reflexive candidate if we have QAD result
        if let Some(observed) = self.observed_addr {
            if let Some(base) = local_addresses.first() {
                if let Some(srflx) = gather_reflexive_candidate(observed, *base) {
                    self.local_candidates.push(srflx);
                }
            }
        }

        // Add relay candidate if we have Intermediate address
        if let Some(intermediate) = self.intermediate_addr {
            if let Some(base) = local_addresses.first() {
                let relay = gather_relay_candidate(intermediate, *base);
                self.local_candidates.push(relay);
            }
        }

        // Transition to signaling
        self.state = HolePunchState::Signaling;
    }

    /// Get candidate offer message to send to Intermediate
    pub fn get_candidate_offer(&self) -> Option<Vec<u8>> {
        if self.state != HolePunchState::Signaling {
            return None;
        }

        let msg = SignalingMessage::CandidateOffer {
            session_id: self.session_id,
            service_id: self.service_id.clone(),
            candidates: self.local_candidates.clone(),
        };

        encode_message(&msg).ok()
    }

    /// Process received signaling message
    pub fn process_signaling(&mut self, data: &[u8]) -> Result<(), String> {
        let (msg, _consumed) = decode_message(data)
            .map_err(|e| format!("Decode error: {}", e))?;

        match msg {
            SignalingMessage::CandidateOffer { session_id, candidates, .. } => {
                if session_id != self.session_id {
                    return Err("Session ID mismatch".to_string());
                }
                self.remote_candidates = candidates;

                // If we're the controlled side (Connector), send answer
                if !self.is_controlling {
                    let answer = SignalingMessage::CandidateAnswer {
                        session_id: self.session_id,
                        candidates: self.local_candidates.clone(),
                    };
                    if let Ok(encoded) = encode_message(&answer) {
                        self.outgoing_messages.push(encoded);
                    }
                }
            }
            SignalingMessage::CandidateAnswer { session_id, candidates } => {
                if session_id != self.session_id {
                    return Err("Session ID mismatch".to_string());
                }
                self.remote_candidates = candidates;
            }
            SignalingMessage::StartPunching { session_id, start_delay_ms, peer_candidates } => {
                if session_id != self.session_id {
                    return Err("Session ID mismatch".to_string());
                }
                // Update remote candidates if provided
                if !peer_candidates.is_empty() {
                    self.remote_candidates = peer_candidates;
                }
                // Schedule start time
                self.check_start_time = Some(Instant::now() + Duration::from_millis(start_delay_ms));
                self.state = HolePunchState::WaitingToStart;
            }
            SignalingMessage::PunchingResult { session_id, success, working_address } => {
                if session_id != self.session_id {
                    return Err("Session ID mismatch".to_string());
                }
                if success {
                    self.working_addr = working_address;
                    self.state = HolePunchState::Connected;
                } else {
                    self.state = HolePunchState::FallbackRelay;
                }
            }
            SignalingMessage::Error { session_id, message, .. } => {
                if session_id == Some(self.session_id) {
                    return Err(format!("Signaling error: {}", message));
                }
            }
        }

        Ok(())
    }

    /// Check if ready to start connectivity checks
    pub fn should_start_checking(&self) -> bool {
        match (self.state, self.check_start_time) {
            (HolePunchState::WaitingToStart, Some(start)) => Instant::now() >= start,
            (HolePunchState::Signaling, _) => {
                // Also allow starting if we have remote candidates but no explicit start signal
                !self.remote_candidates.is_empty() && !self.local_candidates.is_empty()
            }
            _ => false,
        }
    }

    /// Start connectivity checks
    pub fn start_checking(&mut self) {
        if self.local_candidates.is_empty() || self.remote_candidates.is_empty() {
            self.state = HolePunchState::Failed;
            return;
        }

        // Build check list
        self.check_list = CheckList::new(self.is_controlling);
        self.check_list.add_pairs(&self.local_candidates, &self.remote_candidates);
        self.check_list.start();

        self.state = HolePunchState::Checking;
    }

    /// Poll for next binding request to send
    ///
    /// Returns (remote_address, encoded_binding_request)
    pub fn poll_binding_request(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        if self.state != HolePunchState::Checking {
            return None;
        }

        // Check for queued bindings first
        if let Some(binding) = self.outgoing_bindings.pop() {
            return Some(binding);
        }

        // Get next from check list
        if let Some((_idx, request, addr)) = self.check_list.next_request() {
            let msg = BindingMessage::Request(request);
            if let Ok(encoded) = encode_binding(&msg) {
                return Some((addr, encoded));
            }
        }

        None
    }

    /// Process received binding message
    pub fn process_binding(&mut self, from: SocketAddr, data: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let msg = decode_binding(data)?;

        match msg {
            BindingMessage::Request(request) => {
                // Send response back
                let response = BindingResponse::success(request.transaction_id, from);
                let resp_msg = BindingMessage::Response(response);
                let encoded = encode_binding(&resp_msg)?;
                Ok(Some(encoded))
            }
            BindingMessage::Response(response) => {
                // Handle response
                if let Some(_idx) = self.check_list.handle_response(&response) {
                    // Check if we have a successful pair
                    if let Some(pair) = self.check_list.get_best_succeeded() {
                        self.working_addr = Some(pair.remote.address);
                        self.state = HolePunchState::Connected;
                    }
                }
                Ok(None)
            }
        }
    }

    /// Handle timeouts
    pub fn on_timeout(&mut self) {
        if self.state == HolePunchState::Checking {
            self.check_list.handle_timeouts();

            // Check for overall timeout
            if self.check_list.is_timed_out() && !self.check_list.has_succeeded() {
                self.state = HolePunchState::Failed;
            } else if self.check_list.is_complete() {
                if self.check_list.has_succeeded() {
                    if let Some(pair) = self.check_list.get_best_succeeded() {
                        self.working_addr = Some(pair.remote.address);
                        self.state = HolePunchState::Connected;
                    }
                } else {
                    self.state = HolePunchState::Failed;
                }
            }
        }

        // Check overall hole punch timeout
        if let Some(start) = self.start_time {
            if start.elapsed() >= HOLE_PUNCH_TIMEOUT {
                if self.state != HolePunchState::Connected {
                    self.state = HolePunchState::Failed;
                }
            }
        }
    }

    /// Get next outgoing signaling message
    pub fn poll_signaling_message(&mut self) -> Option<Vec<u8>> {
        self.outgoing_messages.pop()
    }

    /// Get result of hole punching
    pub fn result(&self) -> HolePunchResult {
        match self.state {
            HolePunchState::Connected => {
                if let Some(addr) = self.working_addr {
                    HolePunchResult::DirectPath {
                        remote_addr: addr,
                        rtt: None, // Could measure from check timing
                    }
                } else {
                    HolePunchResult::UseRelay {
                        reason: "Connected but no working address".to_string(),
                    }
                }
            }
            HolePunchState::Failed | HolePunchState::FallbackRelay => {
                HolePunchResult::UseRelay {
                    reason: "All connectivity checks failed".to_string(),
                }
            }
            _ => HolePunchResult::UseRelay {
                reason: format!("Hole punch not complete: {:?}", self.state),
            }
        }
    }

    /// Get local candidates (for testing/debugging)
    pub fn local_candidates(&self) -> &[Candidate] {
        &self.local_candidates
    }

    /// Get remote candidates (for testing/debugging)
    pub fn remote_candidates(&self) -> &[Candidate] {
        &self.remote_candidates
    }

    /// Get check list (for testing/debugging)
    pub fn check_list(&self) -> &CheckList {
        &self.check_list
    }
}

// ============================================================================
// Path Selection
// ============================================================================

/// Path selection decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSelection {
    /// Use direct P2P path
    Direct,
    /// Use relay through Intermediate
    Relay,
}

/// Decide whether to use direct or relay path
///
/// # Arguments
/// * `direct_rtt` - RTT on direct path (None if not available)
/// * `relay_rtt` - RTT on relay path
/// * `direct_success_rate` - Success rate of direct path (0.0 - 1.0)
///
/// # Returns
/// Whether to use direct or relay
pub fn select_path(
    direct_rtt: Option<Duration>,
    relay_rtt: Duration,
    direct_success_rate: f64,
) -> PathSelection {
    // If no direct path available, use relay
    let direct_rtt = match direct_rtt {
        Some(rtt) => rtt,
        None => return PathSelection::Relay,
    };

    // If direct path is unreliable, use relay
    if direct_success_rate < 0.8 {
        return PathSelection::Relay;
    }

    // Use direct if it's at least 30% faster than relay
    let threshold = relay_rtt * 70 / 100;
    if direct_rtt <= threshold {
        PathSelection::Direct
    } else {
        // Relay is similar or faster
        PathSelection::Relay
    }
}

/// Should switch from relay to direct path?
pub fn should_switch_to_direct(
    direct_rtt: Duration,
    relay_rtt: Duration,
) -> bool {
    // Switch if direct is at least 50% faster
    direct_rtt < relay_rtt / 2
}

/// Should switch from direct to relay path?
pub fn should_switch_to_relay(
    direct_rtt: Duration,
    relay_rtt: Duration,
    consecutive_failures: u32,
) -> bool {
    // Switch if we've had multiple failures
    if consecutive_failures >= 3 {
        return true;
    }

    // Switch if relay is now significantly faster (shouldn't happen often)
    relay_rtt < direct_rtt / 2
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::candidate::CandidateType;

    #[test]
    fn test_coordinator_creation() {
        let coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);

        assert_eq!(coord.state(), HolePunchState::Idle);
        assert_eq!(coord.session_id(), 12345);
        assert!(coord.working_address().is_none());
    }

    #[test]
    fn test_gathering_host_candidates() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);

        let local_addrs = vec![
            "192.168.1.100:5000".parse().unwrap(),
            "10.0.0.1:5000".parse().unwrap(),
        ];

        coord.start_gathering(&local_addrs);

        assert_eq!(coord.state(), HolePunchState::Signaling);
        assert_eq!(coord.local_candidates().len(), 2);
        assert!(coord.local_candidates().iter().all(|c| c.candidate_type == CandidateType::Host));
    }

    #[test]
    fn test_gathering_with_reflexive() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.set_observed_addr("203.0.113.50:5000".parse().unwrap());

        let local_addrs = vec!["192.168.1.100:5000".parse().unwrap()];
        coord.start_gathering(&local_addrs);

        assert_eq!(coord.local_candidates().len(), 2); // 1 host + 1 srflx
        assert!(coord.local_candidates().iter().any(|c| c.candidate_type == CandidateType::Host));
        assert!(coord.local_candidates().iter().any(|c| c.candidate_type == CandidateType::ServerReflexive));
    }

    #[test]
    fn test_gathering_with_relay() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.set_intermediate_addr("1.2.3.4:4433".parse().unwrap());

        let local_addrs = vec!["192.168.1.100:5000".parse().unwrap()];
        coord.start_gathering(&local_addrs);

        assert_eq!(coord.local_candidates().len(), 2); // 1 host + 1 relay
        assert!(coord.local_candidates().iter().any(|c| c.candidate_type == CandidateType::Relay));
    }

    #[test]
    fn test_candidate_offer_generation() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        let offer = coord.get_candidate_offer();
        assert!(offer.is_some());

        // Decode and verify
        let (msg, _) = decode_message(&offer.unwrap()).unwrap();
        match msg {
            SignalingMessage::CandidateOffer { session_id, service_id, candidates } => {
                assert_eq!(session_id, 12345);
                assert_eq!(service_id, "test-service");
                assert_eq!(candidates.len(), 1);
            }
            _ => panic!("Expected CandidateOffer"),
        }
    }

    #[test]
    fn test_signaling_flow() {
        // Agent side
        let mut agent = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        agent.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        // Connector side
        let mut connector = HolePunchCoordinator::new(12345, "test-service".to_string(), false);
        connector.start_gathering(&["192.168.1.200:5000".parse().unwrap()]);

        // Agent sends offer
        let offer = agent.get_candidate_offer().unwrap();

        // Connector receives offer and sends answer
        connector.process_signaling(&offer).unwrap();
        let answer = connector.poll_signaling_message().unwrap();

        // Agent receives answer
        agent.process_signaling(&answer).unwrap();

        // Both should have remote candidates
        assert!(!agent.remote_candidates().is_empty());
        assert!(!connector.remote_candidates().is_empty());
    }

    #[test]
    fn test_start_checking() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        // Simulate receiving remote candidates
        let remote_offer = SignalingMessage::CandidateAnswer {
            session_id: 12345,
            candidates: vec![Candidate::host("192.168.1.200:5000".parse().unwrap())],
        };
        let encoded = encode_message(&remote_offer).unwrap();
        coord.process_signaling(&encoded).unwrap();

        assert!(coord.should_start_checking());
        coord.start_checking();

        assert_eq!(coord.state(), HolePunchState::Checking);
        assert_eq!(coord.check_list().pair_count(), 1);
    }

    #[test]
    fn test_binding_request_poll() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        // Add remote candidates
        let remote_offer = SignalingMessage::CandidateAnswer {
            session_id: 12345,
            candidates: vec![Candidate::host("192.168.1.200:5000".parse().unwrap())],
        };
        coord.process_signaling(&encode_message(&remote_offer).unwrap()).unwrap();
        coord.start_checking();

        // Should get a binding request
        let result = coord.poll_binding_request();
        assert!(result.is_some());

        let (addr, data) = result.unwrap();
        assert_eq!(addr, "192.168.1.200:5000".parse::<SocketAddr>().unwrap());

        // Verify it's a binding request
        let msg = decode_binding(&data).unwrap();
        assert!(matches!(msg, BindingMessage::Request(_)));
    }

    #[test]
    fn test_binding_response_handling() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        // Add remote candidates
        let remote_offer = SignalingMessage::CandidateAnswer {
            session_id: 12345,
            candidates: vec![Candidate::host("192.168.1.200:5000".parse().unwrap())],
        };
        coord.process_signaling(&encode_message(&remote_offer).unwrap()).unwrap();
        coord.start_checking();

        // Get binding request
        let (addr, data) = coord.poll_binding_request().unwrap();

        // Decode to get transaction ID
        let msg = decode_binding(&data).unwrap();
        let txn_id = match msg {
            BindingMessage::Request(req) => req.transaction_id,
            _ => panic!("Expected request"),
        };

        // Create and process response
        let response = BindingResponse::success(txn_id, addr);
        let resp_msg = BindingMessage::Response(response);
        let resp_data = encode_binding(&resp_msg).unwrap();

        coord.process_binding(addr, &resp_data).unwrap();

        // Should be connected
        assert_eq!(coord.state(), HolePunchState::Connected);
        assert_eq!(coord.working_address(), Some(addr));
    }

    #[test]
    fn test_path_selection_direct_faster() {
        let direct_rtt = Duration::from_millis(50);
        let relay_rtt = Duration::from_millis(200);

        let selection = select_path(Some(direct_rtt), relay_rtt, 0.95);
        assert_eq!(selection, PathSelection::Direct);
    }

    #[test]
    fn test_path_selection_relay_no_direct() {
        let relay_rtt = Duration::from_millis(100);

        let selection = select_path(None, relay_rtt, 0.0);
        assert_eq!(selection, PathSelection::Relay);
    }

    #[test]
    fn test_path_selection_relay_unreliable() {
        let direct_rtt = Duration::from_millis(50);
        let relay_rtt = Duration::from_millis(200);

        // Low success rate means use relay
        let selection = select_path(Some(direct_rtt), relay_rtt, 0.5);
        assert_eq!(selection, PathSelection::Relay);
    }

    #[test]
    fn test_should_switch_to_direct() {
        // Direct is 3x faster - should switch
        assert!(should_switch_to_direct(
            Duration::from_millis(30),
            Duration::from_millis(100),
        ));

        // Direct is only slightly faster - don't switch
        assert!(!should_switch_to_direct(
            Duration::from_millis(80),
            Duration::from_millis(100),
        ));
    }

    #[test]
    fn test_should_switch_to_relay() {
        // Multiple failures - switch
        assert!(should_switch_to_relay(
            Duration::from_millis(50),
            Duration::from_millis(100),
            3,
        ));

        // No failures, direct still good - don't switch
        assert!(!should_switch_to_relay(
            Duration::from_millis(50),
            Duration::from_millis(100),
            0,
        ));
    }

    #[test]
    fn test_result_connected() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.working_addr = Some("192.168.1.200:5000".parse().unwrap());
        coord.state = HolePunchState::Connected;

        match coord.result() {
            HolePunchResult::DirectPath { remote_addr, .. } => {
                assert_eq!(remote_addr, "192.168.1.200:5000".parse::<SocketAddr>().unwrap());
            }
            _ => panic!("Expected DirectPath"),
        }
    }

    #[test]
    fn test_result_failed() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.state = HolePunchState::Failed;

        match coord.result() {
            HolePunchResult::UseRelay { reason } => {
                assert!(reason.contains("failed"));
            }
            _ => panic!("Expected UseRelay"),
        }
    }

    #[test]
    fn test_timeout_handling() {
        let mut coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);
        coord.start_gathering(&["192.168.1.100:5000".parse().unwrap()]);

        // Add remote candidates and start checking
        let remote_offer = SignalingMessage::CandidateAnswer {
            session_id: 12345,
            candidates: vec![Candidate::host("192.168.1.200:5000".parse().unwrap())],
        };
        coord.process_signaling(&encode_message(&remote_offer).unwrap()).unwrap();
        coord.start_checking();

        // Simulate timeout by getting all requests without responding
        while coord.poll_binding_request().is_some() {}

        // Process timeouts - won't fail immediately due to retransmits
        coord.on_timeout();

        // State should still be checking (waiting for retransmits/timeout)
        assert!(matches!(
            coord.state(),
            HolePunchState::Checking | HolePunchState::Failed
        ));
    }
}
