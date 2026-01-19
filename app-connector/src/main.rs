//! ZTNA App Connector
//!
//! A QUIC client that:
//! - Connects to the Intermediate System
//! - Registers as a Connector for a specific service
//! - Receives DATAGRAMs containing encapsulated IP packets
//! - Forwards UDP payloads to a local service
//! - Handles return traffic back through the tunnel

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::rand::{SecureRandom, SystemRandom};

mod qad;

// ============================================================================
// Constants (MUST match Intermediate Server)
// ============================================================================

/// Maximum UDP payload size for QUIC packets (must match Intermediate Server)
const MAX_DATAGRAM_SIZE: usize = 1350;

/// QUIC idle timeout in milliseconds (must match Intermediate Server)
const IDLE_TIMEOUT_MS: u64 = 30_000;

/// ALPN protocol identifier (CRITICAL: must match Intermediate Server)
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// Default Intermediate Server port
const DEFAULT_SERVER_PORT: u16 = 4433;

/// Default local forward port (for testing)
const DEFAULT_FORWARD_PORT: u16 = 8080;

/// Registration message type for Connector
const REG_TYPE_CONNECTOR: u8 = 0x11;

/// QAD message type (OBSERVED_ADDRESS)
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

/// mio token for QUIC socket
const QUIC_SOCKET_TOKEN: Token = Token(0);

/// mio token for local forwarding socket
const LOCAL_SOCKET_TOKEN: Token = Token(1);

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    // --server <addr:port>  Intermediate Server address
    // --service <id>        Service ID to register
    // --forward <addr:port> Local address to forward traffic to

    let server_addr = parse_arg(&args, "--server")
        .unwrap_or_else(|| format!("127.0.0.1:{}", DEFAULT_SERVER_PORT));
    let service_id = parse_arg(&args, "--service")
        .unwrap_or_else(|| "default".to_string());
    let forward_addr = parse_arg(&args, "--forward")
        .unwrap_or_else(|| format!("127.0.0.1:{}", DEFAULT_FORWARD_PORT));

    let server_addr: SocketAddr = server_addr.parse()
        .map_err(|_| "Invalid server address")?;
    let forward_addr: SocketAddr = forward_addr.parse()
        .map_err(|_| "Invalid forward address")?;

    log::info!("ZTNA App Connector starting...");
    log::info!("  Server:  {}", server_addr);
    log::info!("  Service: {}", service_id);
    log::info!("  Forward: {}", forward_addr);
    log::info!("  ALPN:    {:?}", std::str::from_utf8(ALPN_PROTOCOL));

    // Create connector and run
    let mut connector = Connector::new(server_addr, service_id, forward_addr)?;
    connector.run()
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

// ============================================================================
// Connector Structure
// ============================================================================

struct Connector {
    /// mio poll instance
    poll: Poll,
    /// UDP socket for QUIC communication
    quic_socket: UdpSocket,
    /// Local UDP socket for forwarding (registered with mio poll)
    local_socket: UdpSocket,
    /// QUIC connection
    conn: Option<quiche::Connection>,
    /// quiche configuration
    config: quiche::Config,
    /// Server address
    server_addr: SocketAddr,
    /// Service ID for registration
    service_id: String,
    /// Forward address for local traffic
    forward_addr: SocketAddr,
    /// Random number generator
    rng: SystemRandom,
    /// Receive buffer
    recv_buf: Vec<u8>,
    /// Send buffer
    send_buf: Vec<u8>,
    /// Whether registration has been sent
    registered: bool,
    /// Observed public address from QAD
    observed_addr: Option<SocketAddr>,
    /// Mapping from local response source to original agent request
    /// Key: (src_ip, src_port, dst_port) from encapsulated packet
    /// Value: timestamp for cleanup
    flow_map: HashMap<(Ipv4Addr, u16, u16), Instant>,
}

impl Connector {
    fn new(
        server_addr: SocketAddr,
        service_id: String,
        forward_addr: SocketAddr,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create quiche client configuration
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        // CRITICAL: ALPN must match Intermediate Server
        config.set_application_protos(&[ALPN_PROTOCOL])?;

        // Enable DATAGRAM support (for IP tunneling)
        config.enable_dgram(true, 1000, 1000);

        // Set timeouts and limits (match Intermediate Server)
        config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        // Disable server certificate verification (for MVP with self-signed certs)
        config.verify_peer(false);

        // Create mio poll
        let poll = Poll::new()?;

        // Create UDP socket for QUIC (bind to any port)
        let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
        let mut quic_socket = UdpSocket::bind(local_addr)?;

        // Register QUIC socket with poll
        poll.registry()
            .register(&mut quic_socket, QUIC_SOCKET_TOKEN, Interest::READABLE)?;

        // Create local socket for forwarding and register with poll
        let mut local_socket = UdpSocket::bind("0.0.0.0:0".parse()?)?;
        poll.registry()
            .register(&mut local_socket, LOCAL_SOCKET_TOKEN, Interest::READABLE)?;

        log::info!("QUIC socket bound to {}", quic_socket.local_addr()?);
        log::info!("Local socket bound to {}", local_socket.local_addr()?);

        Ok(Connector {
            poll,
            quic_socket,
            local_socket,
            conn: None,
            config,
            server_addr,
            service_id,
            forward_addr,
            rng: SystemRandom::new(),
            recv_buf: vec![0u8; 65535],
            send_buf: vec![0u8; MAX_DATAGRAM_SIZE],
            registered: false,
            observed_addr: None,
            flow_map: HashMap::new(),
        })
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initiate QUIC connection
        self.connect()?;

        // Send initial QUIC handshake packet immediately
        self.send_pending()?;

        let mut events = Events::with_capacity(1024);

        loop {
            // Calculate timeout based on connection timeout
            // Use a reasonable minimum to ensure we check local socket frequently
            let timeout = self.conn.as_ref()
                .and_then(|c| c.timeout())
                .map(|t| t.min(Duration::from_millis(100)))
                .or(Some(Duration::from_millis(100)));

            // Poll for events
            self.poll.poll(&mut events, timeout)?;

            // Process events
            for event in events.iter() {
                match event.token() {
                    QUIC_SOCKET_TOKEN => {
                        self.process_quic_socket()?;
                    }
                    LOCAL_SOCKET_TOKEN => {
                        self.process_local_socket()?;
                    }
                    _ => {}
                }
            }

            // Also check local socket even without events (for edge cases)
            self.process_local_socket()?;

            // Process timeouts
            self.process_timeouts();

            // Send pending packets
            self.send_pending()?;

            // Check if we need to register
            self.maybe_register()?;

            // Check connection state
            if let Some(ref conn) = self.conn {
                if conn.is_closed() {
                    log::warn!("Connection closed");
                    // Reconnect logic could go here
                    break;
                }
            }
        }

        Ok(())
    }

    fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Generate connection ID
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        self.rng
            .fill(&mut scid)
            .map_err(|_| "Failed to generate connection ID")?;
        let scid = quiche::ConnectionId::from_ref(&scid);

        // Create QUIC connection
        let local_addr = self.quic_socket.local_addr()?;
        let conn = quiche::connect(
            None, // No server name for now
            &scid,
            local_addr,
            self.server_addr,
            &mut self.config,
        )?;

        log::info!("Connecting to {} (scid={:?})", self.server_addr, scid);

        self.conn = Some(conn);
        self.registered = false;

        Ok(())
    }

    fn process_quic_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // Receive UDP packet
            let (len, from) = match self.quic_socket.recv_from(&mut self.recv_buf) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };

            log::trace!("Received {} bytes from {}", len, from);

            // Process QUIC packet
            if let Some(ref mut conn) = self.conn {
                let recv_info = quiche::RecvInfo {
                    from,
                    to: self.quic_socket.local_addr()?,
                };

                match conn.recv(&mut self.recv_buf[..len], recv_info) {
                    Ok(_) => {
                        // Check for established connection
                        if conn.is_established() {
                            // Process incoming DATAGRAMs
                            self.process_datagrams()?;
                        }
                    }
                    Err(e) => {
                        log::debug!("Connection recv error: {:?}", e);
                    }
                }
            }
        }

        Ok(())
    }

    fn process_datagrams(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut dgrams = Vec::new();

        // Collect DATAGRAMs from connection
        if let Some(ref mut conn) = self.conn {
            let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
            while let Ok(len) = conn.dgram_recv(&mut buf) {
                dgrams.push(buf[..len].to_vec());
            }
        }

        // Process collected DATAGRAMs
        for dgram in dgrams {
            if dgram.is_empty() {
                continue;
            }

            match dgram[0] {
                QAD_OBSERVED_ADDRESS => {
                    // QAD message - parse observed address
                    self.handle_qad(&dgram)?;
                }
                _ => {
                    // Encapsulated IP packet - forward to local service
                    self.forward_to_local(&dgram)?;
                }
            }
        }

        Ok(())
    }

    fn handle_qad(&mut self, dgram: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(addr) = qad::parse_observed_address(dgram) {
            log::info!("QAD: Observed address is {}", addr);
            self.observed_addr = Some(addr);
        }
        Ok(())
    }

    fn forward_to_local(&mut self, dgram: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        // Parse IP header (minimum 20 bytes)
        if dgram.len() < 20 {
            log::debug!("Datagram too short for IP header: {} bytes", dgram.len());
            return Ok(());
        }

        let version = (dgram[0] >> 4) & 0x0F;
        if version != 4 {
            log::debug!("Non-IPv4 packet (version={}), dropping", version);
            return Ok(());
        }

        let ihl = (dgram[0] & 0x0F) as usize;
        let ip_header_len = ihl * 4;
        if dgram.len() < ip_header_len {
            log::debug!("IP header truncated");
            return Ok(());
        }

        let protocol = dgram[9];
        let src_ip = Ipv4Addr::new(dgram[12], dgram[13], dgram[14], dgram[15]);
        let _dst_ip = Ipv4Addr::new(dgram[16], dgram[17], dgram[18], dgram[19]);

        // For MVP, only handle UDP (protocol 17)
        if protocol != 17 {
            log::trace!("Non-UDP packet (protocol={}), dropping", protocol);
            return Ok(());
        }

        // Parse UDP header (8 bytes)
        if dgram.len() < ip_header_len + 8 {
            log::debug!("UDP header truncated");
            return Ok(());
        }

        let udp_header_start = ip_header_len;
        let src_port = u16::from_be_bytes([dgram[udp_header_start], dgram[udp_header_start + 1]]);
        let dst_port = u16::from_be_bytes([dgram[udp_header_start + 2], dgram[udp_header_start + 3]]);
        let udp_len = u16::from_be_bytes([dgram[udp_header_start + 4], dgram[udp_header_start + 5]]) as usize;

        // Extract UDP payload
        let payload_start = ip_header_len + 8;
        let payload_len = udp_len.saturating_sub(8);
        if dgram.len() < payload_start + payload_len {
            log::debug!("UDP payload truncated");
            return Ok(());
        }

        let payload = &dgram[payload_start..payload_start + payload_len];

        log::debug!(
            "Forwarding UDP: {}:{} -> {}:{} ({} bytes)",
            src_ip, src_port, self.forward_addr.ip(), self.forward_addr.port(), payload.len()
        );

        // Store flow mapping for return traffic
        self.flow_map.insert((src_ip, src_port, dst_port), Instant::now());

        // Forward payload to local service
        match self.local_socket.send_to(payload, self.forward_addr) {
            Ok(sent) => {
                log::trace!("Sent {} bytes to local service", sent);
            }
            Err(e) => {
                log::debug!("Failed to forward to local service: {:?}", e);
            }
        }

        Ok(())
    }

    fn process_local_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = vec![0u8; 65535];

        loop {
            match self.local_socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    log::trace!("Received {} bytes from local service at {}", len, from);

                    // For MVP, we need to re-encapsulate and send back
                    // Find the original flow to construct return packet
                    self.send_return_traffic(&buf[..len], from)?;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    log::debug!("Local socket error: {:?}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    fn send_return_traffic(&mut self, payload: &[u8], from: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        // Find matching flow (any flow for now - simplified MVP)
        // In a real implementation, we'd track the original src/dst properly

        let flow_key = self.flow_map.keys().next().cloned();

        if let Some((orig_src_ip, orig_src_port, _orig_dst_port)) = flow_key {
            // Build return IP/UDP packet
            // Source: the service we're proxying (forward_addr)
            // Destination: original source (agent)

            let packet = build_udp_packet(
                match from.ip() {
                    IpAddr::V4(ip) => ip,
                    _ => return Ok(()),
                },
                from.port(),
                orig_src_ip,
                orig_src_port,
                payload,
            );

            // Send via QUIC DATAGRAM
            if let Some(ref mut conn) = self.conn {
                match conn.dgram_send(&packet) {
                    Ok(_) => {
                        log::trace!(
                            "Sent return packet: {} bytes to agent ({}:{})",
                            packet.len(), orig_src_ip, orig_src_port
                        );
                    }
                    Err(e) => {
                        log::debug!("Failed to send return DATAGRAM: {:?}", e);
                    }
                }
            }
        } else {
            log::trace!("No flow mapping for return traffic from {}", from);
        }

        Ok(())
    }

    fn maybe_register(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.registered {
            return Ok(());
        }

        if let Some(ref mut conn) = self.conn {
            if !conn.is_established() {
                return Ok(());
            }

            // Build registration message: [0x11, id_len, service_id bytes]
            let id_bytes = self.service_id.as_bytes();
            let mut msg = Vec::with_capacity(2 + id_bytes.len());
            msg.push(REG_TYPE_CONNECTOR);
            msg.push(id_bytes.len() as u8);
            msg.extend_from_slice(id_bytes);

            match conn.dgram_send(&msg) {
                Ok(_) => {
                    log::info!("Registered as Connector for service '{}'", self.service_id);
                    self.registered = true;
                }
                Err(e) => {
                    log::debug!("Failed to send registration: {:?}", e);
                }
            }
        }

        Ok(())
    }

    fn process_timeouts(&mut self) {
        if let Some(ref mut conn) = self.conn {
            conn.on_timeout();
        }

        // Clean up old flow mappings (older than 60 seconds)
        let now = Instant::now();
        self.flow_map.retain(|_, ts| now.duration_since(*ts).as_secs() < 60);
    }

    fn send_pending(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.conn {
            loop {
                match conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        self.quic_socket.send_to(&self.send_buf[..len], send_info.to)?;
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        log::debug!("Send error: {:?}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

// ============================================================================
// Packet Building Helpers
// ============================================================================

fn build_udp_packet(
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_ip: Ipv4Addr,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let udp_len = 8 + payload.len();
    let total_len = 20 + udp_len;

    let mut packet = vec![0u8; total_len];

    // IP Header (20 bytes, no options)
    packet[0] = 0x45; // Version 4, IHL 5
    packet[1] = 0x00; // DSCP/ECN
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[4..6].copy_from_slice(&[0x00, 0x00]); // ID
    packet[6..8].copy_from_slice(&[0x40, 0x00]); // Flags (Don't Fragment) + Fragment Offset
    packet[8] = 64; // TTL
    packet[9] = 17; // Protocol (UDP)
    // packet[10..12] = checksum (leave as 0 for now)
    packet[12..16].copy_from_slice(&src_ip.octets());
    packet[16..20].copy_from_slice(&dst_ip.octets());

    // Calculate IP header checksum
    let checksum = ip_checksum(&packet[0..20]);
    packet[10..12].copy_from_slice(&checksum.to_be_bytes());

    // UDP Header (8 bytes)
    packet[20..22].copy_from_slice(&src_port.to_be_bytes());
    packet[22..24].copy_from_slice(&dst_port.to_be_bytes());
    packet[24..26].copy_from_slice(&(udp_len as u16).to_be_bytes());
    // packet[26..28] = checksum (leave as 0, optional for IPv4)

    // UDP Payload
    packet[28..].copy_from_slice(payload);

    packet
}

fn ip_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    for i in (0..header.len()).step_by(2) {
        let word = if i + 1 < header.len() {
            ((header[i] as u32) << 8) | (header[i + 1] as u32)
        } else {
            (header[i] as u32) << 8
        };
        sum = sum.wrapping_add(word);
    }

    // Fold 32-bit sum to 16 bits
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_checksum() {
        // Example IP header (without checksum)
        let mut header = [
            0x45, 0x00, 0x00, 0x3c, // Version, IHL, TOS, Total Length
            0x1c, 0x46, 0x40, 0x00, // ID, Flags, Fragment Offset
            0x40, 0x06, 0x00, 0x00, // TTL, Protocol, Checksum (zeroed)
            0xac, 0x10, 0x0a, 0x63, // Source IP
            0xac, 0x10, 0x0a, 0x0c, // Dest IP
        ];

        let checksum = ip_checksum(&header);
        header[10..12].copy_from_slice(&checksum.to_be_bytes());

        // Verify: checksum of header with checksum should be 0
        assert_eq!(ip_checksum(&header), 0);
    }

    #[test]
    fn test_build_udp_packet() {
        let packet = build_udp_packet(
            Ipv4Addr::new(192, 168, 1, 100),
            12345,
            Ipv4Addr::new(10, 0, 0, 1),
            80,
            b"Hello",
        );

        // Verify IP header
        assert_eq!(packet[0], 0x45); // IPv4, IHL=5
        assert_eq!(packet[9], 17);   // UDP protocol

        // Verify UDP header
        let src_port = u16::from_be_bytes([packet[20], packet[21]]);
        let dst_port = u16::from_be_bytes([packet[22], packet[23]]);
        assert_eq!(src_port, 12345);
        assert_eq!(dst_port, 80);

        // Verify payload
        assert_eq!(&packet[28..], b"Hello");
    }

    #[test]
    fn test_constants_match_intermediate() {
        // These must match intermediate-server/src/main.rs
        assert_eq!(MAX_DATAGRAM_SIZE, 1350);
        assert_eq!(IDLE_TIMEOUT_MS, 30_000);
        assert_eq!(ALPN_PROTOCOL, b"ztna-v1");
    }
}
