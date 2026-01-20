//! ICE-style candidate gathering for P2P connections
//!
//! Implements candidate types and priority calculation based on RFC 8445.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

// ============================================================================
// Constants (RFC 8445 Section 5.1.2.1)
// ============================================================================

/// Type preference for host candidates (highest priority)
const HOST_TYPE_PREF: u32 = 126;

/// Type preference for server reflexive candidates
const SRFLX_TYPE_PREF: u32 = 100;

/// Type preference for peer reflexive candidates
const PRFLX_TYPE_PREF: u32 = 110;

/// Type preference for relay candidates (lowest priority)
const RELAY_TYPE_PREF: u32 = 0;

/// Local preference for IPv4 addresses
const IPV4_LOCAL_PREF: u32 = 65535;

/// Local preference for IPv6 addresses (slightly lower than IPv4 for now)
const IPV6_LOCAL_PREF: u32 = 65534;

/// Component ID for RTP (we only use one component)
const COMPONENT_ID: u32 = 1;

// ============================================================================
// Candidate Types
// ============================================================================

/// Type of ICE candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum CandidateType {
    /// Local network interface address
    Host = 0,
    /// Server reflexive (public address from STUN/QAD)
    ServerReflexive = 1,
    /// Peer reflexive (discovered during connectivity checks)
    PeerReflexive = 2,
    /// Relay candidate (via TURN/Intermediate)
    Relay = 3,
}

impl CandidateType {
    /// Get the type preference value for priority calculation
    pub fn type_preference(&self) -> u32 {
        match self {
            CandidateType::Host => HOST_TYPE_PREF,
            CandidateType::ServerReflexive => SRFLX_TYPE_PREF,
            CandidateType::PeerReflexive => PRFLX_TYPE_PREF,
            CandidateType::Relay => RELAY_TYPE_PREF,
        }
    }
}

impl fmt::Display for CandidateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandidateType::Host => write!(f, "host"),
            CandidateType::ServerReflexive => write!(f, "srflx"),
            CandidateType::PeerReflexive => write!(f, "prflx"),
            CandidateType::Relay => write!(f, "relay"),
        }
    }
}

// ============================================================================
// Candidate
// ============================================================================

/// An ICE candidate representing a potential address for connectivity
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Type of candidate (host, srflx, prflx, relay)
    pub candidate_type: CandidateType,
    /// Transport address (IP:port)
    pub address: SocketAddr,
    /// Priority (higher = more preferred)
    pub priority: u32,
    /// Foundation string for candidate pairing
    pub foundation: String,
    /// Related address (e.g., host address for srflx)
    pub related_address: Option<SocketAddr>,
}

impl Candidate {
    /// Create a new candidate with calculated priority
    pub fn new(
        candidate_type: CandidateType,
        address: SocketAddr,
        related_address: Option<SocketAddr>,
    ) -> Self {
        let local_pref = local_preference(&address);
        let priority = calculate_priority(candidate_type.type_preference(), local_pref, COMPONENT_ID);
        let foundation = generate_foundation(candidate_type, &address);

        Self {
            candidate_type,
            address,
            priority,
            foundation,
            related_address,
        }
    }

    /// Create a host candidate from a local address
    pub fn host(address: SocketAddr) -> Self {
        Self::new(CandidateType::Host, address, None)
    }

    /// Create a server reflexive candidate from QAD response
    pub fn server_reflexive(public_address: SocketAddr, base_address: SocketAddr) -> Self {
        Self::new(CandidateType::ServerReflexive, public_address, Some(base_address))
    }

    /// Create a relay candidate (via Intermediate)
    pub fn relay(relay_address: SocketAddr, base_address: SocketAddr) -> Self {
        Self::new(CandidateType::Relay, relay_address, Some(base_address))
    }

    /// Check if this is a loopback candidate
    pub fn is_loopback(&self) -> bool {
        self.address.ip().is_loopback()
    }

    /// Check if this is a link-local candidate
    pub fn is_link_local(&self) -> bool {
        match self.address.ip() {
            IpAddr::V4(addr) => addr.is_link_local(),
            IpAddr::V6(addr) => {
                // fe80::/10 is link-local for IPv6
                let segments = addr.segments();
                (segments[0] & 0xffc0) == 0xfe80
            }
        }
    }
}

impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} priority {} foundation {}",
            self.candidate_type, self.address, self.priority, self.foundation
        )
    }
}

// ============================================================================
// Priority Calculation (RFC 8445 Section 5.1.2.1)
// ============================================================================

/// Calculate candidate priority per RFC 8445
///
/// Formula: priority = (2^24 * type_preference) + (2^8 * local_preference) + (256 - component_id)
///
/// # Arguments
/// * `type_pref` - Type preference (0-126)
/// * `local_pref` - Local preference (0-65535)
/// * `component_id` - Component ID (1-256)
pub fn calculate_priority(type_pref: u32, local_pref: u32, component_id: u32) -> u32 {
    // Ensure values are in valid ranges
    let type_pref = type_pref.min(126);
    let local_pref = local_pref.min(65535);
    let component_id = component_id.clamp(1, 256);

    (type_pref << 24) | (local_pref << 8) | (256 - component_id)
}

/// Calculate local preference based on address type
fn local_preference(addr: &SocketAddr) -> u32 {
    match addr.ip() {
        IpAddr::V4(_) => IPV4_LOCAL_PREF,
        IpAddr::V6(_) => IPV6_LOCAL_PREF,
    }
}

/// Generate foundation string for candidate
///
/// Foundation is used to determine which candidates can be paired.
/// Same foundation = same base address and type.
fn generate_foundation(candidate_type: CandidateType, addr: &SocketAddr) -> String {
    // Simple foundation: type + IP (port doesn't affect foundation)
    format!("{}_{}", candidate_type, addr.ip())
}

// ============================================================================
// Candidate Gathering
// ============================================================================

/// Gather host candidates from provided local addresses
///
/// This function accepts a list of local addresses (from Swift NetworkExtension)
/// and creates host candidates, filtering out loopback addresses.
///
/// # Arguments
/// * `local_addrs` - List of local addresses from interface enumeration
/// * `include_loopback` - Whether to include loopback addresses (for testing)
pub fn gather_host_candidates(local_addrs: &[SocketAddr], include_loopback: bool) -> Vec<Candidate> {
    local_addrs
        .iter()
        .filter(|addr| include_loopback || !addr.ip().is_loopback())
        .map(|&addr| Candidate::host(addr))
        .collect()
}

/// Gather a server reflexive candidate from QAD response
///
/// # Arguments
/// * `reflexive_addr` - Public address from QAD OBSERVED_ADDRESS
/// * `base_addr` - Local address used to contact the server
pub fn gather_reflexive_candidate(
    reflexive_addr: SocketAddr,
    base_addr: SocketAddr,
) -> Option<Candidate> {
    // Skip if reflexive address matches base (no NAT present)
    if reflexive_addr.ip() == base_addr.ip() {
        return None;
    }
    Some(Candidate::server_reflexive(reflexive_addr, base_addr))
}

/// Create a relay candidate from the Intermediate server address
///
/// # Arguments
/// * `intermediate_addr` - Address of the Intermediate server
/// * `base_addr` - Local address used to connect to Intermediate
pub fn gather_relay_candidate(
    intermediate_addr: SocketAddr,
    base_addr: SocketAddr,
) -> Candidate {
    Candidate::relay(intermediate_addr, base_addr)
}

/// Sort candidates by priority (highest first)
pub fn sort_candidates_by_priority(candidates: &mut [Candidate]) {
    candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
}

// ============================================================================
// Platform-specific Interface Enumeration
// ============================================================================

/// Enumerate local network interface addresses using libc
///
/// Returns IPv4 addresses from non-loopback interfaces.
/// This is a fallback when Swift doesn't provide addresses.
#[cfg(unix)]
pub fn enumerate_local_addresses(port: u16) -> Vec<SocketAddr> {
    let mut addrs = Vec::new();

    unsafe {
        let mut ifaddrs: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifaddrs) != 0 {
            return addrs;
        }

        let mut current = ifaddrs;
        while !current.is_null() {
            let ifa = &*current;

            // Only process AF_INET (IPv4) for now
            if !ifa.ifa_addr.is_null() {
                let family = (*ifa.ifa_addr).sa_family as i32;
                if family == libc::AF_INET {
                    let sockaddr_in = ifa.ifa_addr as *const libc::sockaddr_in;
                    let ip_bytes = (*sockaddr_in).sin_addr.s_addr.to_ne_bytes();
                    let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);

                    // Skip loopback
                    if !ip.is_loopback() {
                        addrs.push(SocketAddr::new(IpAddr::V4(ip), port));
                    }
                }
            }

            current = ifa.ifa_next;
        }

        libc::freeifaddrs(ifaddrs);
    }

    addrs
}

#[cfg(not(unix))]
pub fn enumerate_local_addresses(_port: u16) -> Vec<SocketAddr> {
    // Non-unix platforms: return empty, expect Swift to provide addresses
    Vec::new()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_type_preference() {
        assert_eq!(CandidateType::Host.type_preference(), 126);
        assert_eq!(CandidateType::ServerReflexive.type_preference(), 100);
        assert_eq!(CandidateType::PeerReflexive.type_preference(), 110);
        assert_eq!(CandidateType::Relay.type_preference(), 0);
    }

    #[test]
    fn test_calculate_priority() {
        // Host candidate with max local pref
        let host_priority = calculate_priority(126, 65535, 1);
        assert_eq!(host_priority, (126 << 24) | (65535 << 8) | 255);

        // Relay candidate (lowest type pref)
        let relay_priority = calculate_priority(0, 65535, 1);
        assert_eq!(relay_priority, (0 << 24) | (65535 << 8) | 255);

        // Host > srflx > prflx > relay
        let srflx_priority = calculate_priority(100, 65535, 1);
        let prflx_priority = calculate_priority(110, 65535, 1);

        assert!(host_priority > prflx_priority);
        assert!(prflx_priority > srflx_priority);
        assert!(srflx_priority > relay_priority);
    }

    #[test]
    fn test_host_candidate_creation() {
        let addr: SocketAddr = "192.168.1.100:50000".parse().unwrap();
        let candidate = Candidate::host(addr);

        assert_eq!(candidate.candidate_type, CandidateType::Host);
        assert_eq!(candidate.address, addr);
        assert!(candidate.related_address.is_none());
        assert!(candidate.priority > 0);
        assert!(candidate.foundation.contains("host"));
    }

    #[test]
    fn test_srflx_candidate_creation() {
        let public_addr: SocketAddr = "203.0.113.50:50000".parse().unwrap();
        let base_addr: SocketAddr = "192.168.1.100:50000".parse().unwrap();
        let candidate = Candidate::server_reflexive(public_addr, base_addr);

        assert_eq!(candidate.candidate_type, CandidateType::ServerReflexive);
        assert_eq!(candidate.address, public_addr);
        assert_eq!(candidate.related_address, Some(base_addr));
    }

    #[test]
    fn test_relay_candidate_creation() {
        let relay_addr: SocketAddr = "10.0.0.1:4433".parse().unwrap();
        let base_addr: SocketAddr = "192.168.1.100:50000".parse().unwrap();
        let candidate = Candidate::relay(relay_addr, base_addr);

        assert_eq!(candidate.candidate_type, CandidateType::Relay);
        assert_eq!(candidate.address, relay_addr);
        assert_eq!(candidate.related_address, Some(base_addr));
    }

    #[test]
    fn test_gather_host_candidates() {
        let addrs: Vec<SocketAddr> = vec![
            "192.168.1.100:50000".parse().unwrap(),
            "127.0.0.1:50000".parse().unwrap(),
            "10.0.0.5:50000".parse().unwrap(),
        ];

        // Without loopback
        let candidates = gather_host_candidates(&addrs, false);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|c| !c.is_loopback()));

        // With loopback (for testing)
        let candidates_with_lo = gather_host_candidates(&addrs, true);
        assert_eq!(candidates_with_lo.len(), 3);
    }

    #[test]
    fn test_gather_reflexive_candidate() {
        let reflexive: SocketAddr = "203.0.113.50:50000".parse().unwrap();
        let base: SocketAddr = "192.168.1.100:50000".parse().unwrap();

        // Different IPs -> candidate created
        let candidate = gather_reflexive_candidate(reflexive, base);
        assert!(candidate.is_some());

        // Same IP (no NAT) -> no candidate
        let same_ip: SocketAddr = "192.168.1.100:51000".parse().unwrap();
        let no_candidate = gather_reflexive_candidate(same_ip, base);
        assert!(no_candidate.is_none());
    }

    #[test]
    fn test_sort_candidates_by_priority() {
        let mut candidates = vec![
            Candidate::relay("10.0.0.1:4433".parse().unwrap(), "192.168.1.1:5000".parse().unwrap()),
            Candidate::host("192.168.1.100:50000".parse().unwrap()),
            Candidate::server_reflexive(
                "203.0.113.50:50000".parse().unwrap(),
                "192.168.1.100:50000".parse().unwrap(),
            ),
        ];

        sort_candidates_by_priority(&mut candidates);

        // Should be: Host > ServerReflexive > Relay
        assert_eq!(candidates[0].candidate_type, CandidateType::Host);
        assert_eq!(candidates[1].candidate_type, CandidateType::ServerReflexive);
        assert_eq!(candidates[2].candidate_type, CandidateType::Relay);
    }

    #[test]
    fn test_candidate_display() {
        let candidate = Candidate::host("192.168.1.100:50000".parse().unwrap());
        let display = format!("{}", candidate);
        assert!(display.contains("host"));
        assert!(display.contains("192.168.1.100:50000"));
        assert!(display.contains("priority"));
    }

    #[test]
    fn test_is_loopback() {
        let loopback = Candidate::host("127.0.0.1:5000".parse().unwrap());
        let normal = Candidate::host("192.168.1.1:5000".parse().unwrap());

        assert!(loopback.is_loopback());
        assert!(!normal.is_loopback());
    }

    #[test]
    fn test_enumerate_local_addresses() {
        // This test may return empty on some systems, which is fine
        let addrs = enumerate_local_addresses(50000);
        // Just verify it doesn't panic and returns valid addresses
        for addr in &addrs {
            assert!(!addr.ip().is_loopback());
            assert_eq!(addr.port(), 50000);
        }
    }
}
