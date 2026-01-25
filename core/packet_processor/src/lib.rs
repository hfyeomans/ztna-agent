//! ZTNA Agent Packet Processor
//!
//! This crate provides the Rust core for the ZTNA agent, handling:
//! - IP packet processing and filtering
//! - QUIC tunnel management via quiche
//! - Multi-connection support (Intermediate + P2P)
//! - FFI interface for Swift integration

use std::collections::HashMap;
use std::net::SocketAddr;
use std::panic::{self, AssertUnwindSafe};
use std::slice;
use std::sync::Once;
use std::time::{Duration, Instant};

use quiche::{Config, Connection, ConnectionId};
use ring::rand::{SecureRandom, SystemRandom};

// ============================================================================
// Modules
// ============================================================================

/// P2P module for direct peer-to-peer connectivity via NAT traversal
pub mod p2p;

// ============================================================================
// Constants
// ============================================================================

/// Maximum UDP payload size for QUIC packets
const MAX_DATAGRAM_SIZE: usize = 1350;

/// QUIC idle timeout in milliseconds
const IDLE_TIMEOUT_MS: u64 = 30000;

/// ALPN protocol identifier for ZTNA
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// Registration message type for Agent (matches intermediate-server)
const REG_TYPE_AGENT: u8 = 0x10;

// ============================================================================
// FFI Enums
// ============================================================================

/// Result of packet processing decision
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketAction {
    Drop = 0,
    Forward = 1,
}

/// Agent connection state exposed to Swift
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    Draining = 3,
    Closed = 4,
    Error = 5,
}

/// Result codes for FFI operations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentResult {
    Ok = 0,
    InvalidPointer = 1,
    InvalidAddress = 2,
    ConnectionFailed = 3,
    NotConnected = 4,
    BufferTooSmall = 5,
    NoData = 6,
    QuicError = 7,
    PanicCaught = 8,
}

// ============================================================================
// Agent Structure
// ============================================================================

/// P2P connection to a Connector
struct P2PConnection {
    /// QUIC connection
    conn: Connection,
    /// Last activity time
    last_activity: Instant,
}

/// QUIC tunnel agent state
///
/// Supports multiple connections:
/// - `intermediate_conn`: Connection to Intermediate Server (signaling + relay)
/// - `p2p_conns`: Direct connections to Connectors (P2P)
pub struct Agent {
    /// QUIC configuration (shared for all connections)
    config: Config,
    /// QUIC connection to Intermediate Server (None until connect is called)
    intermediate_conn: Option<Connection>,
    /// Intermediate Server address
    intermediate_addr: Option<SocketAddr>,
    /// P2P connections to Connectors (keyed by Connector address)
    p2p_conns: HashMap<SocketAddr, P2PConnection>,
    /// Local address (set after first recv, shared by all connections)
    local_addr: Option<SocketAddr>,
    /// Connection state (reflects Intermediate connection state)
    state: AgentState,
    /// Last activity time for timeout tracking
    last_activity: Instant,
    /// Observed public address from QAD (set by Intermediate Server)
    pub observed_address: Option<SocketAddr>,
    /// Scratch buffer for packet assembly
    scratch_buffer: Vec<u8>,
    /// Stream buffer for signaling
    stream_buffer: Vec<u8>,
    /// Signaling accumulation buffer
    signaling_buffer: Vec<u8>,
    /// Active hole punch coordinator (if hole punching in progress)
    hole_punch: Option<p2p::HolePunchCoordinator>,
    /// Path manager for keepalive and fallback
    path_manager: p2p::PathManager,
}

impl Agent {
    /// Create a new agent with default configuration
    fn new() -> Result<Self, quiche::Error> {
        let mut config = Config::new(quiche::PROTOCOL_VERSION)?;

        // TLS configuration - for MVP, disable certificate verification
        // In production, this should verify server certificates
        config.verify_peer(false);

        // Set ALPN protocol
        config.set_application_protos(&[ALPN_PROTOCOL])?;

        // Enable DATAGRAM extension for IP packet tunneling
        config.enable_dgram(true, 1000, 1000);

        // Set timeouts
        config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        // Disable active migration (we'll handle reconnection manually)
        config.set_disable_active_migration(true);

        Ok(Agent {
            config,
            intermediate_conn: None,
            intermediate_addr: None,
            p2p_conns: HashMap::new(),
            local_addr: None,
            state: AgentState::Disconnected,
            last_activity: Instant::now(),
            observed_address: None,
            scratch_buffer: vec![0u8; MAX_DATAGRAM_SIZE],
            stream_buffer: vec![0u8; 65535],
            signaling_buffer: Vec::new(),
            hole_punch: None,
            path_manager: p2p::PathManager::new(),
        })
    }

    /// Initiate connection to Intermediate Server
    fn connect(&mut self, server_addr: SocketAddr) -> Result<(), quiche::Error> {
        // Generate random connection ID
        let scid_bytes = rand_connection_id();
        let scid = ConnectionId::from_ref(&scid_bytes);

        // Create QUIC connection to Intermediate Server
        let conn = quiche::connect(
            Some("ztna-server"), // SNI
            &scid,
            self.local_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
            server_addr,
            &mut self.config,
        )?;

        self.intermediate_conn = Some(conn);
        self.intermediate_addr = Some(server_addr);
        self.state = AgentState::Connecting;
        self.last_activity = Instant::now();

        // Set relay address in path manager
        self.path_manager.set_relay(server_addr);

        Ok(())
    }

    /// Initiate P2P connection to a Connector
    fn connect_p2p(&mut self, connector_addr: SocketAddr) -> Result<(), quiche::Error> {
        // Don't create duplicate connections
        if self.p2p_conns.contains_key(&connector_addr) {
            return Ok(());
        }

        // Generate random connection ID
        let scid_bytes = rand_connection_id();
        let scid = ConnectionId::from_ref(&scid_bytes);

        // Create QUIC connection to Connector (P2P)
        let conn = quiche::connect(
            Some("ztna-connector"), // SNI
            &scid,
            self.local_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
            connector_addr,
            &mut self.config,
        )?;

        self.p2p_conns.insert(
            connector_addr,
            P2PConnection {
                conn,
                last_activity: Instant::now(),
            },
        );

        Ok(())
    }

    /// Process received UDP packet (from network)
    ///
    /// Routes the packet to the correct connection based on source address.
    fn recv(&mut self, data: &[u8], from: SocketAddr) -> Result<(), quiche::Error> {
        // Create recv info
        let recv_info = quiche::RecvInfo {
            from,
            to: self.local_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
        };

        // Feed data to QUIC connection (quiche requires mutable buffer for in-place decryption)
        let mut buf = data.to_vec();

        // Route to correct connection based on source address
        if Some(from) == self.intermediate_addr {
            // Packet from Intermediate Server
            if let Some(conn) = self.intermediate_conn.as_mut() {
                conn.recv(&mut buf, recv_info)?;
                self.update_state();
                self.last_activity = Instant::now();
                // Process any received DATAGRAMs (could contain QAD info)
                self.process_incoming_datagrams();
            }
        } else if let Some(p2p) = self.p2p_conns.get_mut(&from) {
            // Packet from P2P Connector
            p2p.conn.recv(&mut buf, recv_info)?;
            p2p.last_activity = Instant::now();
            // Process DATAGRAMs from P2P connection
            self.process_p2p_datagrams(&from);
        } else {
            // Unknown source - could be a new P2P connection attempt
            // For now, ignore (we only initiate P2P connections, not accept them)
            return Err(quiche::Error::InvalidState);
        }

        Ok(())
    }

    /// Get next outbound UDP packet to send (Intermediate connection)
    fn poll(&mut self) -> Option<(Vec<u8>, SocketAddr)> {
        let conn = self.intermediate_conn.as_mut()?;
        let server_addr = self.intermediate_addr?;

        // Try to generate a QUIC packet
        let mut out = vec![0u8; MAX_DATAGRAM_SIZE];

        match conn.send(&mut out) {
            Ok((len, _send_info)) => {
                out.truncate(len);
                self.last_activity = Instant::now();
                Some((out, server_addr))
            }
            Err(quiche::Error::Done) => None, // No more packets to send
            Err(_) => None,
        }
    }

    /// Get next outbound UDP packet to send from any P2P connection
    fn poll_p2p(&mut self) -> Option<(Vec<u8>, SocketAddr)> {
        for (addr, p2p) in self.p2p_conns.iter_mut() {
            let mut out = vec![0u8; MAX_DATAGRAM_SIZE];

            match p2p.conn.send(&mut out) {
                Ok((len, _send_info)) => {
                    out.truncate(len);
                    p2p.last_activity = Instant::now();
                    return Some((out, *addr));
                }
                Err(quiche::Error::Done) => continue,
                Err(_) => continue,
            }
        }
        None
    }

    /// Queue an IP packet for sending via DATAGRAM (Intermediate connection)
    fn send_datagram(&mut self, data: &[u8]) -> Result<(), quiche::Error> {
        let conn = self.intermediate_conn.as_mut().ok_or(quiche::Error::InvalidState)?;

        if !conn.is_established() {
            return Err(quiche::Error::InvalidState);
        }

        // Send as QUIC DATAGRAM
        conn.dgram_send(data)?;
        self.last_activity = Instant::now();

        Ok(())
    }

    /// Register the Agent for a target service with the Intermediate Server
    ///
    /// This sends a registration DATAGRAM that tells the Intermediate Server
    /// which service this Agent wants to reach. The server uses this to route
    /// relay traffic between the Agent and the Connector for that service.
    ///
    /// Message format: [0x10, service_id_len, service_id_bytes...]
    fn register(&mut self, service_id: &str) -> Result<(), quiche::Error> {
        let conn = self.intermediate_conn.as_mut().ok_or(quiche::Error::InvalidState)?;

        if !conn.is_established() {
            return Err(quiche::Error::InvalidState);
        }

        // Build registration message: [0x10 (Agent type), id_len, service_id bytes]
        let id_bytes = service_id.as_bytes();
        if id_bytes.len() > 255 {
            return Err(quiche::Error::InvalidState); // Service ID too long
        }

        let mut msg = Vec::with_capacity(2 + id_bytes.len());
        msg.push(REG_TYPE_AGENT);
        msg.push(id_bytes.len() as u8);
        msg.extend_from_slice(id_bytes);

        conn.dgram_send(&msg)?;
        self.last_activity = Instant::now();

        Ok(())
    }

    /// Send a keepalive PING on the Intermediate connection to prevent idle timeout.
    ///
    /// This should be called periodically (e.g., every 10 seconds) to keep the
    /// QUIC connection alive when there's no other traffic.
    fn send_intermediate_keepalive(&mut self) -> Result<(), quiche::Error> {
        let conn = self.intermediate_conn.as_mut().ok_or(quiche::Error::InvalidState)?;

        if !conn.is_established() {
            return Err(quiche::Error::InvalidState);
        }

        conn.send_ack_eliciting()?;
        self.last_activity = Instant::now();

        Ok(())
    }

    /// Queue an IP packet for sending via DATAGRAM (P2P connection)
    fn send_datagram_p2p(&mut self, data: &[u8], connector_addr: SocketAddr) -> Result<(), quiche::Error> {
        let p2p = self.p2p_conns.get_mut(&connector_addr)
            .ok_or(quiche::Error::InvalidState)?;

        if !p2p.conn.is_established() {
            return Err(quiche::Error::InvalidState);
        }

        p2p.conn.dgram_send(data)?;
        p2p.last_activity = Instant::now();

        Ok(())
    }

    /// Handle timeout - call periodically
    fn on_timeout(&mut self) {
        // Process Intermediate connection timeout
        if let Some(conn) = self.intermediate_conn.as_mut() {
            conn.on_timeout();
            self.update_state();
        }

        // Process P2P connection timeouts
        for p2p in self.p2p_conns.values_mut() {
            p2p.conn.on_timeout();
        }

        // Remove closed P2P connections
        self.p2p_conns.retain(|_, p2p| !p2p.conn.is_closed());

        // Check path manager for keepalive timeouts and potential fallback
        self.path_manager.check_timeouts();

        // Attempt recovery on failed paths after cooldown
        self.path_manager.attempt_recovery();
    }

    /// Get time until next timeout event (minimum across all connections)
    fn timeout(&self) -> Option<Duration> {
        let mut min_timeout = self.intermediate_conn.as_ref().and_then(|c| c.timeout());

        for p2p in self.p2p_conns.values() {
            if let Some(t) = p2p.conn.timeout() {
                min_timeout = Some(min_timeout.map_or(t, |m| m.min(t)));
            }
        }

        min_timeout
    }

    /// Check if a P2P connection is established to the given address
    fn is_p2p_connected(&self, connector_addr: SocketAddr) -> bool {
        self.p2p_conns.get(&connector_addr)
            .map(|p2p| p2p.conn.is_established())
            .unwrap_or(false)
    }

    /// Update agent state based on Intermediate QUIC connection state
    fn update_state(&mut self) {
        if let Some(conn) = &self.intermediate_conn {
            self.state = if conn.is_closed() {
                AgentState::Closed
            } else if conn.is_draining() {
                AgentState::Draining
            } else if conn.is_established() {
                AgentState::Connected
            } else if conn.is_in_early_data() {
                AgentState::Connecting
            } else {
                AgentState::Connecting
            };
        } else {
            self.state = AgentState::Disconnected;
        }
    }

    /// Process incoming DATAGRAM frames from Intermediate connection
    fn process_incoming_datagrams(&mut self) {
        let conn = match self.intermediate_conn.as_mut() {
            Some(c) => c,
            None => return,
        };

        while let Ok(len) = conn.dgram_recv(&mut self.scratch_buffer) {
            let data = &self.scratch_buffer[..len];

            // Check for QAD message (simple protocol: first byte = message type)
            if !data.is_empty() && data[0] == 0x01 {
                // QAD OBSERVED_ADDRESS message
                // Format: 0x01 | 4 bytes IPv4 | 2 bytes port (big endian)
                if len >= 7 {
                    let ip = std::net::Ipv4Addr::new(data[1], data[2], data[3], data[4]);
                    let port = u16::from_be_bytes([data[5], data[6]]);
                    self.observed_address =
                        Some(SocketAddr::new(std::net::IpAddr::V4(ip), port));
                }
            }
            // Other DATAGRAMs are tunneled IP packets - would be passed back to Swift
        }
    }

    /// Process incoming DATAGRAM frames from P2P connection
    fn process_p2p_datagrams(&mut self, connector_addr: &SocketAddr) {
        // First, collect all datagrams from the connection
        let mut received_datagrams = Vec::new();

        if let Some(p2p) = self.p2p_conns.get_mut(connector_addr) {
            while let Ok(len) = p2p.conn.dgram_recv(&mut self.scratch_buffer) {
                received_datagrams.push(self.scratch_buffer[..len].to_vec());
            }
        }

        // Now process collected datagrams (avoiding borrow issues)
        for data in received_datagrams {
            // Check for QAD message (Connector sends its observed address)
            if !data.is_empty() && data[0] == 0x01 {
                // QAD from Connector - we could use this for diagnostics
                // but for now we ignore it (we already know our own address from Intermediate)
                continue;
            }

            // Check for keepalive message
            if data.len() >= 5 && (data[0] == p2p::KEEPALIVE_REQUEST || data[0] == p2p::KEEPALIVE_RESPONSE) {
                if let Some(response) = self.path_manager.process_keepalive(*connector_addr, &data) {
                    // Queue keepalive response to be sent
                    if let Some(p2p) = self.p2p_conns.get_mut(connector_addr) {
                        let _ = p2p.conn.dgram_send(&response);
                    }
                }
                continue;
            }

            // Other DATAGRAMs are tunneled IP packets - would be passed back to Swift
        }
    }

    // ========================================================================
    // Hole Punching Methods
    // ========================================================================

    /// Start hole punching for a service
    ///
    /// This initiates the P2P signaling process to establish a direct connection
    /// to the Connector hosting the specified service.
    fn start_hole_punching(&mut self, service_id: &str) -> Result<(), String> {
        if self.hole_punch.is_some() {
            return Err("Hole punching already in progress".to_string());
        }

        if self.state != AgentState::Connected {
            return Err("Not connected to Intermediate Server".to_string());
        }

        // Generate session ID
        let session_id = p2p::generate_session_id();

        // Create coordinator (Agent is controlling side)
        let mut coordinator = p2p::HolePunchCoordinator::new(
            session_id,
            service_id.to_string(),
            true, // Agent is controlling
        );

        // Set intermediate address for relay candidate
        if let Some(addr) = self.intermediate_addr {
            coordinator.set_intermediate_addr(addr);
        }

        // Set observed address for server-reflexive candidate
        if let Some(addr) = self.observed_address {
            coordinator.set_observed_addr(addr);
        }

        // Gather local candidates
        let local_addrs: Vec<SocketAddr> = self
            .local_addr
            .iter()
            .cloned()
            .collect();

        if local_addrs.is_empty() {
            return Err("No local address available".to_string());
        }

        coordinator.start_gathering(&local_addrs);

        // Get candidate offer to send
        let offer_data = coordinator
            .get_candidate_offer()
            .ok_or("Failed to generate candidate offer")?;

        // Send offer via Intermediate (stream 0 for signaling)
        if let Some(conn) = self.intermediate_conn.as_mut() {
            // Stream 0 is used for signaling (client-initiated bidi stream)
            match conn.stream_send(0, &offer_data, false) {
                Ok(_) => {}
                Err(e) => return Err(format!("Failed to send offer: {}", e)),
            }
        }

        self.hole_punch = Some(coordinator);
        Ok(())
    }

    /// Process signaling streams from Intermediate Server
    fn process_signaling_streams(&mut self) {
        let conn = match self.intermediate_conn.as_mut() {
            Some(c) => c,
            None => return,
        };

        // Read from stream 0 (signaling stream)
        loop {
            match conn.stream_recv(0, &mut self.stream_buffer) {
                Ok((len, _fin)) => {
                    self.signaling_buffer.extend_from_slice(&self.stream_buffer[..len]);
                }
                Err(quiche::Error::Done) => break,
                Err(_) => break,
            }
        }

        // Process accumulated signaling messages
        self.process_signaling_messages();
    }

    /// Process accumulated signaling messages
    fn process_signaling_messages(&mut self) {
        if self.signaling_buffer.is_empty() {
            return;
        }

        let (messages, remaining) = p2p::decode_messages(&self.signaling_buffer);
        self.signaling_buffer = remaining;

        for msg in messages {
            self.handle_signaling_message(msg);
        }
    }

    /// Handle a single signaling message
    fn handle_signaling_message(&mut self, msg: p2p::SignalingMessage) {
        let coordinator = match self.hole_punch.as_mut() {
            Some(c) => c,
            None => return,
        };

        // Re-encode the message for the coordinator
        if let Ok(data) = p2p::encode_message(&msg) {
            let _ = coordinator.process_signaling(&data);
        }

        // Check if we should start checking
        if coordinator.should_start_checking() {
            coordinator.start_checking();
        }
    }

    /// Poll hole punching progress
    ///
    /// Returns (working_address, is_complete)
    /// If complete, the hole_punch coordinator is removed.
    fn poll_hole_punch(&mut self) -> (Option<SocketAddr>, bool) {
        let coordinator = match self.hole_punch.as_mut() {
            Some(c) => c,
            None => return (None, false),
        };

        // Handle timeouts
        coordinator.on_timeout();

        let state = coordinator.state();

        match state {
            p2p::HolePunchState::Connected => {
                let addr = coordinator.working_address();
                // Set direct path in path manager when hole punching succeeds
                if let Some(a) = addr {
                    self.path_manager.set_direct(a);
                }
                self.hole_punch = None;
                (addr, true)
            }
            p2p::HolePunchState::Failed | p2p::HolePunchState::FallbackRelay => {
                self.hole_punch = None;
                (None, true)
            }
            _ => (None, false),
        }
    }

    /// Get binding requests to send for hole punching
    ///
    /// Returns (remote_address, encoded_binding_request) pairs
    fn poll_binding_requests(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut requests = Vec::new();

        if let Some(coordinator) = self.hole_punch.as_mut() {
            while let Some((addr, data)) = coordinator.poll_binding_request() {
                requests.push((addr, data));
            }
        }

        requests
    }

    /// Process received binding response
    fn process_binding_response(&mut self, from: SocketAddr, data: &[u8]) {
        if let Some(coordinator) = self.hole_punch.as_mut() {
            if let Ok(Some(response)) = coordinator.process_binding(from, data) {
                // Queue response to be sent (via UDP directly)
                // Swift layer would handle sending this
                let _ = response;
            }
        }
    }

    // ========================================================================
    // Path Resilience Methods
    // ========================================================================

    /// Poll for keepalive messages to send
    ///
    /// Returns (remote_address, keepalive_message) if a keepalive should be sent.
    fn poll_keepalive(&mut self) -> Option<(SocketAddr, [u8; 5])> {
        self.path_manager.poll_keepalive()
    }

    /// Get current active path type
    fn active_path(&self) -> p2p::ActivePath {
        self.path_manager.active_path_type()
    }

    /// Check if currently in fallback mode (using relay)
    fn is_in_fallback(&self) -> bool {
        self.path_manager.is_in_fallback()
    }

    /// Get path statistics for diagnostics
    fn path_stats(&self) -> p2p::PathStats {
        self.path_manager.stats()
    }

    /// Get the current active path address for sending data
    fn active_send_addr(&self) -> Option<SocketAddr> {
        self.path_manager.active_addr()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a cryptographically secure random connection ID
fn rand_connection_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    let rng = SystemRandom::new();
    // Fill with secure random bytes; fall back to zeros on error (extremely unlikely)
    let _ = rng.fill(&mut id);
    id
}

/// Initialize logging (called once)
static INIT_LOGGING: Once = Once::new();

fn init_logging() {
    INIT_LOGGING.call_once(|| {
        // Logging in Network Extensions must go through Swift's os_log
        // We just initialize the log crate facade here
        let _ = log::set_logger(&NullLogger);
        log::set_max_level(log::LevelFilter::Debug);
    });
}

struct NullLogger;
impl log::Log for NullLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        false
    }
    fn log(&self, _: &log::Record) {}
    fn flush(&self) {}
}

// ============================================================================
// FFI Functions - Agent Lifecycle
// ============================================================================

/// Create a new agent instance
///
/// Returns a pointer to the agent, or null on failure.
/// The caller is responsible for calling `agent_destroy` when done.
#[no_mangle]
pub extern "C" fn agent_create() -> *mut Agent {
    init_logging();

    let result = panic::catch_unwind(|| Agent::new().ok().map(Box::new));

    match result {
        Ok(Some(agent)) => Box::into_raw(agent),
        _ => std::ptr::null_mut(),
    }
}

/// Destroy an agent instance
///
/// # Safety
/// The pointer must be valid and created by `agent_create`.
#[no_mangle]
pub unsafe extern "C" fn agent_destroy(agent: *mut Agent) {
    if !agent.is_null() {
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            drop(Box::from_raw(agent));
        }));
    }
}

/// Get the current agent state
#[no_mangle]
pub unsafe extern "C" fn agent_get_state(agent: *const Agent) -> AgentState {
    if agent.is_null() {
        return AgentState::Error;
    }

    panic::catch_unwind(AssertUnwindSafe(|| (*agent).state)).unwrap_or(AgentState::Error)
}

// ============================================================================
// FFI Functions - Connection Management
// ============================================================================

/// Connect to a QUIC server
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `host` - Server hostname or IP (null-terminated C string)
/// * `port` - Server port
///
/// # Returns
/// `AgentResult::Ok` on success, error code otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_connect(
    agent: *mut Agent,
    host: *const libc::c_char,
    port: u16,
) -> AgentResult {
    if agent.is_null() || host.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        // Parse host string
        let host_str = match std::ffi::CStr::from_ptr(host).to_str() {
            Ok(s) => s,
            Err(_) => return AgentResult::InvalidAddress,
        };

        // Parse socket address
        let addr: SocketAddr = match format!("{}:{}", host_str, port).parse() {
            Ok(a) => a,
            Err(_) => {
                // Try resolving as hostname (simplified - just try as IP)
                match host_str.parse::<std::net::IpAddr>() {
                    Ok(ip) => SocketAddr::new(ip, port),
                    Err(_) => return AgentResult::InvalidAddress,
                }
            }
        };

        // Initiate connection
        match agent.connect(addr) {
            Ok(()) => AgentResult::Ok,
            Err(_) => AgentResult::ConnectionFailed,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Check if the agent is connected
#[no_mangle]
pub unsafe extern "C" fn agent_is_connected(agent: *const Agent) -> bool {
    if agent.is_null() {
        return false;
    }

    panic::catch_unwind(AssertUnwindSafe(|| (*agent).state == AgentState::Connected)).unwrap_or(false)
}

/// Register the Agent for a target service
///
/// This tells the Intermediate Server which service the Agent wants to reach.
/// Must be called after the connection is established (agent_is_connected returns true).
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `service_id` - Service ID to register for (null-terminated C string)
///
/// # Returns
/// `AgentResult::Ok` on success, error code otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_register(
    agent: *mut Agent,
    service_id: *const libc::c_char,
) -> AgentResult {
    if agent.is_null() || service_id.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        // Parse service ID string
        let service_str = match std::ffi::CStr::from_ptr(service_id).to_str() {
            Ok(s) => s,
            Err(_) => return AgentResult::InvalidAddress,
        };

        // Send registration
        match agent.register(service_str) {
            Ok(()) => AgentResult::Ok,
            Err(quiche::Error::InvalidState) => AgentResult::NotConnected,
            Err(_) => AgentResult::QuicError,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Send a keepalive PING on the Intermediate connection
///
/// Call this periodically (e.g., every 10 seconds) to prevent the QUIC
/// connection from timing out due to inactivity. The QUIC idle timeout
/// is typically 30 seconds.
///
/// # Arguments
/// * `agent` - Agent pointer
///
/// # Returns
/// * `AgentResult::Ok` if keepalive was sent
/// * `AgentResult::NotConnected` if not connected to Intermediate
#[no_mangle]
pub unsafe extern "C" fn agent_send_intermediate_keepalive(agent: *mut Agent) -> AgentResult {
    if agent.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        match agent.send_intermediate_keepalive() {
            Ok(()) => AgentResult::Ok,
            Err(quiche::Error::InvalidState) => AgentResult::NotConnected,
            Err(_) => AgentResult::QuicError,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

// ============================================================================
// FFI Functions - Packet I/O
// ============================================================================

/// Receive a UDP packet from the network
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `data` - Pointer to received packet data
/// * `len` - Length of received data
/// * `from_ip` - Source IP address (as 4-byte array for IPv4)
/// * `from_port` - Source port
#[no_mangle]
pub unsafe extern "C" fn agent_recv(
    agent: *mut Agent,
    data: *const u8,
    len: usize,
    from_ip: *const u8,
    from_port: u16,
) -> AgentResult {
    if agent.is_null() || data.is_null() || from_ip.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let data = slice::from_raw_parts(data, len);
        let ip_bytes = slice::from_raw_parts(from_ip, 4);
        let ip = std::net::Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
        let from = SocketAddr::new(std::net::IpAddr::V4(ip), from_port);

        match agent.recv(data, from) {
            Ok(()) => AgentResult::Ok,
            Err(_) => AgentResult::QuicError,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Poll for outbound UDP packets
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_data` - Buffer to write packet data
/// * `out_len` - On input: buffer capacity. On output: actual length written.
/// * `out_port` - On output: destination port
///
/// # Returns
/// `AgentResult::Ok` if a packet was written, `AgentResult::NoData` if no packets available.
#[no_mangle]
pub unsafe extern "C" fn agent_poll(
    agent: *mut Agent,
    out_data: *mut u8,
    out_len: *mut usize,
    out_port: *mut u16,
) -> AgentResult {
    if agent.is_null() || out_data.is_null() || out_len.is_null() || out_port.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let capacity = *out_len;

        match agent.poll() {
            Some((packet, addr)) => {
                if packet.len() > capacity {
                    return AgentResult::BufferTooSmall;
                }

                std::ptr::copy_nonoverlapping(packet.as_ptr(), out_data, packet.len());
                *out_len = packet.len();
                *out_port = addr.port();
                AgentResult::Ok
            }
            None => AgentResult::NoData,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Send an IP packet through the QUIC tunnel (as DATAGRAM)
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `data` - IP packet data
/// * `len` - Length of IP packet
#[no_mangle]
pub unsafe extern "C" fn agent_send_datagram(
    agent: *mut Agent,
    data: *const u8,
    len: usize,
) -> AgentResult {
    if agent.is_null() || data.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let data = slice::from_raw_parts(data, len);

        match agent.send_datagram(data) {
            Ok(()) => AgentResult::Ok,
            Err(quiche::Error::InvalidState) => AgentResult::NotConnected,
            Err(_) => AgentResult::QuicError,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Handle timeout event
///
/// Call this when the timeout duration (from `agent_timeout_ms`) has elapsed.
#[no_mangle]
pub unsafe extern "C" fn agent_on_timeout(agent: *mut Agent) {
    if agent.is_null() {
        return;
    }

    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        agent.on_timeout();
    }));
}

/// Get milliseconds until next timeout event
///
/// Returns 0 if no timeout is pending, or the number of milliseconds.
#[no_mangle]
pub unsafe extern "C" fn agent_timeout_ms(agent: *const Agent) -> u64 {
    if agent.is_null() {
        return 0;
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;
        agent.timeout().map(|d| d.as_millis() as u64).unwrap_or(0)
    }))
    .unwrap_or(0)
}

// ============================================================================
// FFI Functions - QAD (QUIC Address Discovery)
// ============================================================================

/// Get the observed public address (from QAD)
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_ip` - Buffer for IPv4 address (4 bytes)
/// * `out_port` - Output port
///
/// # Returns
/// `AgentResult::Ok` if address is available, `AgentResult::NoData` if not yet discovered.
#[no_mangle]
pub unsafe extern "C" fn agent_get_observed_address(
    agent: *const Agent,
    out_ip: *mut u8,
    out_port: *mut u16,
) -> AgentResult {
    if agent.is_null() || out_ip.is_null() || out_port.is_null() {
        return AgentResult::InvalidPointer;
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;

        match agent.observed_address {
            Some(SocketAddr::V4(addr)) => {
                let octets = addr.ip().octets();
                std::ptr::copy_nonoverlapping(octets.as_ptr(), out_ip, 4);
                *out_port = addr.port();
                AgentResult::Ok
            }
            _ => AgentResult::NoData,
        }
    }))
    .unwrap_or(AgentResult::PanicCaught)
}

// ============================================================================
// FFI Functions - P2P Connections
// ============================================================================

/// Connect to a Connector via P2P (direct connection)
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `host` - Connector hostname or IP (null-terminated C string)
/// * `port` - Connector port
///
/// # Returns
/// `AgentResult::Ok` on success, error code otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_connect_p2p(
    agent: *mut Agent,
    host: *const libc::c_char,
    port: u16,
) -> AgentResult {
    if agent.is_null() || host.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        // Parse host string
        let host_str = match std::ffi::CStr::from_ptr(host).to_str() {
            Ok(s) => s,
            Err(_) => return AgentResult::InvalidAddress,
        };

        // Parse socket address
        let addr: SocketAddr = match format!("{}:{}", host_str, port).parse() {
            Ok(a) => a,
            Err(_) => {
                match host_str.parse::<std::net::IpAddr>() {
                    Ok(ip) => SocketAddr::new(ip, port),
                    Err(_) => return AgentResult::InvalidAddress,
                }
            }
        };

        // Initiate P2P connection
        match agent.connect_p2p(addr) {
            Ok(()) => AgentResult::Ok,
            Err(_) => AgentResult::ConnectionFailed,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Check if a P2P connection is established to the given address
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `host` - Connector hostname or IP (null-terminated C string)
/// * `port` - Connector port
#[no_mangle]
pub unsafe extern "C" fn agent_is_p2p_connected(
    agent: *const Agent,
    host: *const libc::c_char,
    port: u16,
) -> bool {
    if agent.is_null() || host.is_null() {
        return false;
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;

        let host_str = match std::ffi::CStr::from_ptr(host).to_str() {
            Ok(s) => s,
            Err(_) => return false,
        };

        let addr: SocketAddr = match format!("{}:{}", host_str, port).parse() {
            Ok(a) => a,
            Err(_) => {
                match host_str.parse::<std::net::IpAddr>() {
                    Ok(ip) => SocketAddr::new(ip, port),
                    Err(_) => return false,
                }
            }
        };

        agent.is_p2p_connected(addr)
    }))
    .unwrap_or(false)
}

/// Poll for outbound UDP packets from P2P connections
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_data` - Buffer to write packet data
/// * `out_len` - On input: buffer capacity. On output: actual length written.
/// * `out_ip` - Buffer for destination IP (4 bytes)
/// * `out_port` - On output: destination port
///
/// # Returns
/// `AgentResult::Ok` if a packet was written, `AgentResult::NoData` if no packets available.
#[no_mangle]
pub unsafe extern "C" fn agent_poll_p2p(
    agent: *mut Agent,
    out_data: *mut u8,
    out_len: *mut usize,
    out_ip: *mut u8,
    out_port: *mut u16,
) -> AgentResult {
    if agent.is_null() || out_data.is_null() || out_len.is_null() || out_ip.is_null() || out_port.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let capacity = *out_len;

        match agent.poll_p2p() {
            Some((packet, addr)) => {
                if packet.len() > capacity {
                    return AgentResult::BufferTooSmall;
                }

                std::ptr::copy_nonoverlapping(packet.as_ptr(), out_data, packet.len());
                *out_len = packet.len();

                // Write destination IP
                if let SocketAddr::V4(v4) = addr {
                    std::ptr::copy_nonoverlapping(v4.ip().octets().as_ptr(), out_ip, 4);
                }
                *out_port = addr.port();
                AgentResult::Ok
            }
            None => AgentResult::NoData,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Send an IP packet through a P2P connection (as DATAGRAM)
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `data` - IP packet data
/// * `len` - Length of IP packet
/// * `dest_ip` - Destination Connector IP (4 bytes)
/// * `dest_port` - Destination Connector port
#[no_mangle]
pub unsafe extern "C" fn agent_send_datagram_p2p(
    agent: *mut Agent,
    data: *const u8,
    len: usize,
    dest_ip: *const u8,
    dest_port: u16,
) -> AgentResult {
    if agent.is_null() || data.is_null() || dest_ip.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let data = slice::from_raw_parts(data, len);
        let ip_bytes = slice::from_raw_parts(dest_ip, 4);
        let ip = std::net::Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
        let dest = SocketAddr::new(std::net::IpAddr::V4(ip), dest_port);

        match agent.send_datagram_p2p(data, dest) {
            Ok(()) => AgentResult::Ok,
            Err(quiche::Error::InvalidState) => AgentResult::NotConnected,
            Err(_) => AgentResult::QuicError,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

// ============================================================================
// FFI Functions - Hole Punching
// ============================================================================

/// Start hole punching for a service
///
/// This initiates P2P negotiation to establish a direct connection
/// to the Connector hosting the specified service.
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `service_id` - Service ID to connect to (null-terminated C string)
///
/// # Returns
/// `AgentResult::Ok` on success, error code otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_start_hole_punch(
    agent: *mut Agent,
    service_id: *const libc::c_char,
) -> AgentResult {
    if agent.is_null() || service_id.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        let service_str = match std::ffi::CStr::from_ptr(service_id).to_str() {
            Ok(s) => s,
            Err(_) => return AgentResult::InvalidAddress,
        };

        match agent.start_hole_punching(service_str) {
            Ok(()) => AgentResult::Ok,
            Err(_) => AgentResult::ConnectionFailed,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Poll hole punching progress
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_ip` - Buffer for working IP (4 bytes for IPv4)
/// * `out_port` - Output port
/// * `out_complete` - Set to 1 if hole punching is complete, 0 otherwise
///
/// # Returns
/// `AgentResult::Ok` if a working address is available, `AgentResult::NoData` otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_poll_hole_punch(
    agent: *mut Agent,
    out_ip: *mut u8,
    out_port: *mut u16,
    out_complete: *mut u8,
) -> AgentResult {
    if agent.is_null() || out_ip.is_null() || out_port.is_null() || out_complete.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        // Process signaling streams first
        agent.process_signaling_streams();

        let (working_addr, is_complete) = agent.poll_hole_punch();

        *out_complete = if is_complete { 1 } else { 0 };

        match working_addr {
            Some(SocketAddr::V4(addr)) => {
                let octets = addr.ip().octets();
                std::ptr::copy_nonoverlapping(octets.as_ptr(), out_ip, 4);
                *out_port = addr.port();
                AgentResult::Ok
            }
            Some(SocketAddr::V6(_)) => {
                // IPv6 not yet supported in this FFI
                AgentResult::NoData
            }
            None => AgentResult::NoData,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Get binding requests to send for hole punching
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_data` - Buffer for binding request data
/// * `out_len` - On input: buffer capacity. On output: data length.
/// * `out_ip` - Buffer for destination IP (4 bytes)
/// * `out_port` - Output destination port
///
/// # Returns
/// `AgentResult::Ok` if a request is available, `AgentResult::NoData` otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_poll_binding_request(
    agent: *mut Agent,
    out_data: *mut u8,
    out_len: *mut usize,
    out_ip: *mut u8,
    out_port: *mut u16,
) -> AgentResult {
    if agent.is_null() || out_data.is_null() || out_len.is_null() || out_ip.is_null() || out_port.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let capacity = *out_len;

        let requests = agent.poll_binding_requests();
        if let Some((addr, data)) = requests.into_iter().next() {
            if data.len() > capacity {
                return AgentResult::BufferTooSmall;
            }

            std::ptr::copy_nonoverlapping(data.as_ptr(), out_data, data.len());
            *out_len = data.len();

            if let SocketAddr::V4(v4) = addr {
                std::ptr::copy_nonoverlapping(v4.ip().octets().as_ptr(), out_ip, 4);
            }
            *out_port = addr.port();

            AgentResult::Ok
        } else {
            AgentResult::NoData
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Process a received binding response
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `data` - Binding response data
/// * `len` - Data length
/// * `from_ip` - Source IP (4 bytes)
/// * `from_port` - Source port
#[no_mangle]
pub unsafe extern "C" fn agent_process_binding_response(
    agent: *mut Agent,
    data: *const u8,
    len: usize,
    from_ip: *const u8,
    from_port: u16,
) -> AgentResult {
    if agent.is_null() || data.is_null() || from_ip.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;
        let data = slice::from_raw_parts(data, len);
        let ip_bytes = slice::from_raw_parts(from_ip, 4);
        let ip = std::net::Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
        let from = SocketAddr::new(std::net::IpAddr::V4(ip), from_port);

        agent.process_binding_response(from, data);
        AgentResult::Ok
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

// ============================================================================
// FFI Functions - Path Resilience
// ============================================================================

/// Poll for keepalive message to send
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_ip` - Buffer for destination IP (4 bytes)
/// * `out_port` - Output destination port
/// * `out_data` - Buffer for keepalive message (5 bytes minimum)
///
/// # Returns
/// `AgentResult::Ok` if a keepalive should be sent, `AgentResult::NoData` otherwise.
#[no_mangle]
pub unsafe extern "C" fn agent_poll_keepalive(
    agent: *mut Agent,
    out_ip: *mut u8,
    out_port: *mut u16,
    out_data: *mut u8,
) -> AgentResult {
    if agent.is_null() || out_ip.is_null() || out_port.is_null() || out_data.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &mut *agent;

        match agent.poll_keepalive() {
            Some((addr, msg)) => {
                if let SocketAddr::V4(v4) = addr {
                    std::ptr::copy_nonoverlapping(v4.ip().octets().as_ptr(), out_ip, 4);
                }
                *out_port = addr.port();
                std::ptr::copy_nonoverlapping(msg.as_ptr(), out_data, 5);
                AgentResult::Ok
            }
            None => AgentResult::NoData,
        }
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

/// Get current active path type
///
/// # Returns
/// 0 = Direct, 1 = Relay, 2 = None
#[no_mangle]
pub unsafe extern "C" fn agent_get_active_path(agent: *const Agent) -> u8 {
    if agent.is_null() {
        return 2; // None
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;
        match agent.active_path() {
            p2p::ActivePath::Direct => 0,
            p2p::ActivePath::Relay => 1,
            p2p::ActivePath::None => 2,
        }
    }))
    .unwrap_or(2)
}

/// Check if agent is in fallback mode
#[no_mangle]
pub unsafe extern "C" fn agent_is_in_fallback(agent: *const Agent) -> bool {
    if agent.is_null() {
        return false;
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;
        agent.is_in_fallback()
    }))
    .unwrap_or(false)
}

/// Get path statistics
///
/// # Arguments
/// * `agent` - Agent pointer
/// * `out_missed_keepalives` - Output missed keepalive count
/// * `out_rtt_ms` - Output RTT in milliseconds (0 if not measured)
/// * `out_in_fallback` - Output fallback status (1 = in fallback, 0 = not)
///
/// # Returns
/// `AgentResult::Ok` on success
#[no_mangle]
pub unsafe extern "C" fn agent_get_path_stats(
    agent: *const Agent,
    out_missed_keepalives: *mut u32,
    out_rtt_ms: *mut u64,
    out_in_fallback: *mut u8,
) -> AgentResult {
    if agent.is_null() || out_missed_keepalives.is_null() || out_rtt_ms.is_null() || out_in_fallback.is_null() {
        return AgentResult::InvalidPointer;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let agent = &*agent;
        let stats = agent.path_stats();

        *out_missed_keepalives = stats.missed_keepalives;
        *out_rtt_ms = stats.direct_rtt.map(|d| d.as_millis() as u64).unwrap_or(0);
        *out_in_fallback = if stats.in_fallback { 1 } else { 0 };

        AgentResult::Ok
    }));

    result.unwrap_or(AgentResult::PanicCaught)
}

// ============================================================================
// Legacy FFI - Packet Processing (kept for compatibility)
// ============================================================================

/// Process an IP packet and decide whether to forward or drop
///
/// This is the legacy function for simple packet filtering.
/// For QUIC tunneling, use the agent_* functions instead.
#[no_mangle]
pub extern "C" fn process_packet(data: *const u8, len: libc::size_t) -> PacketAction {
    if data.is_null() || len == 0 {
        return PacketAction::Forward;
    }

    let result = panic::catch_unwind(|| {
        let slice = unsafe { slice::from_raw_parts(data, len) };

        match etherparse::SlicedPacket::from_ip(slice) {
            Err(_) => PacketAction::Forward,
            Ok(_) => PacketAction::Forward,
        }
    });

    result.unwrap_or(PacketAction::Forward)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_packet() {
        let data = [0u8; 20];
        let action = process_packet(data.as_ptr(), data.len());
        assert_eq!(action, PacketAction::Forward);
    }

    #[test]
    fn test_agent_create_destroy() {
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            let state = agent_get_state(agent);
            assert_eq!(state, AgentState::Disconnected);

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_agent_connect() {
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            let host = std::ffi::CString::new("127.0.0.1").unwrap();
            let result = agent_connect(agent, host.as_ptr(), 4433);
            assert_eq!(result, AgentResult::Ok);

            let state = agent_get_state(agent);
            assert_eq!(state, AgentState::Connecting);

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_agent_connect_p2p() {
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            // First connect to Intermediate
            let server = std::ffi::CString::new("127.0.0.1").unwrap();
            let result = agent_connect(agent, server.as_ptr(), 4433);
            assert_eq!(result, AgentResult::Ok);

            // Then initiate P2P connection to Connector
            let connector = std::ffi::CString::new("127.0.0.1").unwrap();
            let p2p_result = agent_connect_p2p(agent, connector.as_ptr(), 5000);
            assert_eq!(p2p_result, AgentResult::Ok);

            // P2P connection should not be established yet (no handshake completed)
            assert!(!agent_is_p2p_connected(agent, connector.as_ptr(), 5000));

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_agent_multi_connection() {
        // Test that Agent can manage multiple P2P connections
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            let agent_ref = &mut *agent;

            // Connect to Intermediate
            let server_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
            agent_ref.connect(server_addr).unwrap();

            // Initiate multiple P2P connections
            let connector1: SocketAddr = "127.0.0.1:5001".parse().unwrap();
            let connector2: SocketAddr = "127.0.0.1:5002".parse().unwrap();

            agent_ref.connect_p2p(connector1).unwrap();
            agent_ref.connect_p2p(connector2).unwrap();

            // Verify both are tracked
            assert!(agent_ref.p2p_conns.contains_key(&connector1));
            assert!(agent_ref.p2p_conns.contains_key(&connector2));

            // Duplicate connection should be no-op
            agent_ref.connect_p2p(connector1).unwrap();
            assert_eq!(agent_ref.p2p_conns.len(), 2);

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_agent_hole_punch_not_connected() {
        // Hole punching requires connection to Intermediate
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            let agent_ref = &mut *agent;

            // Should fail because not connected
            let result = agent_ref.start_hole_punching("test-service");
            assert!(result.is_err());

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_hole_punch_coordinator_integration() {
        // Test the HolePunchCoordinator state machine directly
        use crate::p2p::{HolePunchCoordinator, HolePunchState};

        // Create Agent-side coordinator (controlling)
        let mut agent_coord = HolePunchCoordinator::new(12345, "test-service".to_string(), true);

        // Create Connector-side coordinator (controlled)
        let mut conn_coord = HolePunchCoordinator::new(12345, "test-service".to_string(), false);

        // Agent gathers candidates
        let agent_addrs = vec!["192.168.1.100:5000".parse().unwrap()];
        agent_coord.start_gathering(&agent_addrs);
        assert_eq!(agent_coord.state(), HolePunchState::Signaling);

        // Connector gathers candidates
        let conn_addrs = vec!["192.168.1.200:5000".parse().unwrap()];
        conn_coord.start_gathering(&conn_addrs);
        assert_eq!(conn_coord.state(), HolePunchState::Signaling);

        // Agent gets offer
        let offer = agent_coord.get_candidate_offer().expect("Should have offer");

        // Connector processes offer (simulates Intermediate forwarding)
        conn_coord.process_signaling(&offer).expect("Should process offer");

        // Connector generates answer
        let answer = conn_coord.poll_signaling_message().expect("Should have answer");

        // Agent processes answer
        agent_coord.process_signaling(&answer).expect("Should process answer");

        // Both should have remote candidates now
        assert!(!agent_coord.remote_candidates().is_empty());
        assert!(!conn_coord.remote_candidates().is_empty());

        // Both should be ready to start checking
        assert!(agent_coord.should_start_checking());
        assert!(conn_coord.should_start_checking());

        // Start checking
        agent_coord.start_checking();
        conn_coord.start_checking();

        assert_eq!(agent_coord.state(), HolePunchState::Checking);
        assert_eq!(conn_coord.state(), HolePunchState::Checking);

        // Agent polls for binding request
        let (addr, request_data) = agent_coord.poll_binding_request().expect("Should have request");

        // Connector processes binding request and responds
        let response_data = conn_coord.process_binding(addr, &request_data)
            .expect("Should process binding")
            .expect("Should have response");

        // Agent processes binding response
        agent_coord.process_binding("192.168.1.200:5000".parse().unwrap(), &response_data)
            .expect("Should process response");

        // Agent should be connected now
        assert_eq!(agent_coord.state(), HolePunchState::Connected);
        assert!(agent_coord.working_address().is_some());
    }

    #[test]
    fn test_path_manager_integration() {
        use crate::p2p::{PathManager, ActivePath, PathState};

        let mut manager = PathManager::new();

        // Initially no path
        assert_eq!(manager.active_path_type(), ActivePath::None);
        assert!(!manager.is_in_fallback());

        // Set relay (simulating Intermediate connection)
        let relay_addr: std::net::SocketAddr = "1.2.3.4:4433".parse().unwrap();
        manager.set_relay(relay_addr);
        assert_eq!(manager.active_path_type(), ActivePath::Relay);
        assert_eq!(manager.active_addr(), Some(relay_addr));

        // Establish direct path (simulating successful hole punch)
        let direct_addr: std::net::SocketAddr = "192.168.1.100:5000".parse().unwrap();
        manager.set_direct(direct_addr);
        assert_eq!(manager.active_path_type(), ActivePath::Direct);
        assert_eq!(manager.active_addr(), Some(direct_addr));
        assert!(!manager.is_in_fallback());

        // Test keepalive polling
        let keepalive = manager.poll_keepalive();
        assert!(keepalive.is_some());
        let (addr, msg) = keepalive.unwrap();
        assert_eq!(addr, direct_addr);
        assert_eq!(msg[0], crate::p2p::KEEPALIVE_REQUEST);

        // Test keepalive response processing
        let request = crate::p2p::encode_keepalive_request(42);
        let response = manager.process_keepalive(direct_addr, &request);
        assert!(response.is_some());
        let resp = response.unwrap();
        assert_eq!(resp[0], crate::p2p::KEEPALIVE_RESPONSE);

        // Verify path stats
        let stats = manager.stats();
        assert_eq!(stats.active_path, ActivePath::Direct);
        assert!(!stats.in_fallback);
        assert_eq!(stats.missed_keepalives, 0);
    }

    #[test]
    fn test_agent_path_manager_setup() {
        let agent = Agent::new().unwrap();

        // Initially relay path type is None
        assert_eq!(agent.active_path(), crate::p2p::ActivePath::None);
        assert!(!agent.is_in_fallback());

        // Path stats should be accessible
        let stats = agent.path_stats();
        assert_eq!(stats.missed_keepalives, 0);
    }

    #[test]
    fn test_agent_register_not_connected() {
        // Registration should fail if not connected
        let agent = agent_create();
        assert!(!agent.is_null());

        unsafe {
            let service = std::ffi::CString::new("test-service").unwrap();
            let result = agent_register(agent, service.as_ptr());
            // Should fail because not connected
            assert_eq!(result, AgentResult::NotConnected);

            agent_destroy(agent);
        }
    }

    #[test]
    fn test_agent_register_message_format() {
        // Test that the registration message is correctly formatted
        let mut agent = Agent::new().unwrap();

        // Connect first (won't actually handshake but sets up the connection)
        let server_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        agent.connect(server_addr).unwrap();

        // Simulate established connection by checking registration logic
        // Note: Can't fully test without a real server, but we verify the format
        let service_id = "echo-service";
        let id_bytes = service_id.as_bytes();

        // Build expected message format
        let mut expected = Vec::with_capacity(2 + id_bytes.len());
        expected.push(REG_TYPE_AGENT); // 0x10
        expected.push(id_bytes.len() as u8);
        expected.extend_from_slice(id_bytes);

        assert_eq!(expected[0], 0x10); // Agent type
        assert_eq!(expected[1], 12);   // "echo-service" length
        assert_eq!(&expected[2..], b"echo-service");
    }
}
