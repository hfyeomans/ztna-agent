//! QUIC Address Discovery (QAD) message handling
//!
//! QAD allows clients to discover their externally-observed address,
//! which is essential for NAT traversal and P2P hole punching.

use std::net::SocketAddr;

// ============================================================================
// QAD Message Types
// ============================================================================

/// QAD message type for OBSERVED_ADDRESS
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

// ============================================================================
// QAD Message Building
// ============================================================================

/// Build a QAD OBSERVED_ADDRESS message for the given socket address.
///
/// # Format (CRITICAL: must match Agent parser at lib.rs:255-262)
///
/// ```text
/// +--------+--------+--------+--------+--------+--------+--------+
/// | Type   | IPv4 Address (4 bytes)            | Port (2 bytes)  |
/// | (0x01) |                                   | (big-endian)    |
/// +--------+--------+--------+--------+--------+--------+--------+
///
/// Total: 7 bytes (IPv4 only)
/// ```
///
/// Returns `None` for IPv6 addresses (not yet supported).
pub fn build_observed_address(addr: SocketAddr) -> Option<Vec<u8>> {
    match addr {
        SocketAddr::V4(v4) => {
            let mut msg = Vec::with_capacity(7);

            // Type byte
            msg.push(QAD_OBSERVED_ADDRESS);

            // IPv4 address (4 bytes)
            msg.extend_from_slice(&v4.ip().octets());

            // Port (2 bytes, big-endian)
            msg.extend_from_slice(&v4.port().to_be_bytes());

            Some(msg)
        }
        SocketAddr::V6(_) => {
            log::warn!("IPv6 QAD not yet supported, skipping for {:?}", addr);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn test_build_observed_address() {
        // Test case from research.md: 203.0.113.5:54321
        // Expected: 01 CB 00 71 05 D4 31
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 5), 54321));

        let msg = build_observed_address(addr).expect("IPv4 should return Some");

        assert_eq!(msg.len(), 7);
        assert_eq!(msg[0], 0x01); // Type
        assert_eq!(msg[1], 203); // IP octet 1
        assert_eq!(msg[2], 0); // IP octet 2
        assert_eq!(msg[3], 113); // IP octet 3
        assert_eq!(msg[4], 5); // IP octet 4
        assert_eq!(msg[5], 0xD4); // Port high byte (54321 = 0xD431)
        assert_eq!(msg[6], 0x31); // Port low byte
    }

    #[test]
    fn test_localhost_address() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 4433));

        let msg = build_observed_address(addr).expect("IPv4 should return Some");

        assert_eq!(msg.len(), 7);
        assert_eq!(msg[0], 0x01);
        assert_eq!(msg[1..5], [127, 0, 0, 1]);
        // Port 4433 = 0x1151
        assert_eq!(msg[5], 0x11);
        assert_eq!(msg[6], 0x51);
    }

    #[test]
    fn test_ipv6_returns_none() {
        use std::net::{Ipv6Addr, SocketAddrV6};
        let addr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 4433, 0, 0));
        assert!(build_observed_address(addr).is_none());
    }
}
