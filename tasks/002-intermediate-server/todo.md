# TODO: Intermediate Server

**Task ID:** 002-intermediate-server
**Branch:** `feature/002-intermediate-server`

---

## Prerequisites

- [x] Task 001 (Agent QUIC Client) complete and merged
- [x] Create feature branch: `git checkout -b feature/002-intermediate-server`

---

## Phase 1: Project Setup

### Crate Initialization
- [ ] Create `intermediate-server/` directory at project root
- [ ] Run `cargo init --name intermediate-server`
- [ ] Add crate to workspace `Cargo.toml` (if using workspace)
- [ ] Configure Cargo.toml dependencies:
  - [ ] `quiche = "0.22"` (same version as Agent)
  - [ ] `mio = "0.8"` (event loop - matches quiche examples)
  - [ ] `ring = "0.17"` (for retry token HMAC)
  - [ ] `log = "0.4"` and `env_logger = "0.11"`

### TLS Certificates
- [ ] Create `intermediate-server/certs/` directory
- [ ] Generate dev certificate: `openssl genrsa -out key.pem 2048`
- [ ] Generate dev cert: `openssl req -new -x509 -key key.pem -out cert.pem -days 365 -subj "/CN=localhost"`

### Basic Structure
- [ ] Create `src/main.rs` with mio event loop scaffold
- [ ] Create `src/server.rs` module (empty)
- [ ] Create `src/client.rs` module (empty)
- [ ] Set up `env_logger` initialization
- [ ] Add CLI args for port and cert paths

---

## Phase 2: QUIC Server Core

### quiche Configuration
- [ ] Create `quiche::Config::new(quiche::PROTOCOL_VERSION)`
- [ ] Load certificate chain: `config.load_cert_chain_from_pem_file()`
- [ ] Load private key: `config.load_priv_key_from_pem_file()`
- [ ] **CRITICAL**: Set ALPN to match Agent: `config.set_application_protos(&[b"ztna-v1"])`
- [ ] Enable DATAGRAM: `config.enable_dgram(true, 1000, 1000)`
- [ ] Set idle timeout: `config.set_max_idle_timeout(30_000)` (matches Agent)
- [ ] Set UDP payload sizes: `set_max_recv_udp_payload_size(1350)`, `set_max_send_udp_payload_size(1350)`
- [ ] Set stream limits (match Agent values)

### QUIC Header Parsing
- [ ] Parse headers with `quiche::Header::from_slice()`
- [ ] Handle version negotiation via `quiche::negotiate_version()`
- [ ] Extract DCID for connection routing

### Stateless Retry (Anti-Amplification)
- [ ] Generate server secret (random 32 bytes at startup)
- [ ] Implement retry token mint function (HMAC of client addr + original DCID)
- [ ] Implement retry token validation function
- [ ] Call `quiche::retry()` for new connections without valid token
- [ ] Extract original DCID from validated token for `quiche::accept()`

### Connection Management
- [ ] Define `Client` struct with: `conn`, `observed_addr`, `client_type`, `registered_id`
- [ ] Create `HashMap<ConnectionId, Client>` for connection lookup
- [ ] Implement `quiche::accept()` for new connections
- [ ] Track connection states (connecting, established, draining, closed)
- [ ] Implement connection cleanup on close/timeout

### Event Loop
- [ ] Set up mio `Poll` and UDP socket registration
- [ ] Implement receive loop: `socket.recv_from()`
- [ ] Route packets to connections by DCID
- [ ] Call `conn.recv()` for each packet
- [ ] Implement send loop: drain `conn.send()` to socket
- [ ] Implement timeout handling: track per-connection timeouts, call `conn.on_timeout()`

---

## Phase 3: QAD (QUIC Address Discovery)

### Address Observation
- [ ] Store `RecvInfo.from` as `observed_addr` in Client
- [ ] Detect address changes: compare new `from` to stored address
- [ ] Log address changes for debugging

### QAD Message Implementation
- [ ] Create `build_qad_message(addr: SocketAddr) -> Vec<u8>` function
- [ ] **CRITICAL**: Use exact format matching Agent parser:
  - Byte 0: `0x01` (OBSERVED_ADDRESS type)
  - Bytes 1-4: IPv4 address octets
  - Bytes 5-6: Port in big-endian
- [ ] Total message size: 7 bytes for IPv4

### QAD Sending
- [ ] Send QAD via DATAGRAM (NOT stream) when `conn.is_established()` first becomes true
- [ ] Re-send QAD if source address changes (NAT rebinding)
- [ ] Log QAD sends with observed address

### Integration Test
- [ ] Test with Agent from Task 001
- [ ] Verify Agent logs its observed address
- [ ] Verify Agent's `observed_address` field is populated

---

## Phase 4: Client Registry

### Registration Message Protocol
- [ ] Define message format:
  - Byte 0: Type (`0x10` = Agent, `0x11` = Connector)
  - Byte 1: ID length (0-255)
  - Bytes 2+: ID string (UTF-8)
- [ ] Define `ClientType` enum: `Agent`, `Connector`
- [ ] Parse registration from first non-QAD DATAGRAM received

### Registry Implementation
- [ ] Create `Registry` struct with:
  - `connectors: HashMap<String, ConnectionId>` (service_id → conn)
  - Reverse lookup capability for Agent routing
- [ ] Implement `register_client()` function
- [ ] Implement `unregister_client()` on disconnect
- [ ] Implement `lookup_connector(service_id)` function
- [ ] Implement `lookup_paired_agent(connector_conn_id)` function

### Testing
- [ ] Unit test registration message parsing
- [ ] Unit test registry lookup functions

---

## Phase 5: DATAGRAM Relay

### Relay Logic
- [ ] Identify DATAGRAM type by first byte:
  - `0x01`: QAD message (skip relay)
  - `0x10`/`0x11`: Registration message (process, skip relay)
  - Other: Raw IP packet (relay)
- [ ] For relay: look up sender's paired connection
- [ ] Forward DATAGRAM unchanged via `dest_conn.dgram_send()`

### Error Handling
- [ ] Log when destination not found (drop packet)
- [ ] Handle destination connection closed (clean up, notify?)
- [ ] Handle `dgram_send()` errors (buffer full: drop or queue?)

### Bidirectional Testing
- [ ] Test Agent → Connector relay
- [ ] Test Connector → Agent relay
- [ ] Verify packets arrive unchanged

---

## Phase 6: Integration Testing

### Basic Connectivity
- [ ] Run server on `localhost:4433`
- [ ] Connect with Agent
- [ ] Verify handshake succeeds
- [ ] Verify QAD message received

### Connection Lifecycle
- [ ] Test clean disconnect
- [ ] Test reconnection after disconnect
- [ ] Test idle timeout (30s)
- [ ] Test multiple concurrent connections (3+)

### Stress Testing
- [ ] Send 100+ packets rapidly
- [ ] Verify stateless retry works under load
- [ ] Check for memory leaks (monitor RSS)

---

## Phase 7: Documentation & PR

### Documentation
- [ ] Update `tasks/002-intermediate-server/state.md` with completion status
- [ ] Update `tasks/_context/components.md` to mark 002 complete
- [ ] Add usage instructions to server README or main.rs comments

### Code Quality
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run `cargo fmt`
- [ ] Ensure all tests pass: `cargo test`

### PR & Merge
- [ ] Commit all changes with descriptive messages
- [ ] Push branch: `git push -u origin feature/002-intermediate-server`
- [ ] Create PR with summary of changes
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] IPv6 support for QAD (requires Agent update)
- [ ] Graceful shutdown with connection draining
- [ ] Metrics/statistics endpoint
- [ ] Configuration file support (TOML)
- [ ] Multiple bind addresses
- [ ] Rate limiting per client
