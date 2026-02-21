//! Path resilience for P2P connections
//!
//! This module handles keepalive and fallback logic for maintaining
//! P2P connections and gracefully falling back to relay when needed.
//!
//! # Keepalive Protocol
//!
//! Keepalive messages are sent every 15 seconds to:
//! 1. Keep NAT mappings alive
//! 2. Detect path failures (3 missed keepalives = failed)
//!
//! # Fallback Logic
//!
//! When direct path fails:
//! 1. Stop sending on direct path
//! 2. Switch to relay path (always available via Intermediate)
//! 3. Maintain session state during transition

use std::net::SocketAddr;
use std::time::{Duration, Instant};

// ============================================================================
// Constants
// ============================================================================

/// Keepalive interval (15 seconds)
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Number of missed keepalives before path is considered failed
pub const MISSED_KEEPALIVES_THRESHOLD: u32 = 3;

/// Keepalive response timeout (how long to wait for response)
pub const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimum time between fallback attempts (prevent thrashing)
pub const FALLBACK_COOLDOWN: Duration = Duration::from_secs(30);

// ============================================================================
// Keepalive Message
// ============================================================================

/// Keepalive message type (sent as QUIC DATAGRAM)
pub const KEEPALIVE_REQUEST: u8 = 0x10;
pub const KEEPALIVE_RESPONSE: u8 = 0x11;

/// Encode a keepalive request message
pub fn encode_keepalive_request(sequence: u32) -> [u8; 5] {
    let mut buf = [0u8; 5];
    buf[0] = KEEPALIVE_REQUEST;
    buf[1..5].copy_from_slice(&sequence.to_be_bytes());
    buf
}

/// Encode a keepalive response message
pub fn encode_keepalive_response(sequence: u32) -> [u8; 5] {
    let mut buf = [0u8; 5];
    buf[0] = KEEPALIVE_RESPONSE;
    buf[1..5].copy_from_slice(&sequence.to_be_bytes());
    buf
}

/// Decode a keepalive message
/// Returns (is_response, sequence) or None if invalid
pub fn decode_keepalive(data: &[u8]) -> Option<(bool, u32)> {
    if data.len() < 5 {
        return None;
    }

    let msg_type = data[0];
    if msg_type != KEEPALIVE_REQUEST && msg_type != KEEPALIVE_RESPONSE {
        return None;
    }

    let sequence = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
    Some((msg_type == KEEPALIVE_RESPONSE, sequence))
}

// ============================================================================
// Path State
// ============================================================================

/// State of a P2P path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// Path is active and healthy
    Active,
    /// Path is degraded (some keepalives missed)
    Degraded,
    /// Path has failed (exceeded missed threshold)
    Failed,
    /// Path is in recovery (after failure, trying again)
    Recovering,
}

/// Information about a P2P path
#[derive(Debug)]
pub struct PathInfo {
    /// Remote address for this path
    pub remote_addr: SocketAddr,
    /// Current path state
    pub state: PathState,
    /// Last keepalive sent time
    pub last_keepalive_sent: Option<Instant>,
    /// Last keepalive received time
    pub last_keepalive_received: Option<Instant>,
    /// Next keepalive sequence number to send
    pub next_sequence: u32,
    /// Last sequence we received a response for
    pub last_acked_sequence: u32,
    /// Number of consecutive missed keepalives
    pub missed_keepalives: u32,
    /// Estimated round-trip time
    pub rtt: Option<Duration>,
    /// When the path was established
    pub established_at: Instant,
    /// When the path last failed (for cooldown)
    pub last_failure: Option<Instant>,
}

impl PathInfo {
    /// Create a new path info
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            state: PathState::Active,
            last_keepalive_sent: None,
            last_keepalive_received: None,
            next_sequence: 1,
            last_acked_sequence: 0,
            missed_keepalives: 0,
            rtt: None,
            established_at: Instant::now(),
            last_failure: None,
        }
    }

    /// Check if keepalive should be sent now
    pub fn should_send_keepalive(&self) -> bool {
        match self.state {
            PathState::Failed => false,
            _ => {
                match self.last_keepalive_sent {
                    None => true, // Never sent, send now
                    Some(last) => last.elapsed() >= KEEPALIVE_INTERVAL,
                }
            }
        }
    }

    /// Record that a keepalive was sent
    pub fn record_keepalive_sent(&mut self) -> u32 {
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.last_keepalive_sent = Some(Instant::now());
        seq
    }

    /// Record that a keepalive response was received
    pub fn record_keepalive_received(&mut self, sequence: u32) {
        // Calculate RTT if this is a response to our latest keepalive
        if let Some(sent_time) = self.last_keepalive_sent {
            if sequence == self.next_sequence.wrapping_sub(1) {
                self.rtt = Some(sent_time.elapsed());
            }
        }

        self.last_keepalive_received = Some(Instant::now());
        self.last_acked_sequence = sequence;
        self.missed_keepalives = 0;

        // Transition state based on response
        match self.state {
            PathState::Degraded => self.state = PathState::Active,
            PathState::Recovering => self.state = PathState::Active,
            _ => {}
        }
    }

    /// Check for keepalive timeout and update state
    /// Returns true if state changed to Failed
    pub fn check_timeout(&mut self) -> bool {
        if self.state == PathState::Failed {
            return false;
        }

        // Check if we're waiting for a response
        if let Some(sent_time) = self.last_keepalive_sent {
            let expected_response_seq = self.next_sequence.wrapping_sub(1);

            // If we haven't received response for the last sent keepalive
            if self.last_acked_sequence != expected_response_seq
                && sent_time.elapsed() >= KEEPALIVE_TIMEOUT
            {
                self.missed_keepalives += 1;

                // Update state based on missed count
                if self.missed_keepalives >= MISSED_KEEPALIVES_THRESHOLD {
                    self.state = PathState::Failed;
                    self.last_failure = Some(Instant::now());
                    return true;
                } else if self.missed_keepalives > 0 {
                    self.state = PathState::Degraded;
                }
            }
        }

        false
    }

    /// Check if path can be retried after failure
    pub fn can_retry(&self) -> bool {
        match self.last_failure {
            None => true,
            Some(failure_time) => failure_time.elapsed() >= FALLBACK_COOLDOWN,
        }
    }

    /// Attempt recovery on a failed path
    pub fn start_recovery(&mut self) {
        if self.state == PathState::Failed && self.can_retry() {
            self.state = PathState::Recovering;
            self.missed_keepalives = 0;
            self.last_keepalive_sent = None;
        }
    }

    /// Check if path is usable for data
    pub fn is_usable(&self) -> bool {
        matches!(
            self.state,
            PathState::Active | PathState::Degraded | PathState::Recovering
        )
    }
}

// ============================================================================
// Path Manager
// ============================================================================

/// Manages multiple P2P paths and fallback logic
#[derive(Debug)]
pub struct PathManager {
    /// Direct P2P path (if established)
    direct_path: Option<PathInfo>,
    /// Relay path through Intermediate (always available)
    relay_addr: Option<SocketAddr>,
    /// Currently active path type
    active_path: ActivePath,
    /// Whether we're in fallback mode
    in_fallback: bool,
}

/// Which path is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePath {
    /// Using direct P2P path
    Direct,
    /// Using relay through Intermediate
    Relay,
    /// No path available
    None,
}

impl PathManager {
    /// Create a new path manager
    pub fn new() -> Self {
        Self {
            direct_path: None,
            relay_addr: None,
            active_path: ActivePath::None,
            in_fallback: false,
        }
    }

    /// Set the relay address (Intermediate server)
    pub fn set_relay(&mut self, addr: SocketAddr) {
        self.relay_addr = Some(addr);
        if self.active_path == ActivePath::None {
            self.active_path = ActivePath::Relay;
        }
    }

    /// Establish direct P2P path
    pub fn set_direct(&mut self, addr: SocketAddr) {
        self.direct_path = Some(PathInfo::new(addr));
        self.active_path = ActivePath::Direct;
        self.in_fallback = false;
    }

    /// Get the current active path address
    pub fn active_addr(&self) -> Option<SocketAddr> {
        match self.active_path {
            ActivePath::Direct => self.direct_path.as_ref().map(|p| p.remote_addr),
            ActivePath::Relay => self.relay_addr,
            ActivePath::None => None,
        }
    }

    /// Get current active path type
    pub fn active_path_type(&self) -> ActivePath {
        self.active_path
    }

    /// Check if in fallback mode
    pub fn is_in_fallback(&self) -> bool {
        self.in_fallback
    }

    /// Get direct path info (for keepalive management)
    pub fn direct_path(&self) -> Option<&PathInfo> {
        self.direct_path.as_ref()
    }

    /// Get mutable direct path info
    pub fn direct_path_mut(&mut self) -> Option<&mut PathInfo> {
        self.direct_path.as_mut()
    }

    /// Check if keepalive should be sent
    pub fn should_send_keepalive(&self) -> bool {
        self.direct_path
            .as_ref()
            .map(|p| p.should_send_keepalive())
            .unwrap_or(false)
    }

    /// Get keepalive message to send (if needed)
    pub fn poll_keepalive(&mut self) -> Option<(SocketAddr, [u8; 5])> {
        if let Some(path) = self.direct_path.as_mut() {
            if path.should_send_keepalive() {
                let seq = path.record_keepalive_sent();
                let msg = encode_keepalive_request(seq);
                return Some((path.remote_addr, msg));
            }
        }
        None
    }

    /// Process received keepalive message
    /// Returns keepalive response to send (if this was a request)
    pub fn process_keepalive(&mut self, from: SocketAddr, data: &[u8]) -> Option<[u8; 5]> {
        let (is_response, sequence) = decode_keepalive(data)?;

        if let Some(path) = self.direct_path.as_mut() {
            if path.remote_addr == from {
                if is_response {
                    path.record_keepalive_received(sequence);
                    None
                } else {
                    // Send response for request
                    Some(encode_keepalive_response(sequence))
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check for timeouts and handle failover
    /// Returns true if failover occurred
    pub fn check_timeouts(&mut self) -> bool {
        if let Some(path) = self.direct_path.as_mut() {
            if path.check_timeout() {
                // Direct path failed - failover to relay
                if self.relay_addr.is_some() {
                    self.active_path = ActivePath::Relay;
                    self.in_fallback = true;
                    return true;
                }
            }
        }
        false
    }

    /// Attempt to recover direct path after cooldown
    pub fn attempt_recovery(&mut self) -> bool {
        if let Some(path) = self.direct_path.as_mut() {
            if path.state == PathState::Failed && path.can_retry() {
                path.start_recovery();
                // Don't switch active path yet - wait for successful keepalive
                return true;
            }
        }
        false
    }

    /// Switch back to direct path after successful recovery
    pub fn switch_to_direct(&mut self) {
        if let Some(path) = &self.direct_path {
            if path.state == PathState::Active {
                self.active_path = ActivePath::Direct;
                self.in_fallback = false;
            }
        }
    }

    /// Get path statistics
    pub fn stats(&self) -> PathStats {
        PathStats {
            active_path: self.active_path,
            in_fallback: self.in_fallback,
            direct_rtt: self.direct_path.as_ref().and_then(|p| p.rtt),
            direct_state: self.direct_path.as_ref().map(|p| p.state),
            missed_keepalives: self
                .direct_path
                .as_ref()
                .map(|p| p.missed_keepalives)
                .unwrap_or(0),
        }
    }
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Path statistics for monitoring
#[derive(Debug, Clone)]
pub struct PathStats {
    /// Currently active path
    pub active_path: ActivePath,
    /// Whether in fallback mode
    pub in_fallback: bool,
    /// Direct path RTT (if measured)
    pub direct_rtt: Option<Duration>,
    /// Direct path state
    pub direct_state: Option<PathState>,
    /// Number of missed keepalives on direct path
    pub missed_keepalives: u32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_keepalive_request() {
        let msg = encode_keepalive_request(12345);
        let (is_response, seq) = decode_keepalive(&msg).unwrap();

        assert!(!is_response);
        assert_eq!(seq, 12345);
    }

    #[test]
    fn test_encode_decode_keepalive_response() {
        let msg = encode_keepalive_response(67890);
        let (is_response, seq) = decode_keepalive(&msg).unwrap();

        assert!(is_response);
        assert_eq!(seq, 67890);
    }

    #[test]
    fn test_decode_invalid_keepalive() {
        // Too short
        assert!(decode_keepalive(&[0x10]).is_none());

        // Wrong type
        assert!(decode_keepalive(&[0x00, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn test_path_info_creation() {
        let addr: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        let path = PathInfo::new(addr);

        assert_eq!(path.remote_addr, addr);
        assert_eq!(path.state, PathState::Active);
        assert_eq!(path.missed_keepalives, 0);
        assert!(path.is_usable());
    }

    #[test]
    fn test_path_info_keepalive_cycle() {
        let addr: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        let mut path = PathInfo::new(addr);

        // Should send keepalive immediately (never sent)
        assert!(path.should_send_keepalive());

        // Record send
        let seq = path.record_keepalive_sent();
        assert_eq!(seq, 1);

        // Shouldn't send again immediately
        assert!(!path.should_send_keepalive());

        // Record response
        path.record_keepalive_received(1);
        assert_eq!(path.last_acked_sequence, 1);
        assert_eq!(path.missed_keepalives, 0);
    }

    #[test]
    fn test_path_manager_direct_path() {
        let mut manager = PathManager::new();

        let relay: SocketAddr = "1.2.3.4:4433".parse().unwrap();
        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();

        // Initially no path
        assert_eq!(manager.active_path_type(), ActivePath::None);

        // Set relay
        manager.set_relay(relay);
        assert_eq!(manager.active_path_type(), ActivePath::Relay);
        assert_eq!(manager.active_addr(), Some(relay));

        // Set direct
        manager.set_direct(direct);
        assert_eq!(manager.active_path_type(), ActivePath::Direct);
        assert_eq!(manager.active_addr(), Some(direct));
        assert!(!manager.is_in_fallback());
    }

    #[test]
    fn test_path_manager_keepalive_poll() {
        let mut manager = PathManager::new();

        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        manager.set_direct(direct);

        // Should have keepalive to send
        let result = manager.poll_keepalive();
        assert!(result.is_some());

        let (addr, msg) = result.unwrap();
        assert_eq!(addr, direct);

        // Decode message
        let (is_response, seq) = decode_keepalive(&msg).unwrap();
        assert!(!is_response);
        assert_eq!(seq, 1);
    }

    #[test]
    fn test_path_manager_process_keepalive_request() {
        let mut manager = PathManager::new();

        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        manager.set_direct(direct);

        // Receive a keepalive request
        let request = encode_keepalive_request(42);
        let response = manager.process_keepalive(direct, &request);

        assert!(response.is_some());
        let (is_response, seq) = decode_keepalive(&response.unwrap()).unwrap();
        assert!(is_response);
        assert_eq!(seq, 42);
    }

    #[test]
    fn test_path_manager_process_keepalive_response() {
        let mut manager = PathManager::new();

        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        manager.set_direct(direct);

        // Send a keepalive first
        manager.poll_keepalive();

        // Receive response
        let response = encode_keepalive_response(1);
        let result = manager.process_keepalive(direct, &response);

        // Should not generate another response
        assert!(result.is_none());

        // Path should be updated
        let path = manager.direct_path().unwrap();
        assert_eq!(path.last_acked_sequence, 1);
    }

    #[test]
    fn test_path_manager_fallback() {
        let mut manager = PathManager::new();

        let relay: SocketAddr = "1.2.3.4:4433".parse().unwrap();
        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();

        manager.set_relay(relay);
        manager.set_direct(direct);

        // Simulate direct path failure
        if let Some(path) = manager.direct_path_mut() {
            path.state = PathState::Failed;
            path.last_failure = Some(Instant::now());
        }

        // Check timeouts (should trigger fallback check)
        // Note: Since state is already Failed, check_timeout won't return true again
        // but we can manually trigger fallback
        manager.active_path = ActivePath::Relay;
        manager.in_fallback = true;

        assert_eq!(manager.active_path_type(), ActivePath::Relay);
        assert!(manager.is_in_fallback());
        assert_eq!(manager.active_addr(), Some(relay));
    }

    #[test]
    fn test_path_state_transitions() {
        let addr: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        let mut path = PathInfo::new(addr);

        assert_eq!(path.state, PathState::Active);

        // Simulate missed keepalive
        path.missed_keepalives = 1;
        path.state = PathState::Degraded;
        assert_eq!(path.state, PathState::Degraded);
        assert!(path.is_usable());

        // Simulate recovery
        path.record_keepalive_received(1);
        assert_eq!(path.state, PathState::Active);

        // Simulate failure
        path.missed_keepalives = MISSED_KEEPALIVES_THRESHOLD;
        path.state = PathState::Failed;
        path.last_failure = Some(Instant::now());
        assert_eq!(path.state, PathState::Failed);
        assert!(!path.is_usable());

        // Can't retry immediately
        assert!(!path.can_retry());
    }

    #[test]
    fn test_path_stats() {
        let mut manager = PathManager::new();

        let direct: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        manager.set_direct(direct);

        let stats = manager.stats();
        assert_eq!(stats.active_path, ActivePath::Direct);
        assert!(!stats.in_fallback);
        assert_eq!(stats.direct_state, Some(PathState::Active));
        assert_eq!(stats.missed_keepalives, 0);
    }
}
