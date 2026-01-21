//! QAD (QUIC Address Discovery) message handling for App Connector
//!
//! Parses and builds OBSERVED_ADDRESS messages for QUIC Address Discovery.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// QAD message type for OBSERVED_ADDRESS
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

/// Build an OBSERVED_ADDRESS QAD message
///
/// Format: [0x01, IPv4(4 bytes), port(2 bytes BE)]
/// Total: 7 bytes for IPv4
pub fn build_observed_address(addr: SocketAddr) -> Vec<u8> {
    let mut msg = vec![QAD_OBSERVED_ADDRESS];

    match addr {
        SocketAddr::V4(v4) => {
            msg.extend_from_slice(&v4.ip().octets());
            msg.extend_from_slice(&v4.port().to_be_bytes());
        }
        SocketAddr::V6(v6) => {
            // For IPv6, map to IPv4 if possible, otherwise use 0.0.0.0
            // This is a limitation - full IPv6 support would need protocol extension
            let ip = v6.ip().to_ipv4_mapped().unwrap_or(Ipv4Addr::UNSPECIFIED);
            msg.extend_from_slice(&ip.octets());
            msg.extend_from_slice(&v6.port().to_be_bytes());
        }
    }

    msg
}

/// Parse an OBSERVED_ADDRESS QAD message
///
/// Format: [0x01, IPv4(4 bytes), port(2 bytes BE)]
/// Total: 7 bytes for IPv4
pub fn parse_observed_address(data: &[u8]) -> Option<SocketAddr> {
    if data.len() < 7 {
        return None;
    }

    if data[0] != QAD_OBSERVED_ADDRESS {
        return None;
    }

    let ip = Ipv4Addr::new(data[1], data[2], data[3], data[4]);
    let port = u16::from_be_bytes([data[5], data[6]]);

    Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_observed_address() {
        let addr: SocketAddr = "192.168.1.100:12345".parse().unwrap();
        let msg = build_observed_address(addr);

        assert_eq!(msg.len(), 7);
        assert_eq!(msg[0], 0x01); // QAD type
        assert_eq!(msg[1..5], [192, 168, 1, 100]); // IP
        assert_eq!(msg[5..7], [0x30, 0x39]); // Port 12345 in BE
    }

    #[test]
    fn test_build_parse_roundtrip() {
        let original: SocketAddr = "10.0.0.1:8080".parse().unwrap();
        let msg = build_observed_address(original);
        let parsed = parse_observed_address(&msg).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_parse_observed_address() {
        // QAD message: 0x01 + 192.168.1.100 + port 12345
        let msg = [0x01, 192, 168, 1, 100, 0x30, 0x39]; // 0x3039 = 12345

        let addr = parse_observed_address(&msg).unwrap();
        assert_eq!(addr.to_string(), "192.168.1.100:12345");
    }

    #[test]
    fn test_parse_observed_address_too_short() {
        let msg = [0x01, 192, 168, 1, 100]; // Missing port
        assert!(parse_observed_address(&msg).is_none());
    }

    #[test]
    fn test_parse_observed_address_wrong_type() {
        let msg = [0x02, 192, 168, 1, 100, 0x30, 0x39]; // Wrong type
        assert!(parse_observed_address(&msg).is_none());
    }
}
