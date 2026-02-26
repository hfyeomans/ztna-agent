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
//!
//! Phase 4 - Advanced Testing:
//!   quic-test-client --service test --payload-size 100 --payload-pattern random --repeat 10
//!   quic-test-client --service test --burst 50 --payload-size 100
//!
//! Phase 6 - Performance Metrics:
//!   quic-test-client --service test --measure-rtt --rtt-count 100 --dst 127.0.0.1:9999

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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let server_addr = parse_arg(&args, "--server").unwrap_or_else(|| "127.0.0.1:4433".to_string());
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
    let payload_size_arg = parse_arg(&args, "--payload-size");
    let expect_connect_fail = args.iter().any(|a| a == "--expect-fail");
    // Phase 3.5: Query max DATAGRAM size programmatically
    let query_max_size = args.iter().any(|a| a == "--query-max-size");

    // Phase 4: Advanced testing options
    let payload_pattern = parse_arg(&args, "--payload-pattern");
    let repeat_count: usize = parse_arg(&args, "--repeat")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let burst_count: usize = parse_arg(&args, "--burst")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let verify_echo = args.iter().any(|a| a == "--verify-echo");
    let packet_delay_ms: u64 = parse_arg(&args, "--delay")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Phase 6A: mTLS client certificate options
    let client_cert_path = parse_arg(&args, "--client-cert");
    let client_key_path = parse_arg(&args, "--client-key");
    let ca_cert_path = parse_arg(&args, "--ca-cert");

    // Phase 6: Performance metrics options
    let measure_rtt = args.iter().any(|a| a == "--measure-rtt");
    let rtt_count: usize = parse_arg(&args, "--rtt-count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let measure_handshake = args.iter().any(|a| a == "--measure-handshake");

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return Ok(());
    }

    let server_addr: SocketAddr = server_addr.parse().map_err(|_| "Invalid server address")?;

    // Determine ALPN to use
    let alpn_bytes: Vec<u8> = alpn_override
        .as_ref()
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

    if let Some(ref cert) = client_cert_path {
        log::info!("  Client cert: {}", cert);
    }
    if let Some(ref key) = client_key_path {
        log::info!("  Client key:  {}", key);
    }
    if let Some(ref ca) = ca_cert_path {
        log::info!("  CA cert:     {}", ca);
    }

    let mut client = QuicTestClient::new(
        server_addr,
        &alpn_bytes,
        client_cert_path.as_deref(),
        client_key_path.as_deref(),
        ca_cert_path.as_deref(),
    )?;

    // Connect and establish QUIC session (with optional handshake timing)
    let handshake_start = Instant::now();
    client.connect()?;
    match client.wait_for_connection(Duration::from_secs(5)) {
        Ok(_) => {
            let handshake_elapsed = handshake_start.elapsed();
            if expect_connect_fail {
                log::error!("Connection succeeded but expected failure!");
                std::process::exit(1);
            }
            if measure_handshake {
                let handshake_us = handshake_elapsed.as_micros();
                log::info!(
                    "Handshake completed in {} µs ({:.3} ms)",
                    handshake_us,
                    handshake_us as f64 / 1000.0
                );
                println!("HANDSHAKE_US:{}", handshake_us);
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

    // Query and display max DATAGRAM size (Phase 3.5: programmatic sizing)
    let max_dgram_size = client.get_max_datagram_size();
    if query_max_size || max_dgram_size.is_some() {
        if let Some(max_size) = max_dgram_size {
            log::info!("Max DATAGRAM writable size: {} bytes", max_size);
            // IP header (20) + UDP header (8) = 28 bytes overhead
            let max_udp_payload = max_size.saturating_sub(28);
            log::info!(
                "Max UDP payload (after IP/UDP headers): {} bytes",
                max_udp_payload
            );
            if query_max_size {
                println!("MAX_DGRAM_SIZE:{}", max_size);
                println!("MAX_UDP_PAYLOAD:{}", max_udp_payload);
            }
        } else {
            log::warn!("DATAGRAM max size not available (connection may not support datagrams)");
        }
    }

    // Resolve payload size: "max" uses programmatic max, otherwise parse as number
    let payload_size: Option<usize> = match payload_size_arg.as_deref() {
        Some("max") => {
            // Use max writable size minus IP/UDP overhead (28 bytes)
            max_dgram_size.map(|m| m.saturating_sub(28))
        }
        Some("max-1") => {
            // One byte under max (boundary test: should succeed)
            max_dgram_size.map(|m| m.saturating_sub(29))
        }
        Some("max+1") => {
            // One byte over max (boundary test: should fail)
            max_dgram_size.map(|m| m.saturating_sub(27))
        }
        Some(s) => s.parse().ok(),
        None => None,
    };

    // Register as Agent if service specified
    if let Some(ref svc) = service_id {
        client.register_as_agent(svc)?;
        // Brief wait for registration to propagate
        client.wait_for_responses(Duration::from_millis(200))?;
    }

    // Phase 6: RTT measurement mode
    if measure_rtt {
        let size = payload_size.unwrap_or(64);
        let rng = SystemRandom::new();
        let pattern_str = payload_pattern.as_deref();

        let dst: SocketAddrV4 = dst_addr
            .as_ref()
            .ok_or("RTT measurement requires --dst address")?
            .parse()
            .map_err(|_| "Invalid --dst address")?;
        let src: SocketAddrV4 = src_addr
            .clone()
            .unwrap_or_else(|| "10.0.0.100:12345".to_string())
            .parse()
            .map_err(|_| "Invalid --src address")?;

        log::info!(
            "RTT measurement: {} samples, {} byte payload",
            rtt_count,
            size
        );

        let mut rtts: Vec<u128> = Vec::with_capacity(rtt_count);
        let mut timeouts = 0;

        for i in 0..rtt_count {
            let payload = generate_payload(size, pattern_str, &rng);
            let packet = build_ip_udp_packet(src, dst, &payload);

            let send_time = Instant::now();
            client.send_datagram(&packet)?;

            // Wait for response with timeout
            match client.wait_for_first_response(Duration::from_millis(1000)) {
                Ok(true) => {
                    let rtt_us = send_time.elapsed().as_micros();
                    rtts.push(rtt_us);
                    log::debug!("RTT sample {}: {} µs", i + 1, rtt_us);
                }
                Ok(false) => {
                    timeouts += 1;
                    log::debug!("RTT sample {}: timeout", i + 1);
                }
                Err(e) => {
                    log::warn!("RTT sample {} error: {}", i + 1, e);
                    timeouts += 1;
                }
            }

            // Small delay between samples to avoid overwhelming
            if i + 1 < rtt_count {
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        // Calculate statistics
        if !rtts.is_empty() {
            rtts.sort();
            let count = rtts.len();
            let min = rtts[0];
            let max = rtts[count - 1];
            let sum: u128 = rtts.iter().sum();
            let avg = sum / count as u128;
            let p50 = rtts[count * 50 / 100];
            let p95 = rtts[count * 95 / 100];
            let p99 = rtts[(count * 99 / 100).min(count - 1)];

            log::info!("RTT Statistics ({} samples, {} timeouts):", count, timeouts);
            log::info!("  Min: {} µs ({:.3} ms)", min, min as f64 / 1000.0);
            log::info!("  Max: {} µs ({:.3} ms)", max, max as f64 / 1000.0);
            log::info!("  Avg: {} µs ({:.3} ms)", avg, avg as f64 / 1000.0);
            log::info!("  p50: {} µs ({:.3} ms)", p50, p50 as f64 / 1000.0);
            log::info!("  p95: {} µs ({:.3} ms)", p95, p95 as f64 / 1000.0);
            log::info!("  p99: {} µs ({:.3} ms)", p99, p99 as f64 / 1000.0);

            // Output for parsing
            println!("RTT_COUNT:{}", count);
            println!("RTT_TIMEOUTS:{}", timeouts);
            println!("RTT_MIN_US:{}", min);
            println!("RTT_MAX_US:{}", max);
            println!("RTT_AVG_US:{}", avg);
            println!("RTT_P50_US:{}", p50);
            println!("RTT_P95_US:{}", p95);
            println!("RTT_P99_US:{}", p99);
        } else {
            log::error!(
                "No successful RTT samples collected ({} timeouts)",
                timeouts
            );
            println!("RTT_COUNT:0");
            println!("RTT_TIMEOUTS:{}", timeouts);
        }

        return Ok(());
    }

    if interactive {
        // Interactive mode: read from stdin
        client.run_interactive()?;
    } else if burst_count > 0 {
        // Phase 4: Burst mode - send N packets as fast as possible
        let size = payload_size.unwrap_or(100);
        let rng = SystemRandom::new();
        let pattern_str = payload_pattern.as_deref();

        log::info!(
            "Burst mode: sending {} packets of {} bytes (pattern: {:?})",
            burst_count,
            size,
            pattern_str.unwrap_or("sequential")
        );

        let dst: SocketAddrV4 = dst_addr
            .as_ref()
            .ok_or("Burst mode requires --dst address")?
            .parse()
            .map_err(|_| "Invalid --dst address")?;
        let src: SocketAddrV4 = src_addr
            .unwrap_or_else(|| "10.0.0.100:12345".to_string())
            .parse()
            .map_err(|_| "Invalid --src address")?;

        let start = Instant::now();
        let mut sent = 0;

        for i in 0..burst_count {
            let payload = generate_payload(size, pattern_str, &rng);
            let packet = build_ip_udp_packet(src, dst, &payload);
            client.send_datagram(&packet)?;
            sent += 1;
            if (i + 1) % 10 == 0 {
                log::debug!("Sent {}/{} packets", i + 1, burst_count);
            }
        }

        let elapsed = start.elapsed();
        let pps = if elapsed.as_secs_f64() > 0.0 {
            sent as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        log::info!(
            "Burst complete: {} packets in {:?} ({:.1} pps)",
            sent,
            elapsed,
            pps
        );
        println!("BURST_SENT:{}", sent);
        println!("BURST_PPS:{:.1}", pps);

        // Wait for responses
        client.wait_for_responses(Duration::from_millis(wait_ms))?;
    } else if let Some(size) = payload_size {
        // Generate payload of specified size (for boundary/pattern testing)
        let rng = SystemRandom::new();
        let pattern_str = payload_pattern.as_deref();
        let effective_repeat = if repeat_count > 0 { repeat_count } else { 1 };

        log::info!(
            "Generated payload: {} bytes, pattern: {:?}, repeat: {}",
            size,
            pattern_str.unwrap_or("sequential"),
            effective_repeat
        );

        // If dst specified, wrap in IP/UDP
        if let Some(ref dst_str) = dst_addr {
            let dst: SocketAddrV4 = dst_str
                .parse()
                .map_err(|_| "Invalid --dst address (expected ip:port)")?;
            let src: SocketAddrV4 = src_addr
                .unwrap_or_else(|| "10.0.0.100:12345".to_string())
                .parse()
                .map_err(|_| "Invalid --src address (expected ip:port)")?;

            let mut sent_payloads: Vec<Vec<u8>> = Vec::new();

            for i in 0..effective_repeat {
                let payload = generate_payload(size, pattern_str, &rng);
                let packet = build_ip_udp_packet(src, dst, &payload);
                log::info!(
                    "[{}/{}] Built IP/UDP packet: {} bytes total",
                    i + 1,
                    effective_repeat,
                    packet.len()
                );

                if verify_echo {
                    sent_payloads.push(payload.clone());
                }

                client.send_datagram(&packet)?;

                if packet_delay_ms > 0 && i < effective_repeat - 1 {
                    std::thread::sleep(Duration::from_millis(packet_delay_ms));
                }
            }

            // Collect responses
            let responses = client.wait_for_responses_collect(Duration::from_millis(wait_ms))?;

            // Verify echo if requested
            if verify_echo {
                verify_echo_responses(&sent_payloads, &responses, src, dst);
            }
        } else {
            // Send raw payload (no IP/UDP wrapping)
            for i in 0..effective_repeat {
                let payload = generate_payload(size, pattern_str, &rng);
                log::info!(
                    "[{}/{}] Sending raw payload: {} bytes",
                    i + 1,
                    effective_repeat,
                    payload.len()
                );
                client.send_datagram(&payload)?;

                if packet_delay_ms > 0 && i < effective_repeat - 1 {
                    std::thread::sleep(Duration::from_millis(packet_delay_ms));
                }
            }
            client.wait_for_responses(Duration::from_millis(wait_ms))?;
        }
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
        log::info!(
            "Built IP/UDP packet: {} bytes (payload: {} bytes)",
            packet.len(),
            data.len()
        );
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
    eprintln!("                     Special values: 'max', 'max-1', 'max+1' (programmatic sizing)");
    eprintln!("  --expect-fail      Expect connection to fail (negative test)");
    eprintln!();
    eprintln!("Phase 3.5 - Programmatic DATAGRAM Sizing:");
    eprintln!("  --query-max-size   Print MAX_DGRAM_SIZE and MAX_UDP_PAYLOAD after connection");
    eprintln!();
    eprintln!("Phase 6A - mTLS Client Authentication:");
    eprintln!("  --client-cert PATH Client certificate PEM file for mTLS");
    eprintln!("  --client-key PATH  Client private key PEM file for mTLS");
    eprintln!("  --ca-cert PATH     CA certificate PEM file for server verification");
    eprintln!();
    eprintln!("Phase 4 - Advanced Testing:");
    eprintln!("  --payload-pattern P  Payload pattern: zeros, ones, sequential, random");
    eprintln!("  --repeat N           Send N packets (default: 1)");
    eprintln!("  --delay MS           Delay between packets in repeat mode (default: 0)");
    eprintln!("  --burst N            Burst mode: send N packets as fast as possible");
    eprintln!("  --verify-echo        Verify echoed responses match sent data");
    eprintln!();
    eprintln!("Phase 6 - Performance Metrics:");
    eprintln!("  --measure-rtt        Measure round-trip time (outputs RTT_* statistics)");
    eprintln!("  --rtt-count N        Number of RTT samples to collect (default: 10)");
    eprintln!("  --measure-handshake  Measure QUIC handshake time (outputs HANDSHAKE_US)");
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
    eprintln!("  # Phase 4: Echo integrity with random payload");
    eprintln!(
        "  quic-test-client --service test-service --payload-size 100 --payload-pattern random \\"
    );
    eprintln!("    --dst 127.0.0.1:9999 --repeat 5 --verify-echo");
    eprintln!();
    eprintln!("  # Phase 4: Burst stress test (50 packets)");
    eprintln!("  quic-test-client --service test-service --burst 50 --payload-size 100 --dst 127.0.0.1:9999");
    eprintln!();
    eprintln!("  # Phase 6: Measure RTT with 100 samples");
    eprintln!("  quic-test-client --service test-service --measure-rtt --rtt-count 100 --dst 127.0.0.1:9999");
    eprintln!();
    eprintln!("  # Phase 6: Measure handshake time");
    eprintln!("  quic-test-client --measure-handshake --service test-service --send-udp 'test' --dst 127.0.0.1:9999");
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

/// Generate payload of specified size with given pattern
/// Patterns: "zeros", "ones", "sequential", "random"
fn generate_payload(size: usize, pattern: Option<&str>, rng: &SystemRandom) -> Vec<u8> {
    match pattern {
        Some("zeros") => vec![0u8; size],
        Some("ones") => vec![0xFFu8; size],
        Some("random") => {
            let mut payload = vec![0u8; size];
            let _ = rng.fill(&mut payload);
            payload
        }
        Some("sequential") | None => {
            // Default: sequential 0x00, 0x01, 0x02...
            (0..size).map(|i| (i % 256) as u8).collect()
        }
        Some(other) => {
            log::warn!("Unknown pattern '{}', using sequential", other);
            (0..size).map(|i| (i % 256) as u8).collect()
        }
    }
}

/// Verify echo responses match sent payloads
/// Response packets are IP/UDP packets, so we need to extract the UDP payload
fn verify_echo_responses(
    sent_payloads: &[Vec<u8>],
    responses: &[Vec<u8>],
    _src: SocketAddrV4,
    _dst: SocketAddrV4,
) {
    log::info!(
        "Verifying {} sent vs {} received",
        sent_payloads.len(),
        responses.len()
    );

    let mut matches = 0;
    let mut mismatches = 0;

    for (i, sent) in sent_payloads.iter().enumerate() {
        // Find matching response (responses are IP/UDP packets)
        let mut found = false;
        for response in responses {
            // Extract UDP payload from IP/UDP packet
            // IP header: 20 bytes, UDP header: 8 bytes
            if response.len() >= 28 {
                let payload_offset = 28; // Skip IP + UDP headers
                let received_payload = &response[payload_offset..];

                if received_payload == sent.as_slice() {
                    found = true;
                    break;
                }
            }
        }

        if found {
            matches += 1;
            log::debug!("[{}] Payload verified: {} bytes match", i + 1, sent.len());
        } else {
            mismatches += 1;
            log::warn!(
                "[{}] Payload mismatch or not found: {} bytes",
                i + 1,
                sent.len()
            );
        }
    }

    log::info!(
        "Echo verification: {} matches, {} mismatches out of {} sent",
        matches,
        mismatches,
        sent_payloads.len()
    );
    println!("VERIFY_MATCHES:{}", matches);
    println!("VERIFY_MISMATCHES:{}", mismatches);
    println!("VERIFY_TOTAL:{}", sent_payloads.len());

    if mismatches == 0 && matches == sent_payloads.len() {
        println!("VERIFY_RESULT:PASS");
    } else {
        println!("VERIFY_RESULT:FAIL");
    }
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
    fn new(
        server_addr: SocketAddr,
        alpn: &[u8],
        client_cert: Option<&str>,
        client_key: Option<&str>,
        ca_cert: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

        // 6A.9: Load client certificate and key for mTLS
        if let (Some(cert_path), Some(key_path)) = (client_cert, client_key) {
            config.load_cert_chain_from_pem_file(cert_path)?;
            config.load_priv_key_from_pem_file(key_path)?;
            log::info!("Loaded client certificate for mTLS");
        }

        // Load CA certificate for server verification
        if let Some(ca_path) = ca_cert {
            config.load_verify_locations_from_file(ca_path)?;
            config.verify_peer(true);
            log::info!("Loaded CA certificate, peer verification enabled");
        } else {
            // Disable certificate verification (for testing with self-signed certs)
            config.verify_peer(false);
        }

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
        let conn = quiche::connect(None, &scid, local_addr, self.server_addr, &mut self.config)?;

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
            let poll_timeout = self
                .conn
                .as_ref()
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

    /// Get the maximum DATAGRAM size that can be sent on this connection.
    /// Returns None if the connection doesn't support DATAGRAMs or isn't established.
    fn get_max_datagram_size(&self) -> Option<usize> {
        self.conn.as_ref().and_then(|conn| {
            if conn.is_established() {
                conn.dgram_max_writable_len()
            } else {
                None
            }
        })
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
            let poll_timeout = self
                .conn
                .as_ref()
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

    /// Wait for responses and collect them (for verification)
    fn wait_for_responses_collect(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
        let start = Instant::now();
        let mut events = Events::with_capacity(64);
        let mut responses: Vec<Vec<u8>> = Vec::new();

        log::info!("Waiting for responses ({} ms)...", timeout.as_millis());

        while start.elapsed() < timeout {
            let remaining = timeout.saturating_sub(start.elapsed());
            let poll_timeout = self
                .conn
                .as_ref()
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
                    log::info!("Received DATAGRAM: {} bytes", len);
                    log::info!("  Hex: {}", hex_encode(&buf[..len]));
                    // Print to stdout for test scripts
                    println!("RECV:{}", hex_encode(&buf[..len]));
                    responses.push(buf[..len].to_vec());
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

        log::info!("Collected {} responses", responses.len());
        Ok(responses)
    }

    /// Wait for first data response (for RTT measurement)
    /// Returns Ok(true) if data response received, Ok(false) if timeout, Err on error
    /// Ignores QAD responses (7 bytes starting with 0x01)
    fn wait_for_first_response(
        &mut self,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let start = Instant::now();
        let mut events = Events::with_capacity(64);

        while start.elapsed() < timeout {
            let remaining = timeout.saturating_sub(start.elapsed());
            let poll_timeout = self
                .conn
                .as_ref()
                .and_then(|c| c.timeout())
                .map(|t| t.min(remaining))
                .or(Some(remaining.min(Duration::from_millis(10))));

            self.poll.poll(&mut events, poll_timeout)?;

            // Process incoming
            self.process_socket()?;

            // Check for DATAGRAMs
            if let Some(ref mut conn) = self.conn {
                let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
                while let Ok(len) = conn.dgram_recv(&mut buf) {
                    // Skip QAD responses (7 bytes starting with 0x01)
                    if len == 7 && buf[0] == 0x01 {
                        log::trace!("Ignoring QAD response for RTT measurement");
                        continue;
                    }
                    // Data response received - RTT complete
                    log::trace!("RTT response: {} bytes", len);
                    return Ok(true);
                }

                conn.on_timeout();
            }

            self.flush()?;

            if let Some(ref conn) = self.conn {
                if conn.is_closed() {
                    return Err("Connection closed".into());
                }
            }
        }

        Ok(false) // Timeout
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
