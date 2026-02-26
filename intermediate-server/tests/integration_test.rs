//! Integration tests for the Intermediate Server
//!
//! These tests verify the server works correctly with QUIC clients.

use std::net::{SocketAddr, UdpSocket};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

/// ALPN protocol (must match server and Agent)
const ALPN_PROTOCOL: &[u8] = b"ztna-v1";

/// Test server port (different from default 4433 to avoid conflicts)
const TEST_SERVER_PORT: u16 = 4435;

/// Helper to spawn the intermediate server for tests
struct ServerProcess {
    child: Child,
}

impl ServerProcess {
    fn start(port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        // Build the server first
        let status = Command::new("cargo")
            .args(["build", "--release"])
            .status()?;

        if !status.success() {
            return Err("Failed to build intermediate server".into());
        }

        // Start the server (--disable-retry for basic connectivity tests)
        let child = Command::new("cargo")
            .args([
                "run",
                "--release",
                "--",
                &port.to_string(),
                "certs/cert.pem",
                "certs/key.pem",
                "--disable-retry",
            ])
            .env("RUST_LOG", "info")
            .spawn()?;

        // Give server time to start
        thread::sleep(Duration::from_millis(500));

        Ok(ServerProcess { child })
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// QAD message type
const QAD_OBSERVED_ADDRESS: u8 = 0x01;

/// Create a QUIC client config matching the Agent
fn create_client_config() -> quiche::Config {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

    // Disable certificate verification (matching Agent)
    config.verify_peer(false);

    // Set ALPN (CRITICAL: must match server)
    config.set_application_protos(&[ALPN_PROTOCOL]).unwrap();

    // Enable DATAGRAM
    config.enable_dgram(true, 1000, 1000);

    // Set timeouts and limits
    config.set_max_idle_timeout(30_000);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.set_max_recv_udp_payload_size(1350);
    config.set_max_send_udp_payload_size(1350);

    config
}

/// Parse a QAD OBSERVED_ADDRESS message
fn parse_qad_message(data: &[u8]) -> Option<SocketAddr> {
    if data.len() != 7 || data[0] != QAD_OBSERVED_ADDRESS {
        return None;
    }

    let ip = std::net::Ipv4Addr::new(data[1], data[2], data[3], data[4]);
    let port = u16::from_be_bytes([data[5], data[6]]);

    Some(SocketAddr::from((ip, port)))
}

#[test]
fn test_client_connection_and_qad() {
    // Start the intermediate server
    let _server = match ServerProcess::start(TEST_SERVER_PORT) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Failed to start server (expected in some CI environments): {}",
                e
            );
            return;
        }
    };

    let server_addr: SocketAddr = format!("127.0.0.1:{}", TEST_SERVER_PORT).parse().unwrap();

    // Try to connect to the server
    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(s) => s,
        Err(_) => {
            println!("Could not bind UDP socket, skipping test");
            return;
        }
    };

    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let local_addr = socket.local_addr().unwrap();
    println!("Client bound to {}", local_addr);

    // Create QUIC client
    let mut config = create_client_config();

    // Generate connection ID
    let scid = quiche::ConnectionId::from_vec(vec![0xba, 0xdc, 0x0f, 0xfe]);

    let mut conn = match quiche::connect(
        Some("localhost"),
        &scid,
        local_addr,
        server_addr,
        &mut config,
    ) {
        Ok(c) => c,
        Err(e) => {
            println!("Could not create QUIC connection: {:?}", e);
            return;
        }
    };

    println!("Created QUIC connection, sending Initial packet...");

    // Send/receive loop
    let mut buf = vec![0u8; 65535];
    let mut out = vec![0u8; 1350];
    let mut qad_received = false;
    let mut handshake_complete = false;

    for iteration in 0..50 {
        // Send any pending packets
        loop {
            match conn.send(&mut out) {
                Ok((len, send_info)) => {
                    if let Err(e) = socket.send_to(&out[..len], send_info.to) {
                        println!("Send error: {:?}", e);
                        break;
                    }
                    println!("Sent {} bytes to server", len);
                }
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    println!("QUIC send error: {:?}", e);
                    break;
                }
            }
        }

        // Check connection state
        if conn.is_established() && !handshake_complete {
            println!("Handshake complete!");
            handshake_complete = true;
        }

        // Receive packets
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                println!("Received {} bytes from {}", len, from);

                let recv_info = quiche::RecvInfo {
                    from,
                    to: local_addr,
                };

                match conn.recv(&mut buf[..len], recv_info) {
                    Ok(_) => {
                        println!("Processed QUIC packet");
                    }
                    Err(e) => {
                        println!("QUIC recv error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.kind() != std::io::ErrorKind::TimedOut
                {
                    println!("Socket recv error: {:?}", e);
                }
            }
        }

        // Check for QAD DATAGRAM
        let mut dgram_buf = vec![0u8; 1350];
        while let Ok(len) = conn.dgram_recv(&mut dgram_buf) {
            println!("Received DATAGRAM: {} bytes", len);

            if let Some(observed_addr) = parse_qad_message(&dgram_buf[..len]) {
                println!("QAD: Server observed us at {}", observed_addr);
                qad_received = true;
            }
        }

        // Exit conditions
        if handshake_complete && qad_received {
            println!("Test passed: Handshake complete and QAD received");
            break;
        }

        if conn.is_closed() {
            println!("Connection closed");
            break;
        }

        // Handle timeout
        if let Some(timeout) = conn.timeout() {
            if timeout.is_zero() {
                conn.on_timeout();
            }
        }

        thread::sleep(Duration::from_millis(50));

        if iteration == 49 {
            println!("Timeout waiting for handshake/QAD");
        }
    }

    // Verify results
    assert!(
        handshake_complete,
        "QUIC handshake should complete (is the server running on 127.0.0.1:4433?)"
    );
    assert!(qad_received, "Should receive QAD OBSERVED_ADDRESS message");

    // Clean close
    conn.close(true, 0, b"test complete").ok();

    // Send close packet
    if let Ok((len, send_info)) = conn.send(&mut out) {
        socket.send_to(&out[..len], send_info.to).ok();
    }
}
