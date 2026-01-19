//! QUIC Test Client
//!
//! A simple QUIC client for E2E relay testing.
//! Connects to the Intermediate Server and sends/receives DATAGRAMs.
//!
//! Usage:
//!   quic-test-client --server 127.0.0.1:4433 --send "Hello World"
//!   quic-test-client --server 127.0.0.1:4433 --send-hex "48454c4c4f"
//!   quic-test-client --server 127.0.0.1:4433 --send-udp "Hello" --dst 10.0.0.1:9999
//!   quic-test-client --server 127.0.0.1:4433 --interactive

use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::rand::{SecureRandom, SystemRandom};

// ============================================================================
// Constants (MUST match Intermediate Server and App Connector)
// ============================================================================

/// Maximum UDP payload size for QUIC packets
const MAX_DATAGRAM_SIZE: usize = 1350;

/// QUIC idle timeout in milliseconds
const IDLE_TIMEOUT_MS: u64 = 30_000;

/// ALPN protocol identifier (CRITICAL: must match server)
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// mio token for QUIC socket
const QUIC_SOCKET: Token = Token(0);

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let server_addr = parse_arg(&args, "--server")
        .unwrap_or_else(|| "127.0.0.1:4433".to_string());
    let send_data = parse_arg(&args, "--send");
    let send_hex = parse_arg(&args, "--send-hex");
    let send_udp = parse_arg(&args, "--send-udp");
    let dst_addr = parse_arg(&args, "--dst");
    let src_addr = parse_arg(&args, "--src");
    let service_id = parse_arg(&args, "--service");
    let interactive = args.iter().any(|a| a == "--interactive");
    let wait_ms: u64 = parse_arg(&args, "--wait")
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    // Phase 2: Protocol validation options
    let alpn_override = parse_arg(&args, "--alpn");
    let payload_size: Option<usize> = parse_arg(&args, "--payload-size")
        .and_then(|s| s.parse().ok());
    let expect_connect_fail = args.iter().any(|a| a == "--expect-fail");

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return Ok(());
    }

    let server_addr: SocketAddr = server_addr.parse()
        .map_err(|_| "Invalid server address")?;

    // Determine ALPN to use
    let alpn_bytes: Vec<u8> = alpn_override.as_ref()
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_else(|| ALPN_PROTOCOL.to_vec());
    let alpn_str = String::from_utf8_lossy(&alpn_bytes);

    log::info!("QUIC Test Client");
    log::info!("  Server: {}", server_addr);
    log::info!("  ALPN:   {:?}", alpn_str);
    if let Some(ref svc) = service_id {
        log::info!("  Service: {} (will register as Agent)", svc);
    }
    if expect_connect_fail {
        log::info!("  Mode: Expecting connection to FAIL (negative test)");
    }

    let mut client = QuicTestClient::new(server_addr, &alpn_bytes)?;

    // Connect and establish QUIC session
    client.connect()?;
    match client.wait_for_connection(Duration::from_secs(5)) {
        Ok(_) => {
            if expect_connect_fail {
                log::error!("Connection succeeded but expected failure!");
                std::process::exit(1);
            }
        }
        Err(e) => {
            if expect_connect_fail {
                log::info!("Connection failed as expected: {}", e);
                println!("EXPECTED_FAIL:connection_rejected");
                return Ok(());
            }
            return Err(e);
        }
    }

    // Register as Agent if service specified
    if let Some(ref svc) = service_id {
        client.register_as_agent(svc)?;
        // Brief wait for registration to propagate
        client.wait_for_responses(Duration::from_millis(200))?;
    }

    if interactive {
        // Interactive mode: read from stdin
        client.run_interactive()?;
    } else if let Some(size) = payload_size {
        // Generate payload of specified size (for boundary testing)
        let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        log::info!("Generated payload: {} bytes", payload.len());

        // If dst specified, wrap in IP/UDP
        if let Some(ref dst_str) = dst_addr {
            let dst: SocketAddrV4 = dst_str.parse()
                .map_err(|_| "Invalid --dst address (expected ip:port)")?;
            let src: SocketAddrV4 = src_addr
                .unwrap_or_else(|| "10.0.0.100:12345".to_string())
                .parse()
                .map_err(|_| "Invalid --src address (expected ip:port)")?;
            let packet = build_ip_udp_packet(src, dst, &payload);
            log::info!("Built IP/UDP packet: {} bytes total", packet.len());
            client.send_datagram(&packet)?;
        } else {
            // Send raw payload
            client.send_datagram(&payload)?;
        }
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    } else if let Some(data) = send_udp {
        // Send data wrapped in IP/UDP packet (for relay testing)
        let dst: SocketAddrV4 = dst_addr
            .ok_or("--send-udp requires --dst address")?
            .parse()
            .map_err(|_| "Invalid --dst address (expected ip:port)")?;
        let src: SocketAddrV4 = src_addr
            .unwrap_or_else(|| "10.0.0.100:12345".to_string())
            .parse()
            .map_err(|_| "Invalid --src address (expected ip:port)")?;

        let packet = build_ip_udp_packet(src, dst, data.as_bytes());
        log::info!("Built IP/UDP packet: {} bytes (payload: {} bytes)", packet.len(), data.len());
        log::debug!("  Src: {}, Dst: {}", src, dst);
        client.send_datagram(&packet)?;
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    } else if let Some(data) = send_data {
        // Send string data
        client.send_datagram(data.as_bytes())?;
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    } else if let Some(hex) = send_hex {
        // Send hex data
        let bytes = hex_decode(&hex)?;
        client.send_datagram(&bytes)?;
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    } else {
        // Just connect and report
        log::info!("Connected. Use --send, --send-hex, --send-udp, or --interactive to send data.");
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    }

    Ok(())
}

fn print_usage() {
    eprintln!("QUIC Test Client for E2E Relay Testing");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  quic-test-client [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --server ADDR      Intermediate server address (default: 127.0.0.1:4433)");
    eprintln!("  --service ID       Register as Agent for this service (required for relay)");
    eprintln!("  --send TEXT        Send text data as raw DATAGRAM");
    eprintln!("  --send-hex HEX     Send hex-encoded data as raw DATAGRAM");
    eprintln!("  --send-udp TEXT    Send text wrapped in IP/UDP packet (for full E2E relay)");
    eprintln!("  --dst IP:PORT      Destination address for --send-udp (required with --send-udp)");
    eprintln!("  --src IP:PORT      Source address for --send-udp (default: 10.0.0.100:12345)");
    eprintln!("  --interactive      Interactive mode (read lines from stdin)");
    eprintln!("  --wait MS          Wait time for responses (default: 2000)");
    eprintln!();
    eprintln!("Protocol Validation (Phase 2):");
    eprintln!("  --alpn PROTO       Override ALPN protocol (default: ztna-v1)");
    eprintln!("  --payload-size N   Generate N-byte payload for boundary tests");
    eprintln!("  --expect-fail      Expect connection to fail (negative test)");
    eprintln!();
    eprintln!("  -h, --help         Show this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  # Full E2E relay test: send IP/UDP packet through relay to echo server");
    eprintln!("  quic-test-client --service test-service --send-udp 'Hello' --dst 127.0.0.1:9999");
    eprintln!();
    eprintln!("  # Test ALPN validation (negative test - expect failure)");
    eprintln!("  quic-test-client --alpn 'wrong-protocol' --expect-fail");
    eprintln!();
    eprintln!("  # Test MAX_DATAGRAM_SIZE boundary (1350 bytes)");
    eprintln!("  quic-test-client --service test-service --payload-size 1322 --dst 127.0.0.1:9999");
    eprintln!();
    eprintln!("  # Just connect (no relay, receives QAD only)");
    eprintln!("  quic-test-client --server 127.0.0.1:4433");
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("Hex string must have even length".into());
    }

    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| e.into())
}

/// Build a valid IPv4/UDP packet with the given payload
/// This is needed for E2E relay testing because the App Connector
/// expects IP packets (not raw data) to forward to the destination.
fn build_ip_udp_packet(src: SocketAddrV4, dst: SocketAddrV4, payload: &[u8]) -> Vec<u8> {
    let ip_header_len = 20u16;
    let udp_header_len = 8u16;
    let total_len = ip_header_len + udp_header_len + payload.len() as u16;
    let udp_len = udp_header_len + payload.len() as u16;

    let mut packet = Vec::with_capacity(total_len as usize);

    // === IPv4 Header (20 bytes) ===
    // Version (4) + IHL (5 = 20 bytes / 4)
    packet.push(0x45);
    // DSCP + ECN (TOS)
    packet.push(0x00);
    // Total length
    packet.extend_from_slice(&total_len.to_be_bytes());
    // Identification
    packet.extend_from_slice(&0u16.to_be_bytes());
    // Flags (DF=1) + Fragment offset
    packet.extend_from_slice(&0x4000u16.to_be_bytes());
    // TTL
    packet.push(64);
    // Protocol (UDP = 17)
    packet.push(17);
    // Header checksum (placeholder, computed below)
    let checksum_pos = packet.len();
    packet.extend_from_slice(&0u16.to_be_bytes());
    // Source IP
    packet.extend_from_slice(&src.ip().octets());
    // Destination IP
    packet.extend_from_slice(&dst.ip().octets());

    // Compute IP header checksum
    let checksum = ip_checksum(&packet[..20]);
    packet[checksum_pos] = (checksum >> 8) as u8;
    packet[checksum_pos + 1] = (checksum & 0xff) as u8;

    // === UDP Header (8 bytes) ===
    // Source port
    packet.extend_from_slice(&src.port().to_be_bytes());
    // Destination port
    packet.extend_from_slice(&dst.port().to_be_bytes());
    // UDP Length
    packet.extend_from_slice(&udp_len.to_be_bytes());
    // UDP Checksum (0 = disabled, valid for IPv4)
    packet.extend_from_slice(&0u16.to_be_bytes());

    // === Payload ===
    packet.extend_from_slice(payload);

    packet
}

/// Calculate IPv4 header checksum (RFC 1071)
fn ip_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Sum all 16-bit words
    for i in (0..header.len()).step_by(2) {
        let word = if i + 1 < header.len() {
            ((header[i] as u32) << 8) | (header[i + 1] as u32)
        } else {
            (header[i] as u32) << 8
        };
        sum += word;
    }

    // Fold 32-bit sum to 16 bits
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    // One's complement
    !sum as u16
}

// ============================================================================
// QUIC Test Client
// ============================================================================

struct QuicTestClient {
    poll: Poll,
    socket: UdpSocket,
    conn: Option<quiche::Connection>,
    config: quiche::Config,
    server_addr: SocketAddr,
    rng: SystemRandom,
    recv_buf: Vec<u8>,
    send_buf: Vec<u8>,
}

impl QuicTestClient {
    fn new(server_addr: SocketAddr, alpn: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        // Create quiche client configuration
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        // Set ALPN (allows override for testing)
        config.set_application_protos(&[alpn])?;

        // Enable DATAGRAM support
        config.enable_dgram(true, 1000, 1000);

        // Set timeouts and limits
        config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        // Disable certificate verification (for testing with self-signed certs)
        config.verify_peer(false);

        // Create poll and socket
        let poll = Poll::new()?;
        let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
        let mut socket = UdpSocket::bind(local_addr)?;

        poll.registry()
            .register(&mut socket, QUIC_SOCKET, Interest::READABLE)?;

        log::info!("Bound to {}", socket.local_addr()?);

        Ok(QuicTestClient {
            poll,
            socket,
            conn: None,
            config,
            server_addr,
            rng: SystemRandom::new(),
            recv_buf: vec![0u8; 65535],
            send_buf: vec![0u8; MAX_DATAGRAM_SIZE],
        })
    }

    fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Generate connection ID
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        self.rng
            .fill(&mut scid)
            .map_err(|_| "Failed to generate connection ID")?;
        let scid = quiche::ConnectionId::from_ref(&scid);

        // Create connection
        let local_addr = self.socket.local_addr()?;
        let conn = quiche::connect(
            None,
            &scid,
            local_addr,
            self.server_addr,
            &mut self.config,
        )?;

        log::info!("Connecting to {} ...", self.server_addr);
        self.conn = Some(conn);

        // Send initial packet
        self.flush()?;

        Ok(())
    }

    fn wait_for_connection(&mut self, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let start = Instant::now();
        let mut events = Events::with_capacity(64);

        while start.elapsed() < timeout {
            let poll_timeout = self.conn.as_ref()
                .and_then(|c| c.timeout())
                .or(Some(Duration::from_millis(100)));

            self.poll.poll(&mut events, poll_timeout)?;

            // Process incoming
            self.process_socket()?;

            // Process timeouts
            if let Some(ref mut conn) = self.conn {
                conn.on_timeout();
            }

            // Send pending
            self.flush()?;

            // Check if established
            if let Some(ref conn) = self.conn {
                if conn.is_established() {
                    log::info!("Connection established!");
                    return Ok(());
                }
                if conn.is_closed() {
                    return Err("Connection closed during handshake".into());
                }
            }
        }

        Err("Connection timeout".into())
    }

    fn send_datagram(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.conn {
            if !conn.is_established() {
                return Err("Connection not established".into());
            }

            log::info!("Sending DATAGRAM: {} bytes", data.len());
            log::debug!("  Data: {:?}", data);

            match conn.dgram_send(data) {
                Ok(_) => {
                    log::info!("DATAGRAM queued");
                }
                Err(e) => {
                    log::error!("Failed to queue DATAGRAM: {:?}", e);
                    return Err(e.into());
                }
            }

            self.flush()?;
        }

        Ok(())
    }

    /// Register as an Agent targeting a specific service
    /// Protocol: [0x10][len][service_id]
    fn register_as_agent(&mut self, service_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let id_bytes = service_id.as_bytes();
        if id_bytes.len() > 255 {
            return Err("Service ID too long".into());
        }

        let mut msg = Vec::with_capacity(2 + id_bytes.len());
        msg.push(0x10); // Agent registration marker
        msg.push(id_bytes.len() as u8);
        msg.extend_from_slice(id_bytes);

        log::info!("Registering as Agent for service: {}", service_id);
        self.send_datagram(&msg)?;

        Ok(())
    }

    fn wait_for_responses(&mut self, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let start = Instant::now();
        let mut events = Events::with_capacity(64);
        let mut received_any = false;

        log::info!("Waiting for responses ({} ms)...", timeout.as_millis());

        while start.elapsed() < timeout {
            let remaining = timeout.saturating_sub(start.elapsed());
            let poll_timeout = self.conn.as_ref()
                .and_then(|c| c.timeout())
                .map(|t| t.min(remaining))
                .or(Some(remaining.min(Duration::from_millis(100))));

            self.poll.poll(&mut events, poll_timeout)?;

            // Process incoming
            self.process_socket()?;

            // Check for DATAGRAMs
            if let Some(ref mut conn) = self.conn {
                let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
                while let Ok(len) = conn.dgram_recv(&mut buf) {
                    received_any = true;
                    log::info!("Received DATAGRAM: {} bytes", len);
                    log::info!("  Hex: {}", hex_encode(&buf[..len]));
                    if let Ok(s) = std::str::from_utf8(&buf[..len]) {
                        log::info!("  Text: {}", s);
                    }
                    // Print to stdout for test scripts
                    println!("RECV:{}", hex_encode(&buf[..len]));
                }

                conn.on_timeout();
            }

            self.flush()?;

            if let Some(ref conn) = self.conn {
                if conn.is_closed() {
                    log::warn!("Connection closed");
                    break;
                }
            }
        }

        if !received_any {
            log::info!("No DATAGRAMs received");
        }

        Ok(())
    }

    fn run_interactive(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Interactive mode. Type messages to send, 'quit' to exit.");

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("> ");
            stdout.flush()?;

            let mut line = String::new();
            if stdin.lock().read_line(&mut line)? == 0 {
                break; // EOF
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "quit" || line == "exit" {
                break;
            }

            // Send as DATAGRAM
            self.send_datagram(line.as_bytes())?;

            // Brief wait for response
            self.wait_for_responses(Duration::from_millis(500))?;
        }

        Ok(())
    }

    fn process_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let (len, from) = match self.socket.recv_from(&mut self.recv_buf) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };

            log::trace!("Received {} bytes from {}", len, from);

            if let Some(ref mut conn) = self.conn {
                let recv_info = quiche::RecvInfo {
                    from,
                    to: self.socket.local_addr()?,
                };

                match conn.recv(&mut self.recv_buf[..len], recv_info) {
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("QUIC recv error: {:?}", e);
                    }
                }
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.conn {
            loop {
                match conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        self.socket.send_to(&self.send_buf[..len], send_info.to)?;
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        log::debug!("QUIC send error: {:?}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode("48454c4c4f").unwrap(), b"HELLO");
        assert_eq!(hex_decode("00ff").unwrap(), vec![0x00, 0xff]);
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(b"HELLO"), "48454c4c4f");
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_DATAGRAM_SIZE, 1350);
        assert_eq!(ALPN_PROTOCOL, b"ztna-v1");
    }

    #[test]
    fn test_build_ip_udp_packet() {
        let src: SocketAddrV4 = "10.0.0.100:12345".parse().unwrap();
        let dst: SocketAddrV4 = "127.0.0.1:9999".parse().unwrap();
        let payload = b"Hello";

        let packet = build_ip_udp_packet(src, dst, payload);

        // Total size: IP header (20) + UDP header (8) + payload (5)
        assert_eq!(packet.len(), 33);

        // IP version and IHL
        assert_eq!(packet[0], 0x45);

        // IP protocol (UDP = 17)
        assert_eq!(packet[9], 17);

        // Source IP (10.0.0.100)
        assert_eq!(&packet[12..16], &[10, 0, 0, 100]);

        // Dest IP (127.0.0.1)
        assert_eq!(&packet[16..20], &[127, 0, 0, 1]);

        // UDP source port (12345 = 0x3039)
        assert_eq!(&packet[20..22], &[0x30, 0x39]);

        // UDP dest port (9999 = 0x270F)
        assert_eq!(&packet[22..24], &[0x27, 0x0F]);

        // Payload
        assert_eq!(&packet[28..], b"Hello");
    }

    #[test]
    fn test_ip_checksum() {
        // Simple test with known header
        let header = [
            0x45, 0x00, 0x00, 0x21, // Version, IHL, TOS, Total Length
            0x00, 0x00, 0x40, 0x00, // ID, Flags, Fragment
            0x40, 0x11, 0x00, 0x00, // TTL, Protocol, Checksum (0)
            0x0a, 0x00, 0x00, 0x64, // Src IP (10.0.0.100)
            0x7f, 0x00, 0x00, 0x01, // Dst IP (127.0.0.1)
        ];
        let checksum = ip_checksum(&header);
        // Checksum should be non-zero (the return type is u16 so it always fits)
        assert!(checksum != 0);
    }
}
