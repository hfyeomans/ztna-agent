//! ZTNA Intermediate Server
//!
//! A QUIC server that:
//! - Accepts connections from Agents and App Connectors
//! - Implements QAD (QUIC Address Discovery)
//! - Relays DATAGRAM frames between matched pairs

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::Deserialize;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::aead;
use ring::rand::{SecureRandom, SystemRandom};

mod auth;
mod client;
mod qad;
mod registry;
mod signaling;

use client::{Client, ClientType};
use registry::Registry;
use signaling::{
    decode_message, encode_message, DecodeError, SessionManager, SessionState, SignalingError,
    SignalingMessage, PUNCH_START_DELAY_MS,
};

// ============================================================================
// Type Aliases
// ============================================================================

/// Session data collected for a ready-to-punch P2P session:
/// (session_id, agent_conn_id, connector_conn_id, agent_candidates, connector_candidates)
type ReadySession = (
    u64,
    quiche::ConnectionId<'static>,
    quiche::ConnectionId<'static>,
    Vec<signaling::Candidate>,
    Vec<signaling::Candidate>,
);

// ============================================================================
// Constants
// ============================================================================

/// Maximum UDP payload size for QUIC packets (must match Agent)
const MAX_DATAGRAM_SIZE: usize = 1350;

/// QUIC idle timeout in milliseconds (must match Agent)
const IDLE_TIMEOUT_MS: u64 = 30_000;

/// ALPN protocol identifier (CRITICAL: must match Agent at lib.rs:28)
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// Default server port
const DEFAULT_PORT: u16 = 4433;

/// mio token for the UDP socket
const SOCKET_TOKEN: Token = Token(0);

/// 7B: Maximum age for retry tokens (seconds)
const RETRY_TOKEN_MAX_AGE_SECS: u64 = 60;

/// 8A.1: Registration ACK — server sends after successful registration
const REG_TYPE_ACK: u8 = 0x12;

/// 8A.1: Registration NACK — server sends on auth denial or invalid registration
const REG_TYPE_NACK: u8 = 0x13;

/// 8B.1: Connection ID rotation interval in seconds (default: 5 minutes)
const CID_ROTATION_INTERVAL_SECS: u64 = 300;

// ============================================================================
// Configuration
// ============================================================================

#[derive(Deserialize, Default)]
struct ServerConfig {
    port: Option<u16>,
    bind_addr: Option<String>,
    external_ip: Option<String>,
    cert_path: Option<String>,
    key_path: Option<String>,
    ca_cert_path: Option<String>,
    verify_peer: Option<bool>,
    require_client_cert: Option<bool>,
    disable_retry: Option<bool>,
}

fn load_config(path: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let config: ServerConfig = serde_json::from_str(&contents)?;
    log::info!("Loaded config from {}", path);
    Ok(config)
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

    // Load config file: --config <path>, or try default paths
    let config = if let Some(config_path) = parse_arg(&args, "--config") {
        load_config(&config_path)?
    } else {
        let default_paths = ["/etc/ztna/intermediate.json", "intermediate.json"];
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

    // Build effective config: named flags > positional args > config file > defaults
    // Named flags (--port, --cert, etc.) take priority over positional args and config file.
    // Positional args are supported for backwards compatibility with existing systemd services.
    let port: u16 = parse_arg(&args, "--port")
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            // Positional: first non-flag arg
            args.get(1)
                .filter(|a| !a.starts_with("--"))
                .and_then(|s| s.parse().ok())
        })
        .or(config.port)
        .unwrap_or(DEFAULT_PORT);

    let cert_path = parse_arg(&args, "--cert")
        .or_else(|| args.get(2).filter(|a| !a.starts_with("--")).cloned())
        .or(config.cert_path)
        .unwrap_or_else(|| "certs/cert.pem".to_string());

    let key_path = parse_arg(&args, "--key")
        .or_else(|| args.get(3).filter(|a| !a.starts_with("--")).cloned())
        .or(config.key_path)
        .unwrap_or_else(|| "certs/key.pem".to_string());

    let bind_addr = parse_arg(&args, "--bind")
        .or_else(|| args.get(4).filter(|a| !a.starts_with("--")).cloned())
        .or(config.bind_addr)
        .unwrap_or_else(|| "0.0.0.0".to_string());

    let external_ip = parse_arg(&args, "--external-ip")
        .or_else(|| args.get(5).filter(|a| !a.starts_with("--")).cloned())
        .or(config.external_ip);

    let ca_cert_path = parse_arg(&args, "--ca-cert").or(config.ca_cert_path);

    // C1: TLS peer verification enabled by default. Use --no-verify-peer for dev only.
    let verify_peer = if args.iter().any(|a| a == "--no-verify-peer") {
        false
    } else {
        config.verify_peer.unwrap_or(true)
    };

    // 6A: mTLS client certificate requirement (default: false for backward compat)
    let require_client_cert = if args.iter().any(|a| a == "--require-client-cert") {
        true
    } else {
        config.require_client_cert.unwrap_or(false)
    };

    // 7B.4: Stateless retry tokens (enabled by default, --disable-retry for dev/testing)
    let enable_retry = if args.iter().any(|a| a == "--disable-retry") {
        false
    } else {
        !config.disable_retry.unwrap_or(false)
    };

    // L2: Validate cert/key paths exist at startup
    if !Path::new(&cert_path).exists() {
        log::error!("Certificate file not found: {}", cert_path);
        return Err(format!("Certificate file not found: {}", cert_path).into());
    }
    if !Path::new(&key_path).exists() {
        log::error!("Private key file not found: {}", key_path);
        return Err(format!("Private key file not found: {}", key_path).into());
    }

    log::info!("ZTNA Intermediate Server starting...");
    log::info!("  Port: {}", port);
    log::info!("  Bind: {}", bind_addr);
    if let Some(ref ext_ip) = external_ip {
        log::info!("  External IP: {}", ext_ip);
    }
    log::info!("  Cert: {}", cert_path);
    log::info!("  Key:  {}", key_path);
    log::info!("  ALPN: {:?}", std::str::from_utf8(ALPN_PROTOCOL));
    log::info!("  Verify peer: {}", verify_peer);
    log::info!("  Require client cert: {}", require_client_cert);
    log::info!("  Stateless retry: {}", enable_retry);
    if !verify_peer {
        log::warn!("TLS peer verification DISABLED — do not use in production");
    }
    if require_client_cert {
        log::info!("mTLS client authentication ENABLED — connections without valid client certs will be rejected");
    }

    // 6B.1: Register SIGHUP handler for certificate hot-reload
    let reload_flag = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    {
        signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&reload_flag))?;
        log::info!("SIGHUP handler registered for certificate hot-reload");
    }

    // Create server and run
    let mut server = Server::new(
        port,
        &bind_addr,
        external_ip.as_deref(),
        &cert_path,
        &key_path,
        ca_cert_path.as_deref(),
        verify_peer,
        require_client_cert,
        reload_flag,
        enable_retry,
    )?;
    server.run()
}

// ============================================================================
// Server Structure
// ============================================================================

struct Server {
    /// mio poll instance
    poll: Poll,
    /// UDP socket
    socket: UdpSocket,
    /// quiche configuration
    config: quiche::Config,
    /// Connected clients (by connection ID)
    clients: HashMap<quiche::ConnectionId<'static>, Client>,
    /// Client registry for routing
    registry: Registry,
    /// P2P signaling session manager
    session_manager: SessionManager,
    /// Random number generator for connection IDs
    rng: SystemRandom,
    /// Receive buffer
    recv_buf: Vec<u8>,
    /// Send buffer
    send_buf: Vec<u8>,
    /// Stream read buffer
    stream_buf: Vec<u8>,
    /// External/public-facing address for QUIC path validation (NAT environments)
    /// If set, this is used instead of socket.local_addr() in RecvInfo.to
    external_addr: Option<SocketAddr>,
    /// Whether to require valid client certificates (mTLS)
    require_client_cert: bool,
    // 6B.1: Certificate hot-reload support
    /// Atomic flag set by SIGHUP handler to trigger config reload
    reload_flag: Arc<AtomicBool>,
    /// Path to TLS certificate (for reload)
    cert_path: String,
    /// Path to TLS private key (for reload)
    key_path: String,
    /// Path to CA certificate (for reload)
    ca_cert_path: Option<String>,
    /// Whether to verify peer certificates
    verify_peer: bool,
    // 7B: Stateless retry token support
    /// Whether to require retry tokens on new connections
    enable_retry: bool,
    /// AEAD key for retry token encryption/decryption
    retry_key: aead::LessSafeKey,
    // 8B.1: Connection ID rotation
    /// Maps new (rotated) CIDs back to the original CID used as the clients map key.
    /// When a packet arrives with a rotated CID in the DCID field, we look it up here
    /// to find the canonical client entry in `self.clients`.
    cid_aliases: HashMap<quiche::ConnectionId<'static>, quiche::ConnectionId<'static>>,
    /// Last time CID rotation was performed
    last_cid_rotation: Instant,
}

impl Server {
    #[allow(clippy::too_many_arguments)]
    fn new(
        port: u16,
        bind_addr: &str,
        external_ip: Option<&str>,
        cert_path: &str,
        key_path: &str,
        ca_cert_path: Option<&str>,
        verify_peer: bool,
        require_client_cert: bool,
        reload_flag: Arc<AtomicBool>,
        enable_retry: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Parse external address if provided (for NAT environments like AWS Elastic IP)
        let external_addr: Option<SocketAddr> = if let Some(ext_ip) = external_ip {
            Some(format!("{}:{}", ext_ip, port).parse()?)
        } else {
            None
        };

        // Create quiche configuration
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        // Load TLS certificates
        config.load_cert_chain_from_pem_file(cert_path)?;
        config.load_priv_key_from_pem_file(key_path)?;

        // CRITICAL: ALPN must match Agent
        config.set_application_protos(&[ALPN_PROTOCOL])?;

        // Enable DATAGRAM support (for QAD and IP tunneling)
        config.enable_dgram(true, 1000, 1000);

        // Set timeouts and limits (match Agent)
        config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        // C1: TLS peer verification — enabled by default for production
        config.verify_peer(verify_peer);
        if verify_peer {
            if let Some(ca_path) = ca_cert_path {
                config.load_verify_locations_from_file(ca_path)?;
                log::info!("Loaded CA certificate from {}", ca_path);
            }
        }

        // Create mio poll and UDP socket
        let poll = Poll::new()?;
        let addr: SocketAddr = format!("{}:{}", bind_addr, port).parse()?;
        let mut socket = UdpSocket::bind(addr)?;

        // Register socket with poll
        poll.registry()
            .register(&mut socket, SOCKET_TOKEN, Interest::READABLE)?;

        // 7B.1: Generate AEAD key for retry tokens
        let rng = SystemRandom::new();
        let mut key_bytes = [0u8; 32]; // AES-256-GCM
        rng.fill(&mut key_bytes)
            .map_err(|_| "Failed to generate retry token key")?;
        let unbound_key =
            aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes).map_err(|_| "Invalid key")?;
        let retry_key = aead::LessSafeKey::new(unbound_key);

        log::info!("Server listening on {}", addr);
        if let Some(ext) = external_addr {
            log::info!("External address for QUIC path validation: {}", ext);
        }

        Ok(Server {
            poll,
            socket,
            config,
            clients: HashMap::new(),
            registry: Registry::new(),
            session_manager: SessionManager::new(),
            rng,
            recv_buf: vec![0u8; 65535],
            send_buf: vec![0u8; MAX_DATAGRAM_SIZE],
            stream_buf: vec![0u8; 65535],
            external_addr,
            require_client_cert,
            reload_flag,
            cert_path: cert_path.to_string(),
            key_path: key_path.to_string(),
            ca_cert_path: ca_cert_path.map(|s| s.to_string()),
            verify_peer,
            enable_retry,
            retry_key,
            cid_aliases: HashMap::new(),
            last_cid_rotation: Instant::now(),
        })
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut events = Events::with_capacity(1024);

        loop {
            // 6B.1: Check for SIGHUP-triggered certificate reload
            if self.reload_flag.swap(false, Ordering::Relaxed) {
                self.reload_tls_config();
            }

            // Calculate timeout based on earliest connection timeout
            let timeout = self.clients.values().filter_map(|c| c.conn.timeout()).min();

            // Poll for events
            self.poll.poll(&mut events, timeout)?;

            // Process socket events
            for event in events.iter() {
                if event.token() == SOCKET_TOKEN {
                    self.process_socket()?;
                }
            }

            // Process streams for signaling
            self.process_streams()?;

            // Process timeouts for all connections
            self.process_timeouts();

            // 8B.2: Periodic CID rotation for privacy
            if self.last_cid_rotation.elapsed()
                >= std::time::Duration::from_secs(CID_ROTATION_INTERVAL_SECS)
            {
                self.rotate_connection_ids();
                self.last_cid_rotation = Instant::now();
            }

            // Cleanup expired signaling sessions
            let expired = self.session_manager.cleanup_expired();
            for session_id in expired {
                log::debug!("Cleaned up expired signaling session {}", session_id);
            }

            // Send pending packets for all connections
            self.send_pending()?;

            // Clean up closed connections
            self.cleanup_closed();
        }
    }

    /// 6B.1: Reload TLS configuration from disk (triggered by SIGHUP).
    /// New connections will use the updated certificates; existing connections are unaffected.
    fn reload_tls_config(&mut self) {
        log::info!("SIGHUP received — reloading TLS certificates...");

        match self.build_quiche_config() {
            Ok(new_config) => {
                self.config = new_config;
                log::info!(
                    "TLS certificates reloaded (cert={}, key={})",
                    self.cert_path,
                    self.key_path
                );
            }
            Err(e) => {
                log::error!(
                    "Failed to reload TLS certificates: {}. Keeping previous config.",
                    e
                );
            }
        }
    }

    /// Build a fresh quiche::Config from the stored cert/key/CA paths
    fn build_quiche_config(&self) -> Result<quiche::Config, Box<dyn std::error::Error>> {
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        config.load_cert_chain_from_pem_file(&self.cert_path)?;
        config.load_priv_key_from_pem_file(&self.key_path)?;
        config.set_application_protos(&[ALPN_PROTOCOL])?;
        config.enable_dgram(true, 1000, 1000);
        config.set_max_idle_timeout(IDLE_TIMEOUT_MS);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);

        config.verify_peer(self.verify_peer);
        if self.verify_peer {
            if let Some(ref ca_path) = self.ca_cert_path {
                config.load_verify_locations_from_file(ca_path)?;
            }
        }

        Ok(config)
    }

    fn process_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Use a separate buffer to avoid borrow conflicts with self.recv_buf
        let mut pkt_buf = vec![0u8; 65535];

        loop {
            // Receive UDP packet
            let (len, from) = match self.socket.recv_from(&mut self.recv_buf) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            };

            // Copy to working buffer to avoid borrow conflicts
            pkt_buf[..len].copy_from_slice(&self.recv_buf[..len]);
            let pkt_slice = &mut pkt_buf[..len];

            // Parse QUIC header
            let hdr = match quiche::Header::from_slice(pkt_slice, quiche::MAX_CONN_ID_LEN) {
                Ok(v) => v,
                Err(e) => {
                    log::debug!("Failed to parse QUIC header: {:?}", e);
                    continue;
                }
            };

            log::trace!("Received {} bytes from {} dcid={:?}", len, from, hdr.dcid);

            // Find or create connection
            // 8B.1: Resolve CID alias to canonical key if this is a rotated CID
            let conn_id = {
                let raw_dcid = hdr.dcid.clone().into_owned();
                if self.clients.contains_key(&raw_dcid) {
                    raw_dcid
                } else if let Some(canonical) = self.cid_aliases.get(&raw_dcid) {
                    canonical.clone()
                } else {
                    raw_dcid
                }
            };

            if !self.clients.contains_key(&conn_id) {
                // New connection
                if hdr.ty != quiche::Type::Initial {
                    log::debug!("Non-Initial packet for unknown connection");
                    continue;
                }

                // Handle new connection
                if let Err(e) = self.handle_new_connection(&hdr, from, pkt_slice) {
                    log::debug!("Failed to handle new connection: {:?}", e);
                    continue;
                }
            }

            // Process packet for existing connection
            // Use external_addr if set (for NAT environments), otherwise use socket local_addr
            let quic_local_addr = self.external_addr.unwrap_or(self.socket.local_addr()?);
            let (should_send_qad, should_process_dgrams) = if let Some(client) =
                self.clients.get_mut(&conn_id)
            {
                let recv_info = quiche::RecvInfo {
                    from,
                    to: quic_local_addr,
                };

                match client.conn.recv(pkt_slice, recv_info) {
                    Ok(_) => {
                        // Update observed address (for QAD)
                        if client.observed_addr != from {
                            log::debug!(
                                "Address change detected: {} -> {}",
                                client.observed_addr,
                                from
                            );
                            client.observed_addr = from;
                            client.qad_sent = false; // Re-send QAD
                        }

                        // 6A.4: Extract peer cert once connection is established
                        if client.conn.is_established() && client.authenticated_identity.is_none() {
                            if let Some(der_cert) = client.conn.peer_cert() {
                                match auth::extract_identity(der_cert) {
                                    Ok(identity) => {
                                        log::info!(
                                            "Client {:?} authenticated as '{}'",
                                            conn_id,
                                            identity.common_name
                                        );
                                        if let Some(ref services) = identity.authorized_services {
                                            log::info!("  Authorized services: {:?}", services);
                                        }
                                        client.authenticated_identity = Some(identity.common_name);
                                        client.authenticated_services =
                                            identity.authorized_services;
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Failed to extract identity from {:?}: {}",
                                            conn_id,
                                            e
                                        );
                                        if self.require_client_cert {
                                            log::warn!(
                                                "Closing {:?}: cert parse failed (mTLS required)",
                                                conn_id
                                            );
                                            client
                                                .conn
                                                .close(false, 0x01, b"invalid client cert")
                                                .ok();
                                        }
                                    }
                                }
                            } else if self.require_client_cert {
                                log::warn!(
                                    "Closing {:?}: no client certificate (mTLS required)",
                                    conn_id
                                );
                                client.conn.close(false, 0x01, b"client cert required").ok();
                            }
                        }

                        // Check if we need to send QAD or process datagrams
                        let send_qad = client.conn.is_established() && !client.qad_sent;
                        (send_qad, true)
                    }
                    Err(e) => {
                        log::debug!("Connection recv error: {:?}", e);
                        (false, false)
                    }
                }
            } else {
                (false, false)
            };

            // Send QAD if needed (outside the mutable borrow)
            if should_send_qad {
                self.send_qad(&conn_id)?;
            }

            // Process received DATAGRAMs (outside the mutable borrow)
            if should_process_dgrams {
                self.process_datagrams(&conn_id)?;
            }
        }

        Ok(())
    }

    fn handle_new_connection(
        &mut self,
        hdr: &quiche::Header,
        from: SocketAddr,
        pkt_buf: &mut [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Version negotiation if needed
        if !quiche::version_is_supported(hdr.version) {
            log::debug!("Version negotiation needed for {:?}", hdr.version);
            let len = quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut self.send_buf)?;
            self.socket.send_to(&self.send_buf[..len], from)?;
            return Ok(());
        }

        // 7B.3: Stateless retry token validation
        let odcid = if self.enable_retry {
            let token_data = hdr.token.as_deref().unwrap_or(&[]);
            if token_data.is_empty() {
                // No token — send Retry packet
                let token = self.mint_retry_token(&hdr.dcid, from)?;

                let mut new_scid = [0u8; quiche::MAX_CONN_ID_LEN];
                self.rng
                    .fill(&mut new_scid)
                    .map_err(|_| "Failed to generate retry scid")?;
                let new_scid = quiche::ConnectionId::from_ref(&new_scid);

                let len = quiche::retry(
                    &hdr.scid,
                    &hdr.dcid,
                    &new_scid,
                    &token,
                    hdr.version,
                    &mut self.send_buf,
                )?;

                self.socket.send_to(&self.send_buf[..len], from)?;
                log::debug!("Sent Retry to {} (dcid={:?})", from, hdr.dcid);
                return Ok(());
            }

            // Has token — validate it
            match self.validate_retry_token(token_data, from) {
                Some(original_dcid) => Some(original_dcid),
                None => {
                    log::warn!("Invalid retry token from {}", from);
                    return Ok(());
                }
            }
        } else {
            None
        };

        // Generate connection ID for accept.
        // When retry is active, we MUST reuse hdr.dcid (which is the Retry SCID
        // the client received) so quiche's retry_source_connection_id transport
        // parameter matches what the client expects. Using a fresh random SCID
        // causes a transport parameter mismatch → CONNECTION_CLOSE err=0x08.
        let scid = if odcid.is_some() {
            // Retry path: reuse the Retry SCID (= client's dcid in retried Initial)
            hdr.dcid.clone()
        } else {
            // Normal path: generate fresh random SCID
            let mut scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
            self.rng
                .fill(&mut scid_bytes)
                .map_err(|_| "Failed to generate connection ID")?;
            quiche::ConnectionId::from_vec(scid_bytes.to_vec())
        };

        // Accept the connection with optional odcid from retry token
        let quic_local_addr = self.external_addr.unwrap_or(self.socket.local_addr()?);
        let conn = quiche::accept(
            &scid,
            odcid.as_ref(),
            quic_local_addr,
            from,
            &mut self.config,
        )?;

        let scid_owned = scid.into_owned();
        log::info!("New connection from {} (scid={:?})", from, scid_owned);

        // Create client
        let client = Client::new(conn, from);

        // Store the connection (use our generated scid)
        self.clients.insert(scid_owned.clone(), client);

        // Feed the Initial packet to the new connection so the handshake
        // can proceed. quiche::accept() creates the connection but does not
        // process the packet data — that requires an explicit recv() call.
        if let Some(client) = self.clients.get_mut(&scid_owned) {
            let recv_info = quiche::RecvInfo {
                from,
                to: quic_local_addr,
            };
            match client.conn.recv(pkt_buf, recv_info) {
                Ok(_) => {}
                Err(quiche::Error::Done) => {}
                Err(e) => log::debug!("Initial packet recv after accept: {:?}", e),
            }
        }

        Ok(())
    }

    /// 7B.2: Generate an encrypted retry token containing [addr, dcid, timestamp]
    fn mint_retry_token(
        &self,
        dcid: &quiche::ConnectionId<'_>,
        addr: SocketAddr,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Plaintext: [addr_bytes..., dcid_len(u8), dcid_bytes..., timestamp(u64 BE)]
        let addr_str = addr.to_string();
        let addr_bytes = addr_str.as_bytes();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let mut plaintext = Vec::new();
        plaintext.push(addr_bytes.len() as u8);
        plaintext.extend_from_slice(addr_bytes);
        plaintext.push(dcid.len() as u8);
        plaintext.extend_from_slice(dcid.as_ref());
        plaintext.extend_from_slice(&timestamp.to_be_bytes());

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12]; // AES-256-GCM nonce size
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| "Failed to generate nonce")?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);

        // Encrypt in place
        let tag_len = self.retry_key.algorithm().tag_len();
        let mut in_out = plaintext;
        self.retry_key
            .seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
            .map_err(|_| "Token encryption failed")?;

        // Token format: [nonce(12), ciphertext+tag(...)]
        let mut token = Vec::with_capacity(12 + in_out.len());
        token.extend_from_slice(&nonce_bytes);
        token.extend_from_slice(&in_out);

        let _ = tag_len; // suppress unused warning
        Ok(token)
    }

    /// 7B.2: Validate a retry token — returns the original dcid if valid
    fn validate_retry_token(
        &self,
        token: &[u8],
        addr: SocketAddr,
    ) -> Option<quiche::ConnectionId<'static>> {
        if token.len() < 12 {
            return None; // Too short for nonce
        }

        let nonce_bytes: [u8; 12] = token[..12].try_into().ok()?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);

        let mut ciphertext = token[12..].to_vec();
        let plaintext = self
            .retry_key
            .open_in_place(nonce, aead::Aad::empty(), &mut ciphertext)
            .ok()?;

        // Parse: [addr_len(u8), addr_bytes..., dcid_len(u8), dcid_bytes..., timestamp(u64 BE)]
        if plaintext.is_empty() {
            return None;
        }
        let addr_len = plaintext[0] as usize;
        if plaintext.len() < 1 + addr_len + 1 {
            return None;
        }
        let token_addr_str = std::str::from_utf8(&plaintext[1..1 + addr_len]).ok()?;
        let dcid_len = plaintext[1 + addr_len] as usize;
        if plaintext.len() < 1 + addr_len + 1 + dcid_len + 8 {
            return None;
        }
        let dcid_bytes = &plaintext[1 + addr_len + 1..1 + addr_len + 1 + dcid_len];
        let ts_offset = 1 + addr_len + 1 + dcid_len;
        let timestamp = u64::from_be_bytes(plaintext[ts_offset..ts_offset + 8].try_into().ok()?);

        // Validate address matches
        let expected_addr = addr.to_string();
        if token_addr_str != expected_addr {
            log::debug!(
                "Retry token addr mismatch: {} vs {}",
                token_addr_str,
                expected_addr
            );
            return None;
        }

        // Validate timestamp freshness
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        if now.saturating_sub(timestamp) > RETRY_TOKEN_MAX_AGE_SECS {
            log::debug!("Retry token expired ({} seconds old)", now - timestamp);
            return None;
        }

        Some(quiche::ConnectionId::from_vec(dcid_bytes.to_vec()))
    }

    fn send_qad(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(client) = self.clients.get_mut(conn_id) {
            let qad_msg = qad::build_observed_address(client.observed_addr);
            match client.conn.dgram_send(&qad_msg) {
                Ok(_) => {
                    log::info!(
                        "Sent QAD to {:?} (observed: {})",
                        conn_id,
                        client.observed_addr
                    );
                    client.qad_sent = true;
                }
                Err(e) => {
                    log::debug!("Failed to send QAD: {:?}", e);
                }
            }
        }
        Ok(())
    }

    fn process_datagrams(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut dgrams = Vec::new();

        // Collect DATAGRAMs from this connection
        if let Some(client) = self.clients.get_mut(conn_id) {
            let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
            while let Ok(len) = client.conn.dgram_recv(&mut buf) {
                dgrams.push(buf[..len].to_vec());
            }
        }

        // Process collected DATAGRAMs
        for dgram in dgrams {
            if dgram.is_empty() {
                continue;
            }

            match dgram[0] {
                0x01 => {
                    // QAD message (ignore - server doesn't process QAD)
                    log::trace!("Ignoring QAD message from client");
                }
                0x10 | 0x11 => {
                    // Registration message
                    self.handle_registration(conn_id, &dgram)?;
                }
                0x2F => {
                    // Service-routed IP packet: [0x2F, id_len, service_id..., ip_packet...]
                    self.relay_service_datagram(conn_id, &dgram)?;
                }
                _ => {
                    // Raw IP packet - relay to paired connection (implicit routing)
                    log::debug!("Received {} bytes to relay from {:?}", dgram.len(), conn_id);
                    self.relay_datagram(conn_id, &dgram)?;
                }
            }
        }

        Ok(())
    }

    fn handle_registration(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
        dgram: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if dgram.len() < 2 {
            log::debug!("Registration message too short");
            return Ok(());
        }

        let client_type = match dgram[0] {
            0x10 => ClientType::Agent,
            0x11 => ClientType::Connector,
            _ => return Ok(()),
        };

        let id_len = dgram[1] as usize;
        if dgram.len() < 2 + id_len {
            log::debug!("Registration message ID truncated");
            return Ok(());
        }

        let service_id = match String::from_utf8(dgram[2..2 + id_len].to_vec()) {
            Ok(id) => id,
            Err(e) => {
                log::warn!(
                    "Rejecting registration with invalid UTF-8 service ID from {:?}: {}",
                    conn_id,
                    e
                );
                // 8A.2: Send NACK for invalid UTF-8
                self.send_registration_nack(conn_id, &[], 0x01);
                return Ok(());
            }
        };

        log::info!(
            "Registration: {:?} for service '{}' (conn={:?})",
            client_type,
            service_id,
            conn_id
        );

        // 6A.5: Check mTLS authorization before allowing registration
        if self.require_client_cert {
            if let Some(client) = self.clients.get(conn_id) {
                if let Some(ref services) = client.authenticated_services {
                    if !services.is_empty() {
                        let identity = auth::ClientIdentity {
                            common_name: client.authenticated_identity.clone().unwrap_or_default(),
                            authorized_services: Some(services.clone()),
                        };
                        if !auth::is_authorized_for_service(&identity, &service_id, &client_type) {
                            log::warn!(
                                "Rejecting registration: {:?} '{}' not authorized for service '{}' (conn={:?})",
                                client_type,
                                identity.common_name,
                                service_id,
                                conn_id
                            );
                            // 8A.2: Send NACK for auth denial
                            self.send_registration_nack(conn_id, service_id.as_bytes(), 0x02);
                            return Ok(());
                        }
                    }
                    // Empty set with require_client_cert = deny
                } else {
                    // None = no ZTNA SANs = allow all (backward compat)
                }
            }
        }

        // Update client type
        if let Some(client) = self.clients.get_mut(conn_id) {
            client.client_type = Some(client_type.clone());
            client.registered_id = Some(service_id.clone());
        }

        // Register in routing table
        self.registry
            .register(conn_id.clone(), client_type, service_id.clone());

        // 8A.2: Send ACK after successful registration
        self.send_registration_ack(conn_id, &service_id);

        Ok(())
    }

    /// 8A.2: Send registration ACK to client
    /// Wire format: [0x12, status(0x00=ok), id_len, service_id_bytes...]
    fn send_registration_ack(&mut self, conn_id: &quiche::ConnectionId<'static>, service_id: &str) {
        let id_bytes = service_id.as_bytes();
        let mut msg = Vec::with_capacity(3 + id_bytes.len());
        msg.push(REG_TYPE_ACK);
        msg.push(0x00); // status: success
        msg.push(id_bytes.len() as u8);
        msg.extend_from_slice(id_bytes);

        if let Some(client) = self.clients.get_mut(conn_id) {
            match client.conn.dgram_send(&msg) {
                Ok(_) => {
                    log::debug!(
                        "Sent registration ACK for service '{}' to {:?}",
                        service_id,
                        conn_id
                    );
                }
                Err(e) => {
                    log::debug!("Failed to send registration ACK to {:?}: {:?}", conn_id, e);
                }
            }
        }
    }

    /// 8A.2: Send registration NACK to client
    /// Wire format: [0x13, status, id_len, service_id_bytes...]
    /// Status codes: 0x01 = invalid request, 0x02 = auth denied
    fn send_registration_nack(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
        service_id_bytes: &[u8],
        status: u8,
    ) {
        let mut msg = Vec::with_capacity(3 + service_id_bytes.len());
        msg.push(REG_TYPE_NACK);
        msg.push(status);
        msg.push(service_id_bytes.len() as u8);
        msg.extend_from_slice(service_id_bytes);

        if let Some(client) = self.clients.get_mut(conn_id) {
            match client.conn.dgram_send(&msg) {
                Ok(_) => {
                    log::debug!(
                        "Sent registration NACK (status=0x{:02x}) to {:?}",
                        status,
                        conn_id
                    );
                }
                Err(e) => {
                    log::debug!("Failed to send registration NACK to {:?}: {:?}", conn_id, e);
                }
            }
        }
    }

    fn relay_datagram(
        &mut self,
        from_conn_id: &quiche::ConnectionId<'static>,
        dgram: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find destination connection
        let dest_conn_id = match self.registry.find_destination(from_conn_id) {
            Some(id) => {
                log::debug!("Found destination {:?} for {:?}", id, from_conn_id);
                id
            }
            None => {
                log::warn!("No destination for relay from {:?}", from_conn_id);
                return Ok(());
            }
        };

        // Forward the datagram
        if let Some(dest_client) = self.clients.get_mut(&dest_conn_id) {
            log::debug!(
                "Destination connection established: {}",
                dest_client.conn.is_established()
            );
            match dest_client.conn.dgram_send(dgram) {
                Ok(_) => {
                    log::debug!(
                        "Relayed {} bytes from {:?} to {:?}",
                        dgram.len(),
                        from_conn_id,
                        dest_conn_id
                    );
                }
                Err(e) => {
                    log::error!("Failed to relay datagram: {:?}", e);
                }
            }
        } else {
            log::warn!(
                "Destination client {:?} not found in clients map",
                dest_conn_id
            );
        }

        Ok(())
    }

    fn relay_service_datagram(
        &mut self,
        from_conn_id: &quiche::ConnectionId<'static>,
        dgram: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse: [0x2F, id_len, service_id..., ip_packet...]
        if dgram.len() < 3 {
            log::debug!("Service-routed datagram too short");
            return Ok(());
        }

        let id_len = dgram[1] as usize;
        if dgram.len() < 2 + id_len {
            log::debug!("Service ID truncated in routed datagram");
            return Ok(());
        }

        let service_id = match String::from_utf8(dgram[2..2 + id_len].to_vec()) {
            Ok(id) => id,
            Err(e) => {
                log::warn!(
                    "Rejecting service-routed datagram with invalid UTF-8 service ID from {:?}: {}",
                    from_conn_id,
                    e
                );
                return Ok(());
            }
        };
        let ip_packet = &dgram[2 + id_len..];

        // M3: Verify sender is a registered Agent for this service before relaying
        if !self
            .registry
            .is_agent_for_service(from_conn_id, &service_id)
        {
            log::warn!(
                "Unauthorized service datagram: {:?} is not registered for '{}'",
                from_conn_id,
                service_id
            );
            return Ok(());
        }

        log::debug!(
            "Service-routed datagram: {} bytes for '{}' from {:?}",
            ip_packet.len(),
            service_id,
            from_conn_id
        );

        // Find Connector for this service
        let dest_conn_id = match self.registry.find_connector_for_service(&service_id) {
            Some(id) => {
                log::debug!("Routing to Connector {:?} for service '{}'", id, service_id);
                id
            }
            None => {
                log::warn!("No Connector registered for service '{}'", service_id);
                return Ok(());
            }
        };

        // Forward the unwrapped IP packet (Connector doesn't need the service wrapper)
        if let Some(dest_client) = self.clients.get_mut(&dest_conn_id) {
            match dest_client.conn.dgram_send(ip_packet) {
                Ok(_) => {
                    log::debug!(
                        "Relayed {} bytes for '{}' from {:?} to {:?}",
                        ip_packet.len(),
                        service_id,
                        from_conn_id,
                        dest_conn_id
                    );
                }
                Err(e) => {
                    log::error!("Failed to relay service datagram: {:?}", e);
                }
            }
        } else {
            log::warn!(
                "Connector {:?} for '{}' not in clients map",
                dest_conn_id,
                service_id
            );
        }

        Ok(())
    }

    /// Process signaling streams for P2P hole punching coordination
    fn process_streams(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Collect conn_ids with readable streams to avoid borrow conflicts
        let conn_ids: Vec<_> = self.clients.keys().cloned().collect();

        for conn_id in conn_ids {
            // Collect readable stream IDs for this connection
            let readable_streams: Vec<u64> = {
                if let Some(client) = self.clients.get(&conn_id) {
                    client.conn.readable().collect()
                } else {
                    continue;
                }
            };

            for stream_id in readable_streams {
                // Read stream data
                let mut stream_finished = false;
                if let Some(client) = self.clients.get_mut(&conn_id) {
                    loop {
                        match client.conn.stream_recv(stream_id, &mut self.stream_buf) {
                            Ok((len, fin)) => {
                                let buffer = client.get_signaling_buffer(stream_id);
                                buffer.extend_from_slice(&self.stream_buf[..len]);
                                if fin {
                                    stream_finished = true;
                                }
                                if len == 0 {
                                    break;
                                }
                            }
                            Err(quiche::Error::Done) => break,
                            Err(e) => {
                                log::debug!(
                                    "Stream recv error on {:?}/{}: {:?}",
                                    conn_id,
                                    stream_id,
                                    e
                                );
                                break;
                            }
                        }
                    }
                }

                // Try to decode and handle messages
                self.process_stream_messages(&conn_id, stream_id)?;

                // Cleanup finished streams
                if stream_finished {
                    if let Some(client) = self.clients.get_mut(&conn_id) {
                        client.remove_signaling_buffer(stream_id);
                    }
                }
            }
        }

        // Process sessions that are ready to start punching
        self.process_ready_sessions()?;

        Ok(())
    }

    /// Process decoded messages from a stream buffer
    fn process_stream_messages(
        &mut self,
        conn_id: &quiche::ConnectionId<'static>,
        stream_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // Get buffer contents
            let buffer_data: Vec<u8> = {
                if let Some(client) = self.clients.get(conn_id) {
                    if let Some(buf) = client.signaling_buffers.get(&stream_id) {
                        buf.clone()
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            };

            if buffer_data.is_empty() {
                break;
            }

            // Try to decode a message
            match decode_message(&buffer_data) {
                Ok((msg, consumed)) => {
                    log::info!(
                        "Decoded signaling message from {:?}/{}: {:?}",
                        conn_id,
                        stream_id,
                        msg
                    );

                    // Consume the bytes
                    if let Some(client) = self.clients.get_mut(conn_id) {
                        if let Some(buf) = client.signaling_buffers.get_mut(&stream_id) {
                            buf.drain(..consumed);
                        }
                    }

                    // Handle the message
                    self.handle_signaling_message(conn_id, stream_id, msg)?;
                }
                Err(DecodeError::Incomplete(_)) => {
                    // Need more data
                    break;
                }
                Err(DecodeError::TooLarge(size)) => {
                    log::error!("Signaling message too large: {} bytes", size);
                    // Clear the buffer to recover
                    if let Some(client) = self.clients.get_mut(conn_id) {
                        client.remove_signaling_buffer(stream_id);
                    }
                    break;
                }
                Err(DecodeError::Invalid(e)) => {
                    log::error!("Invalid signaling message: {}", e);
                    // Clear the buffer to recover
                    if let Some(client) = self.clients.get_mut(conn_id) {
                        client.remove_signaling_buffer(stream_id);
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    /// Handle a decoded signaling message
    fn handle_signaling_message(
        &mut self,
        from_conn_id: &quiche::ConnectionId<'static>,
        stream_id: u64,
        msg: SignalingMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match msg {
            SignalingMessage::CandidateOffer {
                session_id,
                service_id,
                candidates,
            } => {
                log::info!(
                    "CandidateOffer: session={}, service={}, {} candidates",
                    session_id,
                    service_id,
                    candidates.len()
                );

                // Find the Connector for this service
                let connector_conn_id = match self.registry.find_connector_for_service(&service_id)
                {
                    Some(id) => id,
                    None => {
                        log::warn!("No Connector for service '{}'", service_id);
                        self.send_signaling_error(
                            from_conn_id,
                            stream_id,
                            Some(session_id),
                            SignalingError::NoConnectorAvailable,
                            format!("No Connector available for service '{}'", service_id),
                        )?;
                        return Ok(());
                    }
                };

                // Create signaling session
                self.session_manager.create_session(
                    session_id,
                    service_id.clone(),
                    from_conn_id.clone(),
                    candidates.clone(),
                );

                // Forward CandidateOffer to Connector
                self.forward_signaling_message(
                    &connector_conn_id,
                    &SignalingMessage::CandidateOffer {
                        session_id,
                        service_id,
                        candidates,
                    },
                )?;
            }

            SignalingMessage::CandidateAnswer {
                session_id,
                candidates,
            } => {
                log::info!(
                    "CandidateAnswer: session={}, {} candidates",
                    session_id,
                    candidates.len()
                );

                // Find the session
                if let Some(session) = self.session_manager.get_session_mut(session_id) {
                    // Store Connector's answer
                    session.set_connector_answer(from_conn_id.clone(), candidates, stream_id);

                    log::info!(
                        "Session {} ready to punch (agent={:?}, connector={:?})",
                        session_id,
                        session.agent_conn_id,
                        from_conn_id
                    );
                } else {
                    log::warn!("CandidateAnswer for unknown session {}", session_id);
                    self.send_signaling_error(
                        from_conn_id,
                        stream_id,
                        Some(session_id),
                        SignalingError::SessionNotFound,
                        format!("Session {} not found", session_id),
                    )?;
                }
            }

            SignalingMessage::PunchingResult {
                session_id,
                success,
                working_address,
            } => {
                log::info!(
                    "PunchingResult: session={}, success={}, addr={:?}",
                    session_id,
                    success,
                    working_address
                );

                // Forward to the peer
                if let Some(session) = self.session_manager.get_session(session_id) {
                    let peer_conn_id = if *from_conn_id == session.agent_conn_id {
                        session.connector_conn_id.clone()
                    } else {
                        Some(session.agent_conn_id.clone())
                    };

                    if let Some(peer_id) = peer_conn_id {
                        self.forward_signaling_message(
                            &peer_id,
                            &SignalingMessage::PunchingResult {
                                session_id,
                                success,
                                working_address,
                            },
                        )?;
                    }

                    // Mark session complete if both sides reported
                    if success {
                        log::info!("P2P connection established for session {}", session_id);
                    }
                }
            }

            SignalingMessage::StartPunching { .. } => {
                // Intermediate doesn't originate StartPunching, it creates them
                log::warn!("Unexpected StartPunching from client");
            }

            SignalingMessage::Error {
                session_id,
                code,
                message,
            } => {
                log::warn!(
                    "Signaling error from {:?}: session={:?}, code={:?}, msg={}",
                    from_conn_id,
                    session_id,
                    code,
                    message
                );
                // Forward error to peer if session exists
                if let Some(sid) = session_id {
                    if let Some(session) = self.session_manager.get_session(sid) {
                        let peer_conn_id = if *from_conn_id == session.agent_conn_id {
                            session.connector_conn_id.clone()
                        } else {
                            Some(session.agent_conn_id.clone())
                        };

                        if let Some(peer_id) = peer_conn_id {
                            self.forward_signaling_message(
                                &peer_id,
                                &SignalingMessage::Error {
                                    session_id,
                                    code,
                                    message,
                                },
                            )?;
                        }
                    }
                    // Cleanup the session
                    self.session_manager.remove_session(sid);
                }
            }
        }

        Ok(())
    }

    /// Process sessions that are ready to start hole punching
    fn process_ready_sessions(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Collect sessions ready to punch
        let ready_sessions: Vec<ReadySession> = {
            let mut ready = Vec::new();
            // Manually iterate to avoid borrow issues
            for (session_id, session) in self.session_manager.sessions_iter() {
                if session.state == SessionState::ReadyToPunch {
                    if let Some(ref connector_id) = session.connector_conn_id {
                        if let Some(ref connector_candidates) = session.connector_candidates {
                            ready.push((
                                *session_id,
                                session.agent_conn_id.clone(),
                                connector_id.clone(),
                                session.agent_candidates.clone(),
                                connector_candidates.clone(),
                            ));
                        }
                    }
                }
            }
            ready
        };

        // Send StartPunching to both parties
        for (session_id, agent_id, connector_id, agent_candidates, connector_candidates) in
            ready_sessions
        {
            log::info!("Sending StartPunching for session {}", session_id);

            // Send to Agent with Connector's candidates
            self.forward_signaling_message(
                &agent_id,
                &SignalingMessage::StartPunching {
                    session_id,
                    start_delay_ms: PUNCH_START_DELAY_MS,
                    peer_candidates: connector_candidates.clone(),
                },
            )?;

            // Send to Connector with Agent's candidates
            self.forward_signaling_message(
                &connector_id,
                &SignalingMessage::StartPunching {
                    session_id,
                    start_delay_ms: PUNCH_START_DELAY_MS,
                    peer_candidates: agent_candidates,
                },
            )?;

            // Update session state to Punching
            if let Some(session) = self.session_manager.get_session_mut(session_id) {
                session.state = SessionState::Punching;
            }
        }

        Ok(())
    }

    /// Forward a signaling message to a client
    fn forward_signaling_message(
        &mut self,
        to_conn_id: &quiche::ConnectionId<'static>,
        msg: &SignalingMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_message(msg).map_err(|e| format!("encode error: {}", e))?;

        if let Some(client) = self.clients.get_mut(to_conn_id) {
            // Open a new stream for the response
            let _stream_id = client
                .conn
                .stream_priority(0, 0, true)
                .map(|_| 0u64)
                .unwrap_or(0);

            // Send on stream 0 (server-initiated bidirectional) or find next available
            // For simplicity, we'll use the client-initiated stream pattern
            // The server responds on client streams or uses stream 1 for server-initiated
            match client.conn.stream_send(0, &encoded, false) {
                Ok(_) => {
                    log::debug!(
                        "Forwarded signaling message to {:?} ({} bytes)",
                        to_conn_id,
                        encoded.len()
                    );
                }
                Err(quiche::Error::InvalidStreamState(_)) => {
                    // Stream not open, try to create server-initiated stream (stream_id = 1)
                    match client.conn.stream_send(1, &encoded, false) {
                        Ok(_) => {
                            log::debug!(
                                "Forwarded signaling message to {:?} on stream 1 ({} bytes)",
                                to_conn_id,
                                encoded.len()
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to send signaling message: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to send signaling message: {:?}", e);
                }
            }
        } else {
            log::warn!("Cannot forward message: client {:?} not found", to_conn_id);
        }

        Ok(())
    }

    /// Send an error response to a client
    fn send_signaling_error(
        &mut self,
        to_conn_id: &quiche::ConnectionId<'static>,
        _stream_id: u64,
        session_id: Option<u64>,
        code: SignalingError,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.forward_signaling_message(
            to_conn_id,
            &SignalingMessage::Error {
                session_id,
                code,
                message,
            },
        )
    }

    fn process_timeouts(&mut self) {
        for client in self.clients.values_mut() {
            client.conn.on_timeout();
        }
    }

    fn send_pending(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for client in self.clients.values_mut() {
            loop {
                match client.conn.send(&mut self.send_buf) {
                    Ok((len, send_info)) => {
                        log::trace!("Sending {} bytes to {:?}", len, send_info.to);
                        self.socket.send_to(&self.send_buf[..len], send_info.to)?;
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

    fn cleanup_closed(&mut self) {
        let closed: Vec<_> = self
            .clients
            .iter()
            .filter(|(_, c)| c.conn.is_closed())
            .map(|(id, _)| id.clone())
            .collect();

        for conn_id in closed {
            log::info!("Connection closed: {:?}", conn_id);
            self.registry.unregister(&conn_id);
            self.clients.remove(&conn_id);
            // 8B.2: Remove any CID aliases pointing to this connection
            self.cid_aliases
                .retain(|_, canonical| *canonical != conn_id);
        }
    }

    /// 8B.2: Rotate connection IDs for all established connections.
    ///
    /// For each established connection, generates a new random source CID via
    /// `conn.new_scid()`. The new CID is registered as an alias pointing back
    /// to the canonical (original) CID in `self.clients`, so incoming packets
    /// using the new CID are correctly routed.
    fn rotate_connection_ids(&mut self) {
        let conn_ids: Vec<quiche::ConnectionId<'static>> = self.clients.keys().cloned().collect();
        let mut rotated = 0u32;

        for conn_id in &conn_ids {
            let client = match self.clients.get_mut(conn_id) {
                Some(c) => c,
                None => continue,
            };

            if !client.conn.is_established() {
                continue;
            }

            // Check if the peer can accept more CIDs
            if client.conn.scids_left() == 0 {
                log::debug!(
                    "Skipping CID rotation for {:?}: peer CID limit reached",
                    conn_id
                );
                continue;
            }

            // Generate a new random source CID
            let mut new_scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
            if self.rng.fill(&mut new_scid_bytes).is_err() {
                log::warn!("Failed to generate random CID for rotation");
                continue;
            }
            let new_scid = quiche::ConnectionId::from_vec(new_scid_bytes.to_vec());

            // Generate a random stateless reset token (u128)
            let mut reset_token_bytes = [0u8; 16];
            if self.rng.fill(&mut reset_token_bytes).is_err() {
                log::warn!("Failed to generate reset token for CID rotation");
                continue;
            }
            let reset_token = u128::from_be_bytes(reset_token_bytes);

            // Provide the new source CID to the connection
            match client.conn.new_scid(&new_scid, reset_token, true) {
                Ok(seq) => {
                    // Register alias: new CID -> canonical CID
                    self.cid_aliases
                        .insert(new_scid.clone().into_owned(), conn_id.clone());

                    // Prune stale aliases: keep at most 4 aliases per connection
                    // to bound memory growth on long-lived connections
                    let alias_count = self.cid_aliases.values().filter(|v| *v == conn_id).count();
                    if alias_count > 4 {
                        // Remove oldest aliases (first found) until at limit
                        let excess = alias_count - 4;
                        let stale: Vec<_> = self
                            .cid_aliases
                            .iter()
                            .filter(|(_, v)| *v == conn_id)
                            .map(|(k, _)| k.clone())
                            .take(excess)
                            .collect();
                        for old_alias in stale {
                            self.cid_aliases.remove(&old_alias);
                        }
                    }

                    rotated += 1;
                    log::debug!(
                        "Rotated CID for {:?}: new scid={:?} (seq={})",
                        conn_id,
                        new_scid,
                        seq
                    );
                }
                Err(e) => {
                    log::debug!("CID rotation failed for {:?}: {:?}", conn_id, e);
                }
            }
        }

        if rotated > 0 {
            log::info!(
                "CID rotation complete: {}/{} connections rotated, {} aliases total",
                rotated,
                conn_ids.len(),
                self.cid_aliases.len()
            );
        }
    }
}
