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

use serde::Deserialize;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::rand::{SecureRandom, SystemRandom};

mod client;
mod qad;
mod registry;
mod signaling;

use client::{Client, ClientType};
use registry::Registry;
use signaling::{
    decode_message, encode_message, DecodeError, SessionManager, SessionState,
    SignalingError, SignalingMessage, PUNCH_START_DELAY_MS,
};

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
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

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
            args.get(1).filter(|a| !a.starts_with("--")).and_then(|s| s.parse().ok())
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

    log::info!("ZTNA Intermediate Server starting...");
    log::info!("  Port: {}", port);
    log::info!("  Bind: {}", bind_addr);
    if let Some(ref ext_ip) = external_ip {
        log::info!("  External IP: {}", ext_ip);
    }
    log::info!("  Cert: {}", cert_path);
    log::info!("  Key:  {}", key_path);
    log::info!("  ALPN: {:?}", std::str::from_utf8(ALPN_PROTOCOL));

    // Create server and run
    let mut server = Server::new(port, &bind_addr, external_ip.as_deref(), &cert_path, &key_path)?;
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
}

impl Server {
    fn new(port: u16, bind_addr: &str, external_ip: Option<&str>, cert_path: &str, key_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
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

        // Disable client certificate verification (for MVP)
        config.verify_peer(false);

        // Create mio poll and UDP socket
        let poll = Poll::new()?;
        let addr: SocketAddr = format!("{}:{}", bind_addr, port).parse()?;
        let mut socket = UdpSocket::bind(addr)?;

        // Register socket with poll
        poll.registry()
            .register(&mut socket, SOCKET_TOKEN, Interest::READABLE)?;

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
            rng: SystemRandom::new(),
            recv_buf: vec![0u8; 65535],
            send_buf: vec![0u8; MAX_DATAGRAM_SIZE],
            stream_buf: vec![0u8; 65535],
            external_addr,
        })
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut events = Events::with_capacity(1024);

        loop {
            // Calculate timeout based on earliest connection timeout
            let timeout = self
                .clients
                .values()
                .filter_map(|c| c.conn.timeout())
                .min();

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

            log::trace!(
                "Received {} bytes from {} dcid={:?}",
                len,
                from,
                hdr.dcid
            );

            // Find or create connection
            let conn_id = hdr.dcid.clone().into_owned();

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
            let (should_send_qad, should_process_dgrams) = if let Some(client) = self.clients.get_mut(&conn_id) {
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

        // Generate new connection ID
        let mut scid = [0u8; quiche::MAX_CONN_ID_LEN];
        self.rng
            .fill(&mut scid)
            .map_err(|_| "Failed to generate connection ID")?;
        let scid = quiche::ConnectionId::from_ref(&scid);

        // Accept the connection
        // Use external_addr if set (for NAT environments like AWS Elastic IP)
        let quic_local_addr = self.external_addr.unwrap_or(self.socket.local_addr()?);
        let conn = quiche::accept(&scid, None, quic_local_addr, from, &mut self.config)?;

        let scid_owned = scid.into_owned();
        log::info!("New connection from {} (scid={:?})", from, scid_owned);

        // Create client
        let client = Client::new(conn, from);

        // Store the connection (use our generated scid)
        self.clients.insert(scid_owned.clone(), client);

        // Process the Initial packet
        if let Some(client) = self.clients.get_mut(&scid_owned) {
            let recv_info = quiche::RecvInfo {
                from,
                to: quic_local_addr,
            };
            client.conn.recv(pkt_buf, recv_info)?;
        }

        Ok(())
    }

    fn send_qad(&mut self, conn_id: &quiche::ConnectionId<'static>) -> Result<(), Box<dyn std::error::Error>> {
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

    fn process_datagrams(&mut self, conn_id: &quiche::ConnectionId<'static>) -> Result<(), Box<dyn std::error::Error>> {
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
                    log::info!("Received {} bytes to relay from {:?}", dgram.len(), conn_id);
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

        let service_id = String::from_utf8_lossy(&dgram[2..2 + id_len]).to_string();

        log::info!(
            "Registration: {:?} for service '{}' (conn={:?})",
            client_type,
            service_id,
            conn_id
        );

        // Update client type
        if let Some(client) = self.clients.get_mut(conn_id) {
            client.client_type = Some(client_type.clone());
            client.registered_id = Some(service_id.clone());
        }

        // Register in routing table
        self.registry.register(conn_id.clone(), client_type, service_id);

        Ok(())
    }

    fn relay_datagram(
        &mut self,
        from_conn_id: &quiche::ConnectionId<'static>,
        dgram: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find destination connection
        let dest_conn_id = match self.registry.find_destination(from_conn_id) {
            Some(id) => {
                log::info!("Found destination {:?} for {:?}", id, from_conn_id);
                id
            }
            None => {
                log::warn!("No destination for relay from {:?}", from_conn_id);
                return Ok(());
            }
        };

        // Forward the datagram
        if let Some(dest_client) = self.clients.get_mut(&dest_conn_id) {
            log::info!("Destination connection established: {}", dest_client.conn.is_established());
            match dest_client.conn.dgram_send(dgram) {
                Ok(_) => {
                    log::info!(
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
            log::warn!("Destination client {:?} not found in clients map", dest_conn_id);
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

        let service_id = String::from_utf8_lossy(&dgram[2..2 + id_len]).to_string();
        let ip_packet = &dgram[2 + id_len..];

        log::info!(
            "Service-routed datagram: {} bytes for '{}' from {:?}",
            ip_packet.len(),
            service_id,
            from_conn_id
        );

        // Find Connector for this service
        let dest_conn_id = match self.registry.find_connector_for_service(&service_id) {
            Some(id) => {
                log::info!("Routing to Connector {:?} for service '{}'", id, service_id);
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
                    log::info!(
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
            log::warn!("Connector {:?} for '{}' not in clients map", dest_conn_id, service_id);
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
                                log::debug!("Stream recv error on {:?}/{}: {:?}", conn_id, stream_id, e);
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
                if let Some(client) = self.clients.get(&conn_id) {
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
                    if let Some(client) = self.clients.get_mut(&conn_id) {
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
                    if let Some(client) = self.clients.get_mut(&conn_id) {
                        client.remove_signaling_buffer(stream_id);
                    }
                    break;
                }
                Err(DecodeError::Invalid(e)) => {
                    log::error!("Invalid signaling message: {}", e);
                    // Clear the buffer to recover
                    if let Some(client) = self.clients.get_mut(&conn_id) {
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
                let connector_conn_id = match self.registry.find_connector_for_service(&service_id) {
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
                    stream_id,
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
                    session.set_connector_answer(
                        from_conn_id.clone(),
                        candidates,
                        stream_id,
                    );

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
        let ready_sessions: Vec<(u64, quiche::ConnectionId<'static>, quiche::ConnectionId<'static>, Vec<signaling::Candidate>, Vec<signaling::Candidate>)> = {
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
        for (session_id, agent_id, connector_id, agent_candidates, connector_candidates) in ready_sessions {
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
            let stream_id = client.conn.stream_priority(0, 0, true)
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
        }
    }
}
