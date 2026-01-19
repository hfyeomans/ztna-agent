//! ZTNA Agent Packet Processor
//!
//! This crate provides the Rust core for the ZTNA agent, handling:
//! - IP packet processing and filtering
//! - QUIC tunnel management via quiche
//! - FFI interface for Swift integration

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::panic::{self, AssertUnwindSafe};
use std::slice;
use std::sync::Once;
use std::time::{Duration, Instant};

use quiche::{Config, Connection, ConnectionId};

// ============================================================================
// Constants
// ============================================================================

/// Maximum UDP payload size for QUIC packets
const MAX_DATAGRAM_SIZE: usize = 1350;

/// Maximum number of outbound packets to queue
const MAX_OUTBOUND_QUEUE: usize = 1024;

/// QUIC idle timeout in milliseconds
const IDLE_TIMEOUT_MS: u64 = 30000;

/// ALPN protocol identifier for ZTNA
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

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
// Outbound Packet Structure
// ============================================================================

/// Represents a UDP packet to be sent by Swift
#[repr(C)]
pub struct OutboundPacket {
    /// Pointer to packet data (valid until next agent_poll call)
    pub data: *const u8,
    /// Length of packet data
    pub len: usize,
    /// Destination port (host byte order)
    pub dst_port: u16,
}

// ============================================================================
// Agent Structure
// ============================================================================

/// QUIC tunnel agent state
pub struct Agent {
    /// QUIC configuration
    config: Config,
    /// QUIC connection (None until connect is called)
    conn: Option<Connection>,
    /// Server address
    server_addr: Option<SocketAddr>,
    /// Local address (set after first recv)
    local_addr: Option<SocketAddr>,
    /// Connection state
    state: AgentState,
    /// Outbound UDP packet queue
    outbound_queue: VecDeque<Vec<u8>>,
    /// Buffer for current outbound packet (for FFI pointer stability)
    current_outbound: Option<Vec<u8>>,
    /// Last activity time for timeout tracking
    last_activity: Instant,
    /// Observed public address from QAD (set by server)
    pub observed_address: Option<SocketAddr>,
    /// Scratch buffer for packet assembly
    scratch_buffer: Vec<u8>,
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
            conn: None,
            server_addr: None,
            local_addr: None,
            state: AgentState::Disconnected,
            outbound_queue: VecDeque::with_capacity(MAX_OUTBOUND_QUEUE),
            current_outbound: None,
            last_activity: Instant::now(),
            observed_address: None,
            scratch_buffer: vec![0u8; MAX_DATAGRAM_SIZE],
        })
    }

    /// Initiate connection to server
    fn connect(&mut self, server_addr: SocketAddr) -> Result<(), quiche::Error> {
        // Generate random connection ID
        let scid_bytes = rand_connection_id();
        let scid = ConnectionId::from_ref(&scid_bytes);

        // Create QUIC connection
        let conn = quiche::connect(
            Some("ztna-server"), // SNI
            &scid,
            self.local_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
            server_addr,
            &mut self.config,
        )?;

        self.conn = Some(conn);
        self.server_addr = Some(server_addr);
        self.state = AgentState::Connecting;
        self.last_activity = Instant::now();

        Ok(())
    }

    /// Process received UDP packet (from network)
    fn recv(&mut self, data: &[u8], from: SocketAddr) -> Result<(), quiche::Error> {
        // Update local address if not set
        if self.local_addr.is_none() {
            // We don't know our local addr from recv, but we can track the server
        }

        let conn = self.conn.as_mut().ok_or(quiche::Error::InvalidState)?;

        // Create recv info
        let recv_info = quiche::RecvInfo {
            from,
            to: self.local_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
        };

        // Feed data to QUIC connection (quiche requires mutable buffer for in-place decryption)
        let mut buf = data.to_vec();
        conn.recv(&mut buf, recv_info)?;

        // Update state based on connection state
        self.update_state();
        self.last_activity = Instant::now();

        // Process any received DATAGRAMs (could contain QAD info)
        self.process_incoming_datagrams();

        Ok(())
    }

    /// Get next outbound UDP packet to send
    fn poll(&mut self) -> Option<(Vec<u8>, SocketAddr)> {
        let conn = self.conn.as_mut()?;
        let server_addr = self.server_addr?;

        // Try to generate a QUIC packet
        let mut out = vec![0u8; MAX_DATAGRAM_SIZE];

        match conn.send(&mut out) {
            Ok((len, _send_info)) => {
                out.truncate(len);
                self.last_activity = Instant::now();
                Some((out, server_addr))
            }
            Err(quiche::Error::Done) => {
                // Check queued outbound packets
                self.outbound_queue.pop_front().map(|pkt| (pkt, server_addr))
            }
            Err(_) => None,
        }
    }

    /// Queue an IP packet for sending via DATAGRAM
    fn send_datagram(&mut self, data: &[u8]) -> Result<(), quiche::Error> {
        let conn = self.conn.as_mut().ok_or(quiche::Error::InvalidState)?;

        if !conn.is_established() {
            return Err(quiche::Error::InvalidState);
        }

        // Send as QUIC DATAGRAM
        conn.dgram_send(data)?;
        self.last_activity = Instant::now();

        Ok(())
    }

    /// Handle timeout - call periodically
    fn on_timeout(&mut self) {
        if let Some(conn) = self.conn.as_mut() {
            conn.on_timeout();
            self.update_state();
        }
    }

    /// Get time until next timeout event
    fn timeout(&self) -> Option<Duration> {
        self.conn.as_ref().and_then(|c| c.timeout())
    }

    /// Update agent state based on QUIC connection state
    fn update_state(&mut self) {
        if let Some(conn) = &self.conn {
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

    /// Process incoming DATAGRAM frames
    fn process_incoming_datagrams(&mut self) {
        let conn = match self.conn.as_mut() {
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
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a random connection ID
fn rand_connection_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    // Simple PRNG using system time - not cryptographically secure but sufficient for conn IDs
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for (i, byte) in id.iter_mut().enumerate() {
        *byte = ((seed >> (i * 8)) & 0xFF) as u8;
    }
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
}
