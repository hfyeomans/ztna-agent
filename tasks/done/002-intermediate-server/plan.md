# Implementation Plan: Intermediate Server

**Task ID:** 002-intermediate-server
**Branch:** `feature/002-intermediate-server`
**Depends On:** 001 (Agent QUIC Client) ✅

---

## Goal

Build a QUIC server that:
1. Accepts connections from Agents and App Connectors
2. Implements QAD (QUIC Address Discovery) - tells clients their public IP:Port
3. Relays DATAGRAM frames between matched Agent/Connector pairs
4. Serves as bootstrap and fallback for the ZTNA system

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/002-intermediate-server

# While working:
git add . && git commit -m "002: descriptive message"

# When complete:
git push -u origin feature/002-intermediate-server
# Create PR → Review → Merge to master
```

---

## Critical Compatibility Requirements

These must match the existing Agent implementation (`core/packet_processor/src/lib.rs`):

| Parameter | Value | Agent Reference |
|-----------|-------|-----------------|
| **ALPN** | `b"ztna-v1"` | `lib.rs:28` |
| **QAD Format** | `0x01 + IPv4(4 bytes) + port(2 bytes BE)` | `lib.rs:255-262` |
| **QAD Transport** | DATAGRAM only (not Stream) | `lib.rs:251` |
| **Max DATAGRAM** | 1350 bytes | `lib.rs:22` |
| **Idle Timeout** | 30000ms | `lib.rs:25` |

---

## Phase 1: Project Setup

### 1.1 Create Rust Crate
- [ ] Create `intermediate-server/` directory at project root
- [ ] Initialize with `cargo init --name intermediate-server`
- [ ] Add to workspace `Cargo.toml` if using workspace
- [ ] Add dependencies to Cargo.toml:
  - `quiche = "0.22"` (QUIC library - same version as Agent)
  - `mio = "0.8"` (event loop - matches quiche examples)
  - `ring = "0.17"` (crypto for retry tokens)
  - `log = "0.4"` + `env_logger = "0.11"` (logging)

### 1.2 Basic Server Structure
- [ ] Create main.rs with mio event loop
- [ ] Create UDP socket binding to `0.0.0.0:4433`
- [ ] Set up logging infrastructure with `env_logger`
- [ ] Add CLI arguments (port, cert paths) via `std::env::args`

### 1.3 TLS Certificates
- [ ] Generate self-signed cert for development:
  ```bash
  openssl genrsa -out key.pem 2048
  openssl req -new -x509 -key key.pem -out cert.pem -days 365 -subj "/CN=localhost"
  ```
- [ ] Store in `intermediate-server/certs/`

---

## Phase 2: QUIC Server Implementation

### 2.1 quiche Configuration
- [ ] Create `quiche::Config` with server settings:
  ```rust
  let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;
  config.load_cert_chain_from_pem_file("certs/cert.pem")?;
  config.load_priv_key_from_pem_file("certs/key.pem")?;

  // CRITICAL: Must match Agent ALPN
  config.set_application_protos(&[b"ztna-v1"])?;

  // Enable DATAGRAM support (for QAD and IP tunneling)
  config.enable_dgram(true, 1000, 1000);

  // Timeouts and limits (match Agent)
  config.set_max_idle_timeout(30_000);
  config.set_max_recv_udp_payload_size(1350);
  config.set_max_send_udp_payload_size(1350);
  config.set_initial_max_data(10_000_000);
  config.set_initial_max_stream_data_bidi_local(1_000_000);
  config.set_initial_max_stream_data_bidi_remote(1_000_000);
  config.set_initial_max_streams_bidi(100);
  ```

### 2.2 QUIC Header Parsing
- [ ] Parse incoming UDP packets with `quiche::Header::from_slice()`
- [ ] Handle version negotiation for unsupported versions
- [ ] Extract destination connection ID (DCID) for routing

### 2.3 Stateless Retry (Anti-Amplification)
- [ ] Generate retry token using HMAC with server secret
- [ ] Validate retry token on subsequent Initial packets
- [ ] Mint original DCID for connection tracking
- [ ] Send `quiche::negotiate_version()` or `quiche::retry()` as needed

### 2.4 Connection Management
- [ ] Create `Client` struct to track each connection:
  ```rust
  struct Client {
      conn: quiche::Connection,
      observed_addr: SocketAddr,
      client_type: Option<ClientType>,
      registered_id: Option<String>,
  }
  ```
- [ ] HashMap for connection ID → Client lookup
- [ ] Accept connections with `quiche::accept()`
- [ ] Track connection state (connecting, established, draining)

### 2.5 Event Loop
- [ ] mio-based UDP socket polling
- [ ] Receive UDP packets from socket
- [ ] Route to correct `quiche::Connection` by DCID
- [ ] Process QUIC events (handshake, streams, datagrams)
- [ ] Send outbound packets via `conn.send()`
- [ ] Handle timeouts via `conn.on_timeout()`

---

## Phase 3: QAD Implementation

### 3.1 Address Observation
- [ ] Extract source IP:Port from `RecvInfo.from`
- [ ] Store observed address per connection
- [ ] Detect address changes (NAT rebinding)

### 3.2 OBSERVED_ADDRESS Message
**Format (must match Agent parser at `lib.rs:255-262`):**
```
+------+----------+----------+----------+----------+----------+----------+
| 0x01 | IPv4[0]  | IPv4[1]  | IPv4[2]  | IPv4[3]  | Port[HI] | Port[LO] |
+------+----------+----------+----------+----------+----------+----------+
  1 byte           4 bytes (IPv4)              2 bytes (big-endian)
```

- [ ] Create QAD message builder function
- [ ] Send via DATAGRAM (NOT Stream) immediately after handshake
- [ ] Resend on address change detection

### 3.3 Client Notification
- [ ] Send QAD message when `conn.is_established()` becomes true
- [ ] Log observed addresses for debugging
- [ ] Re-send QAD if `RecvInfo.from` differs from stored address

---

## Phase 4: Client Registry

### 4.1 Client Registration Protocol
Since ALPN is fixed to `ztna-v1`, client type must be communicated via a registration message.

**Registration Message Format (sent by client after handshake):**
```
+------+------+------------------+
| Type | Len  | ID (UTF-8)       |
+------+------+------------------+
  1      1      variable

Type: 0x10 = Agent, 0x11 = Connector
Len: Length of ID string (0-255)
ID: Service/destination identifier
```

- [ ] Define `ClientType` enum: `Agent`, `Connector`
- [ ] Parse registration message from first received DATAGRAM
- [ ] Store type and ID in Client struct

### 4.2 Routing Table
- [ ] Map service IDs to Connector connections
- [ ] Map Agent connections to their target service ID
- [ ] Handle connection cleanup on disconnect
- [ ] Support multiple Agents connecting to same Connector

---

## Phase 5: DATAGRAM Relay

### 5.1 Relay Model
**Key Decision:** Agent sends raw IP packets (no routing header). Routing is based on connection registration, not packet contents.

```
Agent (registered for service "app1")
    │
    │ DATAGRAM: raw IP packet
    ▼
Intermediate looks up: Agent's target → "app1"
Intermediate looks up: Connector for "app1" → conn_id_xyz
    │
    │ DATAGRAM: raw IP packet (forwarded unchanged)
    ▼
Connector (registered as "app1")
```

### 5.2 Receive and Relay
- [ ] Receive DATAGRAMs via `conn.dgram_recv()`
- [ ] Skip QAD messages (type 0x01) and registration messages (type 0x10/0x11)
- [ ] For other DATAGRAMs (raw IP packets):
  - Look up sender in registry
  - Find paired connection (Agent→Connector or Connector→Agent)
  - Forward via `dest_conn.dgram_send()`

### 5.3 Error Handling
- [ ] Handle destination not found (log, drop packet)
- [ ] Handle destination connection closed (clean up pairing)
- [ ] Handle DATAGRAM send failures (buffer full, etc.)

---

## Phase 6: Testing with Agent

### 6.1 Local Testing
- [ ] Run server on localhost:4433
- [ ] Start Agent (from Task 001)
- [ ] Verify QUIC handshake succeeds
- [ ] Verify Agent receives OBSERVED_ADDRESS (QAD)
- [ ] Check logs on both sides

### 6.2 Connection Lifecycle
- [ ] Test connect/disconnect cycles
- [ ] Test multiple concurrent connections
- [ ] Test idle timeout handling
- [ ] Verify clean shutdown without resource leaks

### 6.3 Basic Relay Test (Placeholder)
- [ ] Create simple mock connector (can be done in Phase 3 task)
- [ ] Verify DATAGRAM forwarding works bidirectionally

---

## Success Criteria

1. [ ] Server starts and listens on UDP port 4433
2. [ ] Agent connects successfully via QUIC (ALPN `ztna-v1`)
3. [ ] Agent receives OBSERVED_ADDRESS in correct format
4. [ ] Server logs show connection events
5. [ ] Stateless retry works (tested with high packet rate)
6. [ ] Clean shutdown without resource leaks

---

## File Structure

```
intermediate-server/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI, mio event loop
│   ├── server.rs         # QUIC server: config, accept, send/recv
│   ├── client.rs         # Client connection state
│   ├── registry.rs       # Client registry and routing table
│   ├── qad.rs            # QAD message builder
│   └── relay.rs          # DATAGRAM relay logic
└── certs/
    ├── cert.pem          # Development certificate
    └── key.pem           # Development private key
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| ALPN mismatch breaks handshake | Explicitly set `b"ztna-v1"` to match Agent |
| QAD format incompatibility | Use exact format from Agent parser |
| Amplification attacks | Implement stateless retry tokens |
| Connection ID management | Use quiche's built-in handling via `accept()` |
| Memory leaks from orphan connections | Implement timeout-based cleanup |
| NAT rebinding goes undetected | Compare `RecvInfo.from` to stored address |

---

## References

- [quiche server example](https://github.com/cloudflare/quiche/blob/master/quiche/examples/server.rs)
- [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000)
- [QUIC DATAGRAM RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221)
- Agent implementation: `core/packet_processor/src/lib.rs`
