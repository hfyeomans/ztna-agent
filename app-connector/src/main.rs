//! ZTNA App Connector
//!
//! A QUIC client/server that:
//! - Connects to the Intermediate System (client mode)
//! - Accepts P2P connections from Agents (server mode)
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
// P2P Client Connection
// ============================================================================

/// Represents an incoming P2P connection from an Agent
struct P2PClient {
    /// QUIC connection
    conn: quiche::Connection,
    /// Remote address of the Agent
    addr: SocketAddr,
    /// Whether QAD has been sent to this client
    qad_sent: bool,
}

impl P2PClient {
    fn new(conn: quiche::Connection, addr: SocketAddr) -> Self {
        P2PClient {
            conn,
            addr,
            qad_sent: false,
        }
    }
}

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
    // --p2p-cert <path>     TLS certificate for P2P server mode (optional)
    // --p2p-key <path>      TLS private key for P2P server mode (optional)

    let server_addr = parse_arg(&args, "--server")
        .unwrap_or_else(|| format!("127.0.0.1:{}", DEFAULT_SERVER_PORT));
    let service_id = parse_arg(&args, "--service")
        .unwrap_or_else(|| "default".to_string());
    let forward_addr = parse_arg(&args, "--forward")
        .unwrap_or_else(|| format!("127.0.0.1:{}", DEFAULT_FORWARD_PORT));
    let p2p_cert = parse_arg(&args, "--p2p-cert");
    let p2p_key = parse_arg(&args, "--p2p-key");

    let server_addr: SocketAddr = server_addr.parse()
        .map_err(|_| "Invalid server address")?;
    let forward_addr: SocketAddr = forward_addr.parse()
        .map_err(|_| "Invalid forward address")?;

    log::info!("ZTNA App Connector starting...");
    log::info!("  Server:  {}", server_addr);
    log::info!("  Service: {}", service_id);
    log::info!("  Forward: {}", forward_addr);
    log::info!("  ALPN:    {:?}", std::str::from_utf8(ALPN_PROTOCOL));
    log::info!("  P2P:     {}", if p2p_cert.is_some() { "enabled" } else { "disabled" });

    // Create connector and run
    let mut connector = Connector::new(
        server_addr,
        service_id,
        forward_addr,
        p2p_cert.as_deref(),
        p2p_key.as_deref(),
    )?;
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
    /// UDP socket for QUIC communication (shared for client and server)
    quic_socket: UdpSocket,
    /// Local UDP socket for forwarding (registered with mio poll)
    local_socket: UdpSocket,
    /// QUIC connection to Intermediate Server (client mode)
    intermediate_conn: Option<quiche::Connection>,
    /// P2P connections from Agents (server mode)
    p2p_clients: HashMap<quiche::ConnectionId<'static>, P2PClient>,
    /// quiche configuration for client mode (to Intermediate)
    client_config: quiche::Config,
    /// quiche configuration for P2P server mode (optional)
    server_config: Option<quiche::Config>,
    /// Intermediate Server address
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
    /// Whether registration has been sent to Intermediate
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
        p2p_cert_path: Option<&str>,
        p2p_key_path: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create quiche client configuration (for connecting to Intermediate)
        let mut client_config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        // CRITICAL: ALPN must match Intermediate Server
        client_config.set_application_protos(&[ALPN_PROTOCOL])?;

        // Enable DATAGRAM support (for IP tunneling)
        client_config.enable_dgram(true, 1000, 1000);

        // Set timeouts and limits (match Intermediate Server)
        client_config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        client_config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        client_config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        client_config.set_initial_max_data(10_000_000);
        client_config.set_initial_max_stream_data_bidi_local(1_000_000);
        client_config.set_initial_max_stream_data_bidi_remote(1_000_000);
        client_config.set_initial_max_streams_bidi(100);
        client_config.set_initial_max_streams_uni(100);

        // Disable server certificate verification (for MVP with self-signed certs)
        client_config.verify_peer(false);

        // Create server config if P2P certificates are provided
        let server_config = if let (Some(cert_path), Some(key_path)) = (p2p_cert_path, p2p_key_path) {
            let mut cfg = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

            // Load TLS certificates for server mode
            cfg.load_cert_chain_from_pem_file(cert_path)?;
            cfg.load_priv_key_from_pem_file(key_path)?;

            // Same settings as client config
            cfg.set_application_protos(&[ALPN_PROTOCOL])?;
            cfg.enable_dgram(true, 1000, 1000);
            cfg.set_max_idle_timeout(IDLE_TIMEOUT_MS);
            cfg.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
            cfg.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
            cfg.set_initial_max_data(10_000_000);
            cfg.set_initial_max_stream_data_bidi_local(1_000_000);
            cfg.set_initial_max_stream_data_bidi_remote(1_000_000);
            cfg.set_initial_max_streams_bidi(100);
            cfg.set_initial_max_streams_uni(100);

            // Disable client certificate verification (for MVP)
            cfg.verify_peer(false);

            log::info!("P2P server mode enabled with certificates");
            Some(cfg)
        } else {
            None
        };

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
            intermediate_conn: None,
            p2p_clients: HashMap::new(),
            client_config,
            server_config,
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
        // Initiate QUIC connection to Intermediate Server
        self.connect_to_intermediate()?;

        // Send initial QUIC handshake packet immediately
        self.send_pending()?;

        let mut events = Events::with_capacity(1024);

        loop {
            // Calculate timeout based on all connection timeouts
            let timeout = self.calculate_min_timeout()
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

            // Process timeouts for all connections
            self.process_timeouts();

            // Send pending packets for all connections
            self.send_pending()?;

            // Check if we need to register with Intermediate
            self.maybe_register()?;

            // Check connection states
            if let Some(ref conn) = self.intermediate_conn {
                if conn.is_closed() {
                    log::warn!("Intermediate connection closed");
                    // Reconnect logic could go here
                    break;
                }
            }

            // Clean up closed P2P connections
            self.cleanup_closed_p2p();
        }

        Ok(())
    }

    fn calculate_min_timeout(&self) -> Option<Duration> {
        let mut min_timeout = self.intermediate_conn.as_ref().and_then(|c| c.timeout());

        for client in self.p2p_clients.values() {
            if let Some(t) = client.conn.timeout() {
                min_timeout = Some(min_timeout.map_or(t, |m| m.min(t)));
            }
        }

        min_timeout
    }

    fn connect_to_intermediate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Generate connection ID
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        self.rng
            .fill(&mut scid)
            .map_err(|_| "Failed to generate connection ID")?;
        let scid = quiche::ConnectionId::from_ref(&scid);

        // Create QUIC connection to Intermediate Server
        let local_addr = self.quic_socket.local_addr()?;
        let conn = quiche::connect(
            None, // No server name for now
            &scid,
            local_addr,
            self.server_addr,
            &mut self.client_config,
        )?;

        log::info!("Connecting to Intermediate at {} (scid={:?})", self.server_addr, scid);

        self.intermediate_conn = Some(conn);
        self.registered = false;

        Ok(())
    }

    fn process_quic_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Use a separate buffer to avoid borrow conflicts
        let mut pkt_buf = vec![0u8; 65535];

        loop {
            // Receive UDP packet
            let (len, from) = match self.quic_socket.recv_from(&mut self.recv_buf) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };

            // Copy to working buffer to avoid borrow conflicts
            pkt_buf[..len].copy_from_slice(&self.recv_buf[..len]);
            let pkt_slice = &mut pkt_buf[..len];

            log::trace!("Received {} bytes from {}", len, from);

            // Route packet based on source address
            if from == self.server_addr {
                // Packet from Intermediate Server - process with client connection
                self.process_intermediate_packet(pkt_slice, from)?;
            } else {
                // Packet from P2P client (Agent) - process with server logic
                self.process_p2p_packet(pkt_slice, from)?;
            }
        }

        Ok(())
    }

    fn process_intermediate_packet(&mut self, pkt_buf: &mut [u8], from: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.intermediate_conn {
            let recv_info = quiche::RecvInfo {
                from,
                to: self.quic_socket.local_addr()?,
            };

            match conn.recv(pkt_buf, recv_info) {
                Ok(_) => {
                    // Check for established connection
                    if conn.is_established() {
                        // Process incoming DATAGRAMs
                        self.process_intermediate_datagrams()?;
                    }
                }
                Err(e) => {
                    log::debug!("Intermediate connection recv error: {:?}", e);
                }
            }
        }

        Ok(())
    }

    fn process_p2p_packet(&mut self, pkt_buf: &mut [u8], from: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        // P2P server mode not enabled
        if self.server_config.is_none() {
            log::trace!("P2P packet from {} ignored (server mode disabled)", from);
            return Ok(());
        }

        // Parse QUIC header
        let hdr = match quiche::Header::from_slice(pkt_buf, quiche::MAX_CONN_ID_LEN) {
            Ok(v) => v,
            Err(e) => {
                log::debug!("Failed to parse QUIC header from P2P packet: {:?}", e);
                return Ok(());
            }
        };

        let conn_id = hdr.dcid.clone().into_owned();

        // Check if this is an existing P2P connection
        if let Some(client) = self.p2p_clients.get_mut(&conn_id) {
            let recv_info = quiche::RecvInfo {
                from,
                to: self.quic_socket.local_addr()?,
            };

            match client.conn.recv(pkt_buf, recv_info) {
                Ok(_) => {
                    // Process DATAGRAMs from P2P client
                    if client.conn.is_established() {
                        self.process_p2p_client_datagrams(&conn_id)?;
                    }
                }
                Err(e) => {
                    log::debug!("P2P client recv error: {:?}", e);
                }
            }
        } else if hdr.ty == quiche::Type::Initial {
            // New P2P connection from Agent
            self.handle_new_p2p_connection(&hdr, from, pkt_buf)?;
        } else {
            log::debug!("Non-Initial packet for unknown P2P connection from {}", from);
        }

        Ok(())
    }

    fn handle_new_p2p_connection(
        &mut self,
        hdr: &quiche::Header,
        from: SocketAddr,
        pkt_buf: &mut [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server_config = match self.server_config.as_mut() {
            Some(cfg) => cfg,
            None => return Ok(()),
        };

        // Version negotiation if needed
        if !quiche::version_is_supported(hdr.version) {
            log::debug!("Version negotiation needed for P2P client");
            let len = quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut self.send_buf)?;
            self.quic_socket.send_to(&self.send_buf[..len], from)?;
            return Ok(());
        }

        // Generate new connection ID
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        self.rng
            .fill(&mut scid)
            .map_err(|_| "Failed to generate P2P connection ID")?;
        let scid = quiche::ConnectionId::from_ref(&scid);

        // Accept the P2P connection
        let local_addr = self.quic_socket.local_addr()?;
        let conn = quiche::accept(&scid, None, local_addr, from, server_config)?;

        let scid_owned = scid.into_owned();
        log::info!("New P2P connection from Agent at {} (scid={:?})", from, scid_owned);

        // Create P2P client
        let mut client = P2PClient::new(conn, from);

        // Process the Initial packet
        let recv_info = quiche::RecvInfo {
            from,
            to: local_addr,
        };
        client.conn.recv(pkt_buf, recv_info)?;

        // Store the P2P client
        self.p2p_clients.insert(scid_owned, client);

        Ok(())
    }

    fn process_intermediate_datagrams(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut dgrams = Vec::new();

        // Collect DATAGRAMs from Intermediate connection
        if let Some(ref mut conn) = self.intermediate_conn {
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

    fn process_p2p_client_datagrams(&mut self, conn_id: &quiche::ConnectionId<'static>) -> Result<(), Box<dyn std::error::Error>> {
        let mut dgrams = Vec::new();
        let mut should_send_qad = false;
        let mut client_addr = None;

        // Collect DATAGRAMs from P2P client
        if let Some(client) = self.p2p_clients.get_mut(conn_id) {
            let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
            while let Ok(len) = client.conn.dgram_recv(&mut buf) {
                dgrams.push(buf[..len].to_vec());
            }

            // Check if we need to send QAD
            if client.conn.is_established() && !client.qad_sent {
                should_send_qad = true;
                client_addr = Some(client.addr);
            }
        }

        // Send QAD if needed
        if should_send_qad {
            if let Some(addr) = client_addr {
                self.send_qad_to_p2p_client(conn_id, addr)?;
            }
        }

        // Process collected DATAGRAMs (same as from Intermediate)
        for dgram in dgrams {
            if dgram.is_empty() {
                continue;
            }

            match dgram[0] {
                QAD_OBSERVED_ADDRESS => {
                    // Ignore QAD from client
                    log::trace!("Ignoring QAD message from P2P client");
                }
                _ => {
                    // Encapsulated IP packet - forward to local service
                    self.forward_to_local(&dgram)?;
                }
            }
        }

        Ok(())
    }

    fn send_qad_to_p2p_client(&mut self, conn_id: &quiche::ConnectionId<'static>, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(client) = self.p2p_clients.get_mut(conn_id) {
            let qad_msg = qad::build_observed_address(addr);
            match client.conn.dgram_send(&qad_msg) {
                Ok(_) => {
                    log::info!("Sent QAD to P2P client {:?} (observed: {})", conn_id, addr);
                    client.qad_sent = true;
                }
                Err(e) => {
                    log::debug!("Failed to send QAD to P2P client: {:?}", e);
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

            // Send via Intermediate connection (relay path)
            // In future, could also send via P2P connection if available
            if let Some(ref mut conn) = self.intermediate_conn {
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

        if let Some(ref mut conn) = self.intermediate_conn {
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
        // Process Intermediate connection timeout
        if let Some(ref mut conn) = self.intermediate_conn {
            conn.on_timeout();
        }

        // Process P2P connection timeouts
        for client in self.p2p_clients.values_mut() {
            client.conn.on_timeout();
        }

        // Clean up old flow mappings (older than 60 seconds)
        let now = Instant::now();
        self.flow_map.retain(|_, ts| now.duration_since(*ts).as_secs() < 60);
    }

    fn send_pending(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send pending for Intermediate connection
        if let Some(ref mut conn) = self.intermediate_conn {
            loop {
                match conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        self.quic_socket.send_to(&self.send_buf[..len], send_info.to)?;
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        log::debug!("Intermediate send error: {:?}", e);
                        break;
                    }
                }
            }
        }

        // Send pending for P2P connections
        for client in self.p2p_clients.values_mut() {
            loop {
                match client.conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        self.quic_socket.send_to(&self.send_buf[..len], send_info.to)?;
                    }
                    Err(quiche::Error::Done) => break,
                    Err(e) => {
                        log::debug!("P2P client send error: {:?}", e);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn cleanup_closed_p2p(&mut self) {
        let closed: Vec<_> = self.p2p_clients
            .iter()
            .filter(|(_, c)| c.conn.is_closed())
            .map(|(id, _)| id.clone())
            .collect();

        for conn_id in closed {
            if let Some(client) = self.p2p_clients.remove(&conn_id) {
                log::info!("P2P connection closed: {:?} from {}", conn_id, client.addr);
            }
        }
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
