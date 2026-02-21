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
use std::io::{self, Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream as StdTcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::rand::{SecureRandom, SystemRandom};

mod qad;
mod signaling;

use signaling::{
    decode_message, encode_message, gather_candidates_with_observed, DecodeError,
    P2PSessionManager, SignalingMessage,
};

// ============================================================================
// Constants (MUST match Intermediate Server)
// ============================================================================

/// Maximum UDP payload size for QUIC packets (must match Intermediate Server)
const MAX_DATAGRAM_SIZE: usize = 1350;

/// QUIC idle timeout in milliseconds (must match Intermediate Server)
const IDLE_TIMEOUT_MS: u64 = 30_000;

/// Keepalive interval in seconds (should be less than half of idle timeout)
const KEEPALIVE_INTERVAL_SECS: u64 = 10;

/// ALPN protocol identifier (CRITICAL: must match Intermediate Server)
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// Default Intermediate Server port
const DEFAULT_SERVER_PORT: u16 = 4433;

/// Default local forward port (for testing)
const DEFAULT_FORWARD_PORT: u16 = 8080;

/// Default P2P listen port (for direct Agent connections)
const DEFAULT_P2P_PORT: u16 = 4434;

/// Registration message type for Connector
const REG_TYPE_CONNECTOR: u8 = 0x11;

/// QAD message type (OBSERVED_ADDRESS)
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

/// TCP flag: FIN (connection teardown)
const TCP_FIN: u8 = 0x01;
/// TCP flag: SYN (connection establishment)
const TCP_SYN: u8 = 0x02;
/// TCP flag: RST (connection reset)
const TCP_RST: u8 = 0x04;
/// TCP flag: PSH (push buffered data)
const TCP_PSH: u8 = 0x08;
/// TCP flag: ACK (acknowledgment)
const TCP_ACK: u8 = 0x10;

/// Maximum TCP payload per QUIC DATAGRAM: 1350 - 20 (IP) - 20 (TCP)
const MAX_TCP_PAYLOAD: usize = MAX_DATAGRAM_SIZE - 40;

/// TCP session idle timeout in seconds
const TCP_SESSION_TIMEOUT_SECS: u64 = 120;

/// Maximum concurrent TCP proxy sessions (prevents fd exhaustion)
const MAX_TCP_SESSIONS: usize = 256;

/// mio token for QUIC socket
const QUIC_SOCKET_TOKEN: Token = Token(0);

/// mio token for local forwarding socket
const LOCAL_SOCKET_TOKEN: Token = Token(1);

// ============================================================================
// P2P Binding Messages (must match packet_processor::p2p::connectivity)
// ============================================================================

const TRANSACTION_ID_LEN: usize = 12;

#[derive(serde::Serialize, serde::Deserialize)]
struct BindingRequest {
    transaction_id: [u8; TRANSACTION_ID_LEN],
    priority: u64,
    use_candidate: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BindingResponse {
    transaction_id: [u8; TRANSACTION_ID_LEN],
    success: bool,
    mapped_address: Option<SocketAddr>,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum BindingMessage {
    Request(BindingRequest),
    Response(BindingResponse),
}

/// P2P keepalive request type (must match resilience.rs)
const KEEPALIVE_REQUEST: u8 = 0x10;
/// P2P keepalive response type (must match resilience.rs)
const KEEPALIVE_RESPONSE: u8 = 0x11;

/// Returns true if this packet looks like a non-QUIC P2P message.
/// B5: Heuristic P2P control packet demux — relies on first byte bit pattern.
/// Wire format: QUIC long header has bit 7 set (0x80), short header has bit 6 set (0x40).
/// Control packets (binding 0x00/0x01, keepalive 0x10/0x11) have both bits clear.
/// Post-MVP (Task 007): add explicit magic byte to avoid collision with future protocols.
fn is_p2p_control_packet(data: &[u8]) -> bool {
    !data.is_empty() && (data[0] & 0xC0) == 0
}

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

/// Represents a proxied TCP connection through the ZTNA tunnel
struct TcpSession {
    /// Non-blocking TCP connection to the backend service
    stream: StdTcpStream,
    /// Our (Connector-side) next sequence number
    our_seq: u32,
    /// Agent's next expected sequence number
    their_seq: u32,
    /// Agent's source IP (for constructing return packets)
    agent_ip: Ipv4Addr,
    /// Agent's source port
    agent_port: u16,
    /// Virtual service IP (destination in original packet)
    service_ip: Ipv4Addr,
    /// Virtual service port
    service_port: u16,
    /// Last activity time for session cleanup
    last_active: Instant,
    /// Whether the TCP 3-way handshake is complete
    established: bool,
}

// ============================================================================
// Configuration
// ============================================================================

#[derive(Deserialize, Default)]
struct ConnectorConfig {
    intermediate_server: Option<IntermediateServerConfig>,
    services: Option<Vec<ServiceConfig>>,
    p2p: Option<P2PConfig>,
}

#[derive(Deserialize)]
struct IntermediateServerConfig {
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Deserialize)]
struct ServiceConfig {
    id: String,
    backend: Option<String>,
    #[allow(dead_code)]
    protocol: Option<String>,
}

#[derive(Deserialize)]
struct P2PConfig {
    cert: Option<String>,
    key: Option<String>,
    port: Option<u16>,
}

fn load_config(path: &str) -> Result<ConnectorConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let config: ConnectorConfig = serde_json::from_str(&contents)?;
    log::info!("Loaded config from {}", path);
    Ok(config)
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    // --config <path>            Load config from JSON file
    // --server <addr:port>       Intermediate Server address (overrides config)
    // --service <id>             Service ID to register (overrides config)
    // --forward <addr:port>      Local address to forward traffic to (overrides config)
    // --p2p-cert <path>          TLS certificate for P2P server mode (overrides config)
    // --p2p-key <path>           TLS private key for P2P server mode (overrides config)
    // --p2p-listen-port <port>   Port for P2P connections (overrides config)
    // --external-ip <ip>         Public IP for P2P candidates (for NAT/cloud environments)

    // Load config file if provided (or from default paths)
    let config = if let Some(config_path) = parse_arg(&args, "--config") {
        load_config(&config_path)?
    } else {
        // Try default config paths
        let default_paths = ["/etc/ztna/connector.json", "connector.json"];
        let mut loaded = None;
        for path in &default_paths {
            if Path::new(path).exists() {
                match load_config(path) {
                    Ok(cfg) => {
                        loaded = Some(cfg);
                        break;
                    }
                    Err(e) => log::warn!("Failed to load {}: {}", path, e),
                }
            }
        }
        loaded.unwrap_or_default()
    };

    // Build effective config: CLI args override config file values
    let config_server = config.intermediate_server.as_ref();
    let config_host = config_server
        .and_then(|s| s.host.as_deref())
        .unwrap_or("127.0.0.1");
    let config_port = config_server
        .and_then(|s| s.port)
        .unwrap_or(DEFAULT_SERVER_PORT);
    let config_server_addr = format!("{}:{}", config_host, config_port);

    let first_service = config.services.as_ref().and_then(|s| s.first());

    let server_addr = parse_arg(&args, "--server").unwrap_or(config_server_addr);
    let service_id = parse_arg(&args, "--service")
        .or_else(|| first_service.map(|s| s.id.clone()))
        .unwrap_or_else(|| "default".to_string());
    let forward_addr = parse_arg(&args, "--forward")
        .or_else(|| first_service.and_then(|s| s.backend.clone()))
        .unwrap_or_else(|| format!("127.0.0.1:{}", DEFAULT_FORWARD_PORT));

    let p2p_config = config.p2p.as_ref();
    let p2p_cert =
        parse_arg(&args, "--p2p-cert").or_else(|| p2p_config.and_then(|p| p.cert.clone()));
    let p2p_key = parse_arg(&args, "--p2p-key").or_else(|| p2p_config.and_then(|p| p.key.clone()));
    let p2p_port: u16 = parse_arg(&args, "--p2p-listen-port")
        .and_then(|s| s.parse().ok())
        .or_else(|| p2p_config.and_then(|p| p.port))
        .unwrap_or(DEFAULT_P2P_PORT);
    let external_ip: Option<std::net::IpAddr> =
        parse_arg(&args, "--external-ip").and_then(|s| s.parse().ok());

    let server_addr: SocketAddr = server_addr.parse().map_err(|_| "Invalid server address")?;
    let forward_addr: SocketAddr = forward_addr
        .parse()
        .map_err(|_| "Invalid forward address")?;

    log::info!("ZTNA App Connector starting...");
    log::info!("  Server:  {}", server_addr);
    log::info!("  Service: {}", service_id);
    log::info!("  Forward: {}", forward_addr);
    log::info!("  P2P Port: {}", p2p_port);
    log::info!("  ALPN:    {:?}", std::str::from_utf8(ALPN_PROTOCOL));
    log::info!(
        "  P2P:     {}",
        if p2p_cert.is_some() {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(ip) = external_ip {
        log::info!("  External IP: {}", ip);
    }

    // Create connector and run
    let mut connector = Connector::new(
        server_addr,
        service_id,
        forward_addr,
        p2p_cert.as_deref(),
        p2p_key.as_deref(),
        p2p_port,
        external_ip,
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
    /// Forward address for local traffic.
    /// B7: MVP limitation — all TCP traffic routes to this single backend regardless
    /// of destination IP. Post-MVP (Task 009): per-service TCP routing.
    forward_addr: SocketAddr,
    /// Random number generator
    rng: SystemRandom,
    /// Receive buffer
    recv_buf: Vec<u8>,
    /// Send buffer
    send_buf: Vec<u8>,
    /// Stream read buffer
    stream_buf: Vec<u8>,
    /// Whether registration has been sent to Intermediate
    registered: bool,
    /// Observed public address from QAD
    observed_addr: Option<SocketAddr>,
    /// Mapping from local response source to original agent request
    /// Key: (src_ip, src_port, dst_port) from encapsulated packet
    /// Value: timestamp for cleanup
    flow_map: HashMap<(Ipv4Addr, u16, u16), Instant>,
    /// Active TCP proxy sessions, keyed by (src_ip, src_port, dst_ip, dst_port)
    tcp_sessions: HashMap<(Ipv4Addr, u16, Ipv4Addr, u16), TcpSession>,
    /// Buffer for accumulating signaling stream data
    signaling_buffer: Vec<u8>,
    /// P2P session manager
    session_manager: P2PSessionManager,
    /// Last time we sent a keepalive PING to Intermediate
    last_keepalive: Instant,
    /// External/public IP for P2P candidates (for NAT/cloud environments like AWS)
    external_ip: Option<std::net::IpAddr>,
}

impl Connector {
    fn new(
        server_addr: SocketAddr,
        service_id: String,
        forward_addr: SocketAddr,
        p2p_cert_path: Option<&str>,
        p2p_key_path: Option<&str>,
        p2p_port: u16,
        external_ip: Option<std::net::IpAddr>,
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

        // TODO(007): Enable TLS certificate verification — currently disabled for
        // MVP self-signed certs. Task 007 (Security Hardening) will add Let's Encrypt
        // certs and proper CA chain validation.
        client_config.verify_peer(false);

        // Create server config if P2P certificates are provided
        let server_config = if let (Some(cert_path), Some(key_path)) = (p2p_cert_path, p2p_key_path)
        {
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

            // TODO(007): Enable client certificate verification — disabled for MVP.
            // Task 007 (Security Hardening) will add mutual TLS authentication.
            cfg.verify_peer(false);

            log::info!("P2P server mode enabled with certificates");
            Some(cfg)
        } else {
            None
        };

        // Create mio poll
        let poll = Poll::new()?;

        // Create UDP socket for QUIC (bind to P2P port for predictable firewall rules)
        let local_addr: SocketAddr = format!("0.0.0.0:{}", p2p_port).parse()?;
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
            stream_buf: vec![0u8; 65535],
            registered: false,
            observed_addr: None,
            flow_map: HashMap::new(),
            tcp_sessions: HashMap::new(),
            signaling_buffer: Vec::new(),
            session_manager: P2PSessionManager::new(),
            last_keepalive: Instant::now(),
            external_ip,
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
            let timeout = self
                .calculate_min_timeout()
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

            // Poll TCP backend connections for return traffic
            self.process_tcp_sessions()?;

            // Process timeouts for all connections
            self.process_timeouts();

            // Send keepalive to Intermediate if needed
            self.maybe_send_keepalive();

            // Send pending packets for all connections
            self.send_pending()?;

            // Check if we need to register with Intermediate
            self.maybe_register()?;

            // Process signaling streams from Intermediate
            self.process_signaling_streams()?;

            // Cleanup expired P2P sessions
            let expired = self.session_manager.cleanup_expired();
            for session_id in expired {
                log::debug!("Cleaned up expired P2P session {}", session_id);
            }

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

        log::info!(
            "Connecting to Intermediate at {} (scid={:?})",
            self.server_addr,
            scid
        );

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

    fn process_intermediate_packet(
        &mut self,
        pkt_buf: &mut [u8],
        from: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.intermediate_conn {
            let recv_info = quiche::RecvInfo {
                from,
                to: self.quic_socket.local_addr()?,
            };

            match conn.recv(pkt_buf, recv_info) {
                Ok(_read) => {
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

    fn process_p2p_packet(
        &mut self,
        pkt_buf: &mut [u8],
        from: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // P2P server mode not enabled
        if self.server_config.is_none() {
            log::trace!("P2P packet from {} ignored (server mode disabled)", from);
            return Ok(());
        }

        // Demultiplex: P2P control messages vs QUIC packets.
        // Binding messages start with bincode enum index (0x00/0x01),
        // keepalive messages start with 0x10/0x11,
        // QUIC packets have first byte with upper bits set (0x40 or 0x80).
        if is_p2p_control_packet(pkt_buf) {
            return self.process_p2p_control_packet(pkt_buf, from);
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
            log::debug!(
                "Non-Initial packet for unknown P2P connection from {}",
                from
            );
        }

        Ok(())
    }

    fn process_p2p_control_packet(
        &mut self,
        data: &[u8],
        from: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if data.is_empty() {
            return Ok(());
        }

        match data[0] {
            KEEPALIVE_REQUEST => {
                // Echo back with response type byte
                if data.len() == 5 {
                    let mut response = [0u8; 5];
                    response[0] = KEEPALIVE_RESPONSE;
                    response[1..].copy_from_slice(&data[1..]);
                    self.quic_socket.send_to(&response, from)?;
                    log::trace!("Keepalive response sent to {}", from);
                }
            }
            KEEPALIVE_RESPONSE => {
                log::trace!("Keepalive response from {}", from);
            }
            _ => {
                // Try as binding message
                match bincode::deserialize::<BindingMessage>(data) {
                    Ok(BindingMessage::Request(request)) => {
                        log::info!(
                            "Binding request from {} (txn {:02x}{:02x}{:02x}{:02x})",
                            from,
                            request.transaction_id[0],
                            request.transaction_id[1],
                            request.transaction_id[2],
                            request.transaction_id[3]
                        );

                        let response = BindingMessage::Response(BindingResponse {
                            transaction_id: request.transaction_id,
                            success: true,
                            mapped_address: Some(from),
                        });

                        if let Ok(encoded) = bincode::serialize(&response) {
                            self.quic_socket.send_to(&encoded, from)?;
                            log::info!(
                                "Binding response sent to {} ({} bytes)",
                                from,
                                encoded.len()
                            );
                        }
                    }
                    Ok(BindingMessage::Response(response)) => {
                        log::info!(
                            "Binding response from {} (success={})",
                            from,
                            response.success
                        );
                    }
                    Err(e) => {
                        log::debug!(
                            "Unknown P2P control message from {} (type=0x{:02x}, len={}): {}",
                            from,
                            data[0],
                            data.len(),
                            e
                        );
                    }
                }
            }
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
        log::info!(
            "New P2P connection from Agent at {} (scid={:?})",
            from,
            scid_owned
        );

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
            loop {
                match conn.dgram_recv(&mut buf) {
                    Ok(len) => {
                        dgrams.push(buf[..len].to_vec());
                    }
                    Err(e) => {
                        if dgrams.is_empty() {
                            log::trace!("dgram_recv: {:?} (no datagrams)", e);
                        } else {
                            log::debug!(
                                "dgram_recv: {:?} (collected {} datagrams)",
                                e,
                                dgrams.len()
                            );
                        }
                        break;
                    }
                }
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

    fn process_p2p_client_datagrams(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    fn send_qad_to_p2p_client(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        let dst_ip = Ipv4Addr::new(dgram[16], dgram[17], dgram[18], dgram[19]);

        // Handle TCP (protocol 6)
        if protocol == 6 {
            return self.handle_tcp_packet(dgram, ip_header_len, src_ip, dst_ip);
        }

        // Handle ICMP (protocol 1)
        if protocol == 1 {
            return self.handle_icmp_packet(dgram, ip_header_len, src_ip, dst_ip);
        }

        // Handle UDP (protocol 17) - other protocols dropped
        if protocol != 17 {
            log::trace!("Unsupported protocol ({}), dropping", protocol);
            return Ok(());
        }

        // Parse UDP header (8 bytes)
        if dgram.len() < ip_header_len + 8 {
            log::debug!("UDP header truncated");
            return Ok(());
        }

        let udp_header_start = ip_header_len;
        let src_port = u16::from_be_bytes([dgram[udp_header_start], dgram[udp_header_start + 1]]);
        let dst_port =
            u16::from_be_bytes([dgram[udp_header_start + 2], dgram[udp_header_start + 3]]);
        let udp_len =
            u16::from_be_bytes([dgram[udp_header_start + 4], dgram[udp_header_start + 5]]) as usize;

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
            src_ip,
            src_port,
            self.forward_addr.ip(),
            self.forward_addr.port(),
            payload.len()
        );

        // Store flow mapping for return traffic
        self.flow_map
            .insert((src_ip, src_port, dst_port), Instant::now());

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

    fn handle_tcp_packet(
        &mut self,
        dgram: &[u8],
        ip_header_len: usize,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if dgram.len() < ip_header_len + 20 {
            log::debug!("TCP header truncated");
            return Ok(());
        }

        let tcp_start = ip_header_len;
        let src_port = u16::from_be_bytes([dgram[tcp_start], dgram[tcp_start + 1]]);
        let dst_port = u16::from_be_bytes([dgram[tcp_start + 2], dgram[tcp_start + 3]]);
        let seq_num = u32::from_be_bytes([
            dgram[tcp_start + 4],
            dgram[tcp_start + 5],
            dgram[tcp_start + 6],
            dgram[tcp_start + 7],
        ]);
        let _ack_num = u32::from_be_bytes([
            dgram[tcp_start + 8],
            dgram[tcp_start + 9],
            dgram[tcp_start + 10],
            dgram[tcp_start + 11],
        ]);
        let data_offset = ((dgram[tcp_start + 12] >> 4) & 0x0F) as usize * 4;
        let flags = dgram[tcp_start + 13];

        let payload_start = ip_header_len + data_offset;
        let payload = if dgram.len() > payload_start {
            &dgram[payload_start..]
        } else {
            &[]
        };

        let flow_key = (src_ip, src_port, dst_ip, dst_port);
        let mut packets_to_send: Vec<Vec<u8>> = Vec::new();

        if flags & TCP_SYN != 0 && flags & TCP_ACK == 0 {
            // B3: Reject new connections when at capacity (prevents fd exhaustion)
            if self.tcp_sessions.len() >= MAX_TCP_SESSIONS {
                log::warn!(
                    "TCP session limit ({}) reached, sending RST to {}:{}",
                    MAX_TCP_SESSIONS,
                    src_ip,
                    src_port
                );
                packets_to_send.push(build_tcp_packet(
                    dst_ip,
                    dst_port,
                    src_ip,
                    src_port,
                    0,
                    seq_num.wrapping_add(1),
                    TCP_RST | TCP_ACK,
                    0,
                    &[],
                ));
                for packet in &packets_to_send {
                    self.send_ip_packet(packet)?;
                }
                return Ok(());
            }

            // SYN - new connection request
            log::debug!(
                "TCP SYN: {}:{} -> {}:{} (seq={})",
                src_ip,
                src_port,
                dst_ip,
                dst_port,
                seq_num
            );

            // MVP: blocking connect with reduced timeout (500ms).
            // Post-MVP (Task 008): use non-blocking mio::net::TcpStream::connect()
            // to avoid stalling the event loop entirely.
            match StdTcpStream::connect_timeout(&self.forward_addr, Duration::from_millis(500)) {
                Ok(stream) => {
                    stream.set_nonblocking(true)?;
                    stream.set_nodelay(true)?;

                    let our_isn: u32 = {
                        let mut buf = [0u8; 4];
                        let _ = self.rng.fill(&mut buf);
                        u32::from_be_bytes(buf)
                    };

                    let session = TcpSession {
                        stream,
                        our_seq: our_isn.wrapping_add(1),
                        their_seq: seq_num.wrapping_add(1),
                        agent_ip: src_ip,
                        agent_port: src_port,
                        service_ip: dst_ip,
                        service_port: dst_port,
                        last_active: Instant::now(),
                        established: false,
                    };

                    packets_to_send.push(build_tcp_packet(
                        dst_ip,
                        dst_port,
                        src_ip,
                        src_port,
                        our_isn,
                        seq_num.wrapping_add(1),
                        TCP_SYN | TCP_ACK,
                        65535,
                        &[],
                    ));

                    self.tcp_sessions.insert(flow_key, session);
                    log::debug!("TCP session created, SYN-ACK sent");
                }
                Err(e) => {
                    log::warn!("TCP connect to {} failed: {}", self.forward_addr, e);
                    packets_to_send.push(build_tcp_packet(
                        dst_ip,
                        dst_port,
                        src_ip,
                        src_port,
                        0,
                        seq_num.wrapping_add(1),
                        TCP_RST | TCP_ACK,
                        0,
                        &[],
                    ));
                }
            }
        } else if flags & TCP_RST != 0 {
            if self.tcp_sessions.remove(&flow_key).is_some() {
                log::debug!("TCP session reset: {}:{}", src_ip, src_port);
            }
        } else if flags & TCP_FIN != 0 {
            if let Some(session) = self.tcp_sessions.remove(&flow_key) {
                packets_to_send.push(build_tcp_packet(
                    dst_ip,
                    dst_port,
                    src_ip,
                    src_port,
                    session.our_seq,
                    seq_num.wrapping_add(1 + payload.len() as u32),
                    TCP_FIN | TCP_ACK,
                    65535,
                    &[],
                ));
                drop(session.stream);
                log::debug!("TCP session closed by FIN: {}:{}", src_ip, src_port);
            }
        } else if flags & TCP_ACK != 0 {
            let mut remove_session = false;

            if let Some(session) = self.tcp_sessions.get_mut(&flow_key) {
                session.last_active = Instant::now();

                if !session.established {
                    session.established = true;
                    log::debug!("TCP session established: {}:{}", src_ip, src_port);
                }

                if !payload.is_empty() {
                    match session.stream.write(payload) {
                        Ok(n) => {
                            session.their_seq = seq_num.wrapping_add(n as u32);
                            packets_to_send.push(build_tcp_packet(
                                dst_ip,
                                dst_port,
                                src_ip,
                                src_port,
                                session.our_seq,
                                session.their_seq,
                                TCP_ACK,
                                65535,
                                &[],
                            ));
                            log::trace!(
                                "TCP forwarded {} bytes to backend for {}:{}",
                                n,
                                src_ip,
                                src_port
                            );
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // Backend buffer full - don't ACK, agent retransmits
                            log::trace!("TCP write WouldBlock for {}:{}", src_ip, src_port);
                        }
                        Err(e) => {
                            log::debug!("TCP write to backend failed: {}", e);
                            packets_to_send.push(build_tcp_packet(
                                dst_ip,
                                dst_port,
                                src_ip,
                                src_port,
                                session.our_seq,
                                seq_num.wrapping_add(payload.len() as u32),
                                TCP_RST | TCP_ACK,
                                0,
                                &[],
                            ));
                            remove_session = true;
                        }
                    }
                }
            }

            if remove_session {
                self.tcp_sessions.remove(&flow_key);
            }
        }

        for packet in packets_to_send {
            self.send_ip_packet(&packet)?;
        }

        Ok(())
    }

    fn handle_icmp_packet(
        &mut self,
        dgram: &[u8],
        ip_header_len: usize,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // ICMP header is at least 8 bytes
        if dgram.len() < ip_header_len + 8 {
            log::debug!("ICMP header truncated");
            return Ok(());
        }

        let icmp_start = ip_header_len;
        let icmp_type = dgram[icmp_start];
        let icmp_code = dgram[icmp_start + 1];

        // Only handle Echo Request (type 8, code 0)
        if icmp_type != 8 || icmp_code != 0 {
            log::trace!(
                "ICMP type={} code={}, ignoring (only Echo Request handled)",
                icmp_type,
                icmp_code
            );
            return Ok(());
        }

        let icmp_data = &dgram[icmp_start..];

        log::debug!(
            "ICMP Echo Request: {} -> {} ({} bytes)",
            src_ip,
            dst_ip,
            icmp_data.len()
        );

        // Build Echo Reply: swap src/dst IP, change type 8→0, recalculate checksum
        if let Some(reply) = build_icmp_reply(dst_ip, src_ip, icmp_data) {
            self.send_ip_packet(&reply)?;
            log::trace!("ICMP Echo Reply sent: {} -> {}", dst_ip, src_ip);
        }
        Ok(())
    }

    fn send_ip_packet(&mut self, packet: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut conn) = self.intermediate_conn {
            match conn.dgram_send(packet) {
                Ok(_) => {
                    log::trace!("Sent {} byte IP packet via QUIC", packet.len());
                }
                Err(e) => {
                    log::debug!("Failed to send IP packet via QUIC: {:?}", e);
                }
            }
        }
        Ok(())
    }

    fn process_tcp_sessions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut read_buf = [0u8; MAX_TCP_PAYLOAD];
        let mut to_remove = Vec::new();
        let mut packets_to_send: Vec<Vec<u8>> = Vec::new();

        for (flow_key, session) in &mut self.tcp_sessions {
            if !session.established {
                continue;
            }

            loop {
                match session.stream.read(&mut read_buf) {
                    Ok(0) => {
                        // Backend closed connection
                        packets_to_send.push(build_tcp_packet(
                            session.service_ip,
                            session.service_port,
                            session.agent_ip,
                            session.agent_port,
                            session.our_seq,
                            session.their_seq,
                            TCP_FIN | TCP_ACK,
                            65535,
                            &[],
                        ));
                        to_remove.push(*flow_key);
                        log::debug!(
                            "TCP backend closed for {}:{}",
                            session.agent_ip,
                            session.agent_port
                        );
                        break;
                    }
                    Ok(n) => {
                        packets_to_send.push(build_tcp_packet(
                            session.service_ip,
                            session.service_port,
                            session.agent_ip,
                            session.agent_port,
                            session.our_seq,
                            session.their_seq,
                            TCP_PSH | TCP_ACK,
                            65535,
                            &read_buf[..n],
                        ));
                        session.our_seq = session.our_seq.wrapping_add(n as u32);
                        session.last_active = Instant::now();
                        log::trace!(
                            "TCP backend -> agent: {} bytes for {}:{}",
                            n,
                            session.agent_ip,
                            session.agent_port
                        );
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        log::debug!(
                            "TCP backend read error for {}:{}: {}",
                            session.agent_ip,
                            session.agent_port,
                            e
                        );
                        packets_to_send.push(build_tcp_packet(
                            session.service_ip,
                            session.service_port,
                            session.agent_ip,
                            session.agent_port,
                            session.our_seq,
                            session.their_seq,
                            TCP_RST | TCP_ACK,
                            0,
                            &[],
                        ));
                        to_remove.push(*flow_key);
                        break;
                    }
                }
            }
        }

        for key in to_remove {
            self.tcp_sessions.remove(&key);
        }

        for packet in packets_to_send {
            self.send_ip_packet(&packet)?;
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

    fn send_return_traffic(
        &mut self,
        payload: &[u8],
        from: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                            packet.len(),
                            orig_src_ip,
                            orig_src_port
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

            log::debug!(
                "DATAGRAM max_writable_len={:?}",
                conn.dgram_max_writable_len()
            );

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
        self.flow_map
            .retain(|_, ts| now.duration_since(*ts).as_secs() < 60);

        // Clean up idle TCP sessions
        let tcp_timeout = TCP_SESSION_TIMEOUT_SECS;
        self.tcp_sessions
            .retain(|_, session| now.duration_since(session.last_active).as_secs() < tcp_timeout);
    }

    /// Send a QUIC PING to keep the Intermediate connection alive
    fn maybe_send_keepalive(&mut self) {
        if self.last_keepalive.elapsed().as_secs() >= KEEPALIVE_INTERVAL_SECS {
            if let Some(ref mut conn) = self.intermediate_conn {
                if conn.is_established() {
                    // send_ack_eliciting() sends a PING frame to keep connection alive
                    match conn.send_ack_eliciting() {
                        Ok(_) => {
                            log::debug!("Sent keepalive PING to Intermediate");
                        }
                        Err(e) => {
                            log::warn!("Failed to send keepalive: {:?}", e);
                        }
                    }
                }
            }
            self.last_keepalive = Instant::now();
        }
    }

    fn send_pending(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send pending for Intermediate connection
        if let Some(ref mut conn) = self.intermediate_conn {
            loop {
                match conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        self.quic_socket
                            .send_to(&self.send_buf[..len], send_info.to)?;
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
                        self.quic_socket
                            .send_to(&self.send_buf[..len], send_info.to)?;
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
        let closed: Vec<_> = self
            .p2p_clients
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

    /// Process signaling streams from Intermediate Server
    fn process_signaling_streams(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we have an established connection
        let has_conn = self
            .intermediate_conn
            .as_ref()
            .map(|c| c.is_established())
            .unwrap_or(false);

        if !has_conn {
            return Ok(());
        }

        // Collect readable streams
        let readable_streams: Vec<u64> = {
            if let Some(ref conn) = self.intermediate_conn {
                conn.readable().collect()
            } else {
                return Ok(());
            }
        };

        // Read from each stream
        for stream_id in readable_streams {
            if let Some(ref mut conn) = self.intermediate_conn {
                loop {
                    match conn.stream_recv(stream_id, &mut self.stream_buf) {
                        Ok((len, _fin)) => {
                            if len == 0 {
                                break;
                            }
                            self.signaling_buffer
                                .extend_from_slice(&self.stream_buf[..len]);
                        }
                        Err(quiche::Error::Done) => break,
                        Err(e) => {
                            log::debug!("Stream recv error on {}: {:?}", stream_id, e);
                            break;
                        }
                    }
                }
            }
        }

        // Try to decode and handle messages
        self.process_signaling_messages()?;

        Ok(())
    }

    /// Process decoded signaling messages from buffer
    fn process_signaling_messages(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            if self.signaling_buffer.is_empty() {
                break;
            }

            match decode_message(&self.signaling_buffer) {
                Ok((msg, consumed)) => {
                    log::info!("Received signaling message: {:?}", msg);

                    // Consume the bytes
                    self.signaling_buffer.drain(..consumed);

                    // Handle the message
                    self.handle_signaling_message(msg)?;
                }
                Err(DecodeError::Incomplete(_)) => {
                    // Need more data
                    break;
                }
                Err(DecodeError::TooLarge(size)) => {
                    log::error!("Signaling message too large: {} bytes", size);
                    self.signaling_buffer.clear();
                    break;
                }
                Err(DecodeError::Invalid(e)) => {
                    log::error!("Invalid signaling message: {}", e);
                    self.signaling_buffer.clear();
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a decoded signaling message
    fn handle_signaling_message(
        &mut self,
        msg: SignalingMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match msg {
            SignalingMessage::CandidateOffer {
                session_id,
                service_id: _,
                candidates,
            } => {
                log::info!(
                    "CandidateOffer: session={}, {} candidates",
                    session_id,
                    candidates.len()
                );

                // Create session
                self.session_manager.create_session(session_id, candidates);

                // Gather our candidates
                let bind_addr = self.quic_socket.local_addr()?;
                // When external_ip is set (e.g., AWS Elastic IP), override the
                // observed address so the ServerReflexive candidate uses the
                // publicly routable IP instead of a VPC-internal address.
                let effective_observed = if let Some(ext_ip) = self.external_ip {
                    Some(SocketAddr::new(ext_ip, bind_addr.port()))
                } else {
                    self.observed_addr
                };
                let local_candidates = gather_candidates_with_observed(
                    bind_addr,
                    effective_observed,
                    Some(self.server_addr),
                );

                // Update session with local candidates
                if let Some(session) = self.session_manager.get_session_mut(session_id) {
                    session.set_local_candidates(local_candidates.clone());
                }

                // Send CandidateAnswer
                let answer = SignalingMessage::CandidateAnswer {
                    session_id,
                    candidates: local_candidates,
                };
                self.send_signaling_message(&answer)?;

                log::info!("Sent CandidateAnswer for session {}", session_id);
            }

            SignalingMessage::StartPunching {
                session_id,
                start_delay_ms,
                peer_candidates,
            } => {
                log::info!(
                    "StartPunching: session={}, delay={}ms, {} peer candidates",
                    session_id,
                    start_delay_ms,
                    peer_candidates.len()
                );

                // Update session
                if let Some(session) = self.session_manager.get_session_mut(session_id) {
                    // Update with peer candidates if provided
                    if !peer_candidates.is_empty() {
                        session.agent_candidates = peer_candidates;
                    }
                    session.set_punch_start(start_delay_ms);
                }

                // For MVP, we'll report success immediately since we're on localhost
                // In a real implementation, we'd perform connectivity checks here
                log::info!("P2P session {} ready for connectivity checks", session_id);
            }

            SignalingMessage::PunchingResult {
                session_id,
                success,
                working_address,
            } => {
                log::info!(
                    "PunchingResult from peer: session={}, success={}, addr={:?}",
                    session_id,
                    success,
                    working_address
                );

                // Update our session state
                if let Some(session) = self.session_manager.get_session_mut(session_id) {
                    if success {
                        if let Some(addr) = working_address {
                            session.set_connected(addr);
                        }
                    } else {
                        session.set_fallback();
                    }
                }
            }

            SignalingMessage::CandidateAnswer { session_id, .. } => {
                // Connector shouldn't receive CandidateAnswer (that's what it sends)
                log::warn!(
                    "Unexpected CandidateAnswer received for session {}",
                    session_id
                );
            }

            SignalingMessage::Error {
                session_id,
                code,
                message,
            } => {
                log::error!(
                    "Signaling error: session={:?}, code={:?}, msg={}",
                    session_id,
                    code,
                    message
                );

                // Clean up the session
                if let Some(sid) = session_id {
                    self.session_manager.remove_session(sid);
                }
            }
        }

        Ok(())
    }

    /// Send a signaling message to the Intermediate Server
    fn send_signaling_message(
        &mut self,
        msg: &SignalingMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_message(msg).map_err(|e| format!("encode error: {}", e))?;

        if let Some(ref mut conn) = self.intermediate_conn {
            // Send on stream 0 (client-initiated bidirectional)
            match conn.stream_send(0, &encoded, false) {
                Ok(_) => {
                    log::debug!("Sent signaling message ({} bytes)", encoded.len());
                }
                Err(e) => {
                    log::error!("Failed to send signaling message: {:?}", e);
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

#[allow(clippy::too_many_arguments)]
fn build_tcp_packet(
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_ip: Ipv4Addr,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
) -> Vec<u8> {
    let tcp_header_len = 20;
    let tcp_len = tcp_header_len + payload.len();
    let total_len = 20 + tcp_len;

    let mut packet = vec![0u8; total_len];

    // IP Header (20 bytes)
    packet[0] = 0x45; // Version 4, IHL 5
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[6..8].copy_from_slice(&[0x40, 0x00]); // Don't Fragment
    packet[8] = 64; // TTL
    packet[9] = 6; // Protocol: TCP
    packet[12..16].copy_from_slice(&src_ip.octets());
    packet[16..20].copy_from_slice(&dst_ip.octets());

    let ip_cksum = ip_checksum(&packet[0..20]);
    packet[10..12].copy_from_slice(&ip_cksum.to_be_bytes());

    // TCP Header (20 bytes)
    let t = 20; // tcp_start
    packet[t..t + 2].copy_from_slice(&src_port.to_be_bytes());
    packet[t + 2..t + 4].copy_from_slice(&dst_port.to_be_bytes());
    packet[t + 4..t + 8].copy_from_slice(&seq.to_be_bytes());
    packet[t + 8..t + 12].copy_from_slice(&ack.to_be_bytes());
    packet[t + 12] = 0x50; // Data offset: 5 words (20 bytes)
    packet[t + 13] = flags;
    packet[t + 14..t + 16].copy_from_slice(&window.to_be_bytes());

    // TCP Payload
    if !payload.is_empty() {
        packet[t + 20..].copy_from_slice(payload);
    }

    // TCP Checksum (includes pseudo-header)
    let tcp_cksum = tcp_checksum(src_ip, dst_ip, &packet[t..]);
    packet[t + 16..t + 18].copy_from_slice(&tcp_cksum.to_be_bytes());

    packet
}

fn tcp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_segment: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    let src = src_ip.octets();
    let dst = dst_ip.octets();
    sum = sum.wrapping_add(((src[0] as u32) << 8) | src[1] as u32);
    sum = sum.wrapping_add(((src[2] as u32) << 8) | src[3] as u32);
    sum = sum.wrapping_add(((dst[0] as u32) << 8) | dst[1] as u32);
    sum = sum.wrapping_add(((dst[2] as u32) << 8) | dst[3] as u32);
    sum = sum.wrapping_add(6); // Protocol: TCP
    sum = sum.wrapping_add(tcp_segment.len() as u32);

    // TCP segment (header + data)
    for i in (0..tcp_segment.len()).step_by(2) {
        let word = if i + 1 < tcp_segment.len() {
            ((tcp_segment[i] as u32) << 8) | (tcp_segment[i + 1] as u32)
        } else {
            (tcp_segment[i] as u32) << 8
        };
        sum = sum.wrapping_add(word);
    }

    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

fn build_icmp_reply(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, echo_request: &[u8]) -> Option<Vec<u8>> {
    // B1: Validate minimum ICMP echo length (type + code + checksum + id + seq = 8 bytes)
    if echo_request.len() < 8 {
        log::warn!(
            "Dropping malformed ICMP packet ({} bytes, need >= 8)",
            echo_request.len()
        );
        return None;
    }

    let total_len = 20 + echo_request.len();
    let mut packet = vec![0u8; total_len];

    // IP Header (20 bytes)
    packet[0] = 0x45;
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[6..8].copy_from_slice(&[0x40, 0x00]); // Don't Fragment
    packet[8] = 64; // TTL
    packet[9] = 1; // Protocol: ICMP
    packet[12..16].copy_from_slice(&src_ip.octets());
    packet[16..20].copy_from_slice(&dst_ip.octets());

    let ip_cksum = ip_checksum(&packet[0..20]);
    packet[10..12].copy_from_slice(&ip_cksum.to_be_bytes());

    // Copy ICMP data from request, then change type to Echo Reply (0)
    packet[20..].copy_from_slice(echo_request);
    packet[20] = 0; // Type: Echo Reply

    // Zero out ICMP checksum and recalculate
    packet[22] = 0;
    packet[23] = 0;
    let icmp_cksum = icmp_checksum(&packet[20..]);
    packet[22..24].copy_from_slice(&icmp_cksum.to_be_bytes());

    Some(packet)
}

fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    for i in (0..data.len()).step_by(2) {
        let word = if i + 1 < data.len() {
            ((data[i] as u32) << 8) | (data[i + 1] as u32)
        } else {
            (data[i] as u32) << 8
        };
        sum = sum.wrapping_add(word);
    }

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
        assert_eq!(packet[9], 17); // UDP protocol

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

    #[test]
    fn test_default_p2p_port() {
        // P2P port should be 4434 (one above Intermediate's 4433)
        assert_eq!(DEFAULT_P2P_PORT, 4434);
        assert_eq!(DEFAULT_SERVER_PORT, 4433);
        assert_eq!(DEFAULT_P2P_PORT, DEFAULT_SERVER_PORT + 1);
    }

    #[test]
    fn test_tcp_flags() {
        assert_eq!(TCP_FIN, 0x01);
        assert_eq!(TCP_SYN, 0x02);
        assert_eq!(TCP_RST, 0x04);
        assert_eq!(TCP_PSH, 0x08);
        assert_eq!(TCP_ACK, 0x10);
        // SYN-ACK
        assert_eq!(TCP_SYN | TCP_ACK, 0x12);
        // FIN-ACK
        assert_eq!(TCP_FIN | TCP_ACK, 0x11);
        // PSH-ACK (data)
        assert_eq!(TCP_PSH | TCP_ACK, 0x18);
    }

    #[test]
    fn test_build_tcp_packet_syn_ack() {
        let packet = build_tcp_packet(
            Ipv4Addr::new(10, 100, 0, 1),
            80,
            Ipv4Addr::new(192, 168, 1, 100),
            54321,
            1000, // seq
            500,  // ack
            TCP_SYN | TCP_ACK,
            65535,
            &[],
        );

        // IP header checks
        assert_eq!(packet[0], 0x45); // IPv4, IHL=5
        assert_eq!(packet[9], 6); // TCP protocol
        assert_eq!(packet.len(), 40); // 20 IP + 20 TCP, no payload

        // TCP header checks
        let src_port = u16::from_be_bytes([packet[20], packet[21]]);
        let dst_port = u16::from_be_bytes([packet[22], packet[23]]);
        assert_eq!(src_port, 80);
        assert_eq!(dst_port, 54321);

        let seq = u32::from_be_bytes([packet[24], packet[25], packet[26], packet[27]]);
        let ack = u32::from_be_bytes([packet[28], packet[29], packet[30], packet[31]]);
        assert_eq!(seq, 1000);
        assert_eq!(ack, 500);

        assert_eq!(packet[32], 0x50); // Data offset: 5 words
        assert_eq!(packet[33], TCP_SYN | TCP_ACK); // Flags
    }

    #[test]
    fn test_build_tcp_packet_with_data() {
        let payload = b"HTTP/1.1 200 OK\r\n";
        let packet = build_tcp_packet(
            Ipv4Addr::new(10, 0, 0, 1),
            8080,
            Ipv4Addr::new(172, 16, 0, 1),
            12345,
            2000,
            1500,
            TCP_PSH | TCP_ACK,
            65535,
            payload,
        );

        assert_eq!(packet.len(), 40 + payload.len());
        assert_eq!(packet[33], TCP_PSH | TCP_ACK);
        assert_eq!(&packet[40..], payload);
    }

    #[test]
    fn test_tcp_checksum_validity() {
        let packet = build_tcp_packet(
            Ipv4Addr::new(192, 168, 1, 1),
            80,
            Ipv4Addr::new(192, 168, 1, 2),
            54321,
            100,
            200,
            TCP_ACK,
            65535,
            b"test",
        );

        // Verify TCP checksum: recomputing over the TCP segment
        // (with checksum field included) using the pseudo-header should yield 0
        let tcp_segment = &packet[20..];
        let result = tcp_checksum(
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(192, 168, 1, 2),
            tcp_segment,
        );
        assert_eq!(result, 0, "TCP checksum should verify to 0");
    }

    #[test]
    fn test_max_tcp_payload_fits_datagram() {
        // MAX_TCP_PAYLOAD + IP header + TCP header must fit in MAX_DATAGRAM_SIZE
        assert_eq!(MAX_TCP_PAYLOAD + 40, MAX_DATAGRAM_SIZE);
        assert_eq!(MAX_TCP_PAYLOAD, 1310);
    }

    #[test]
    fn test_build_icmp_reply() {
        // Build a mock Echo Request ICMP payload:
        // Type=8, Code=0, Checksum=XX, Identifier=0x1234, Sequence=0x0001, Data="ping"
        let mut echo_request = vec![
            8, // Type: Echo Request
            0, // Code
            0, 0, // Checksum (placeholder)
            0x12, 0x34, // Identifier
            0x00, 0x01, // Sequence
        ];
        echo_request.extend_from_slice(b"ping");

        // Calculate request checksum
        let cksum = icmp_checksum(&echo_request);
        echo_request[2..4].copy_from_slice(&cksum.to_be_bytes());

        let reply = build_icmp_reply(
            Ipv4Addr::new(10, 100, 0, 1),
            Ipv4Addr::new(192, 168, 1, 100),
            &echo_request,
        )
        .expect("valid ICMP echo should produce reply");

        // IP header checks
        assert_eq!(reply[0], 0x45);
        assert_eq!(reply[9], 1); // ICMP protocol
        assert_eq!(reply.len(), 20 + echo_request.len());

        // Source IP should be the service IP
        assert_eq!(&reply[12..16], &[10, 100, 0, 1]);
        // Dest IP should be the agent IP
        assert_eq!(&reply[16..20], &[192, 168, 1, 100]);

        // ICMP type should be Echo Reply (0)
        assert_eq!(reply[20], 0);
        assert_eq!(reply[21], 0); // Code unchanged

        // Identifier and sequence should be preserved
        assert_eq!(&reply[24..26], &[0x12, 0x34]);
        assert_eq!(&reply[26..28], &[0x00, 0x01]);

        // Data should be preserved
        assert_eq!(&reply[28..], b"ping");

        // ICMP checksum should verify
        assert_eq!(icmp_checksum(&reply[20..]), 0);
    }

    #[test]
    fn test_icmp_checksum_validity() {
        let data = [
            0u8, 0, // Type: Echo Reply, Code: 0
            0, 0, // Checksum (will be computed)
            0x12, 0x34, // Identifier
            0x00, 0x01, // Sequence
            b't', b'e', b's', b't', // Data
        ];
        let cksum = icmp_checksum(&data);

        let mut with_cksum = data.to_vec();
        with_cksum[2..4].copy_from_slice(&cksum.to_be_bytes());

        assert_eq!(
            icmp_checksum(&with_cksum),
            0,
            "ICMP checksum should verify to 0"
        );
    }
}
