//! ZTNA Intermediate Server
//!
//! A QUIC server that:
//! - Accepts connections from Agents and App Connectors
//! - Implements QAD (QUIC Address Discovery)
//! - Relays DATAGRAM frames between matched pairs

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;

use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use ring::rand::{SecureRandom, SystemRandom};

mod client;
mod qad;
mod registry;
mod signaling;

use client::{Client, ClientType};
use registry::Registry;

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
    let port = if args.len() > 1 {
        args[1].parse().unwrap_or(DEFAULT_PORT)
    } else {
        DEFAULT_PORT
    };

    let cert_path = args.get(2).map(|s| s.as_str()).unwrap_or("certs/cert.pem");
    let key_path = args.get(3).map(|s| s.as_str()).unwrap_or("certs/key.pem");

    log::info!("ZTNA Intermediate Server starting...");
    log::info!("  Port: {}", port);
    log::info!("  Cert: {}", cert_path);
    log::info!("  Key:  {}", key_path);
    log::info!("  ALPN: {:?}", std::str::from_utf8(ALPN_PROTOCOL));

    // Create server and run
    let mut server = Server::new(port, cert_path, key_path)?;
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
    /// Random number generator for connection IDs
    rng: SystemRandom,
    /// Receive buffer
    recv_buf: Vec<u8>,
    /// Send buffer
    send_buf: Vec<u8>,
}

impl Server {
    fn new(port: u16, cert_path: &str, key_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
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
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let mut socket = UdpSocket::bind(addr)?;

        // Register socket with poll
        poll.registry()
            .register(&mut socket, SOCKET_TOKEN, Interest::READABLE)?;

        log::info!("Server listening on {}", addr);

        Ok(Server {
            poll,
            socket,
            config,
            clients: HashMap::new(),
            registry: Registry::new(),
            rng: SystemRandom::new(),
            recv_buf: vec![0u8; 65535],
            send_buf: vec![0u8; MAX_DATAGRAM_SIZE],
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

            // Process timeouts for all connections
            self.process_timeouts();

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
            let local_addr = self.socket.local_addr()?;
            let (should_send_qad, should_process_dgrams) = if let Some(client) = self.clients.get_mut(&conn_id) {
                let recv_info = quiche::RecvInfo {
                    from,
                    to: local_addr,
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
        let local_addr = self.socket.local_addr()?;
        let conn = quiche::accept(&scid, None, local_addr, from, &mut self.config)?;

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
                to: local_addr,
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
                _ => {
                    // Raw IP packet - relay to paired connection
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
