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

## Phase 1: Project Setup

### 1.1 Create Rust Crate
- [ ] Create `intermediate-server/` directory at project root
- [ ] Initialize with `cargo init --name intermediate-server`
- [ ] Add dependencies to Cargo.toml:
  - `quiche` (QUIC library)
  - `tokio` (async runtime)
  - `mio` (event loop)
  - `ring` (crypto)
  - `log` + `env_logger` (logging)

### 1.2 Basic Server Structure
- [ ] Create main.rs with tokio runtime
- [ ] Create UDP socket binding to `0.0.0.0:4433`
- [ ] Set up logging infrastructure
- [ ] Add CLI arguments (port, cert paths)

---

## Phase 2: QUIC Server Implementation

### 2.1 TLS Configuration
- [ ] Generate self-signed cert for development
- [ ] Create `quiche::Config` with server settings
- [ ] Enable DATAGRAM support
- [ ] Set appropriate timeouts and limits

### 2.2 Connection Management
- [ ] Create `Client` struct to track each connection
- [ ] HashMap for connection ID → Client lookup
- [ ] Handle QUIC handshake
- [ ] Track connection state (connecting, established, draining)

### 2.3 Packet Processing Loop
- [ ] Receive UDP packets from socket
- [ ] Route to correct `quiche::Connection`
- [ ] Process QUIC events
- [ ] Send outbound packets

---

## Phase 3: QAD Implementation

### 3.1 Address Observation
- [ ] Extract source IP:Port from each UDP packet
- [ ] Store observed address per connection

### 3.2 OBSERVED_ADDRESS Message
- [ ] Define message format (type byte + IP + port)
- [ ] Send via DATAGRAM or Stream 0
- [ ] Send on connection establishment
- [ ] Resend on address change detection

### 3.3 Client Notification
- [ ] Notify client immediately after handshake
- [ ] Log observed addresses for debugging

---

## Phase 4: Client Registry

### 4.1 Client Types
- [ ] Define `ClientType` enum: Agent, Connector
- [ ] Parse client type from ALPN or initial message
- [ ] Store type in Client struct

### 4.2 Routing Table
- [ ] Map destination IDs to Connector connections
- [ ] Map Agent connections to their target destinations
- [ ] Handle connection cleanup on disconnect

---

## Phase 5: DATAGRAM Relay

### 5.1 Receive DATAGRAMs
- [ ] Process incoming DATAGRAMs from connections
- [ ] Parse routing header (destination ID)

### 5.2 Relay Logic
- [ ] Look up destination in routing table
- [ ] Forward DATAGRAM to destination connection
- [ ] Handle destination not found (drop or queue)

### 5.3 Bidirectional Relay
- [ ] Agent → Intermediate → Connector
- [ ] Connector → Intermediate → Agent

---

## Phase 6: Testing with Agent

### 6.1 Local Testing
- [ ] Run server on localhost:4433
- [ ] Start Agent, verify connection
- [ ] Verify QAD message received by Agent
- [ ] Check logs on both sides

### 6.2 Connection Lifecycle
- [ ] Test connect/disconnect cycles
- [ ] Test multiple concurrent connections
- [ ] Verify clean shutdown

---

## Success Criteria

1. [ ] Server starts and listens on UDP port
2. [ ] Agent connects successfully via QUIC
3. [ ] Agent receives OBSERVED_ADDRESS (QAD)
4. [ ] Server logs show connection events
5. [ ] Clean shutdown without resource leaks

---

## File Structure

```
intermediate-server/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI, server loop
│   ├── server.rs         # QUIC server implementation
│   ├── client.rs         # Client connection state
│   ├── registry.rs       # Client registry and routing
│   ├── qad.rs            # QAD message handling
│   └── relay.rs          # DATAGRAM relay logic
└── certs/
    ├── cert.pem          # Development certificate
    └── key.pem           # Development private key
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| quiche API learning curve | Reference quiche examples |
| Certificate handling | Use self-signed for dev, document production setup |
| Connection ID management | Use quiche's built-in connection ID handling |
| Memory leaks from orphan connections | Implement timeout-based cleanup |

---

## References

- [quiche server example](https://github.com/cloudflare/quiche/blob/master/quiche/examples/server.rs)
- [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000)
- [QUIC DATAGRAM RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221)
