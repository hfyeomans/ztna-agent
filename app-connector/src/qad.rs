//! QAD (QUIC Address Discovery) message handling for App Connector
//!
//! Parses OBSERVED_ADDRESS messages sent by the Intermediate Server.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// QAD message type for OBSERVED_ADDRESS
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

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
