# Implementation Plan: App Connector

**Task ID:** 003-app-connector
**Branch:** `feature/003-app-connector`
**Depends On:** 002 (Intermediate Server)

---

## Goal

Build a QUIC client that:
1. Connects to the Intermediate System
2. Registers as a destination for specific services
3. Receives DATAGRAMs containing encapsulated IP packets
4. Decapsulates and forwards to local applications
5. Handles return traffic back through the tunnel

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/003-app-connector

# While working:
git add . && git commit -m "003: descriptive message"

# When complete:
git push -u origin feature/003-app-connector
# Create PR → Review → Merge to master
```

---

## Phase 1: Project Setup

### 1.1 Create Rust Crate
- [ ] Create `app-connector/` directory at project root
- [ ] Initialize with `cargo init --name app-connector`
- [ ] Add dependencies:
  - `quiche` (QUIC library)
  - `tokio` (async runtime)
  - `socket2` (raw socket for forwarding)
  - `log` + `env_logger` (logging)
  - `clap` (CLI arguments)

### 1.2 CLI Configuration
- [ ] `--server` - Intermediate System address (default: 127.0.0.1:4433)
- [ ] `--service-id` - Service identifier to register
- [ ] `--forward-to` - Local address:port to forward traffic
- [ ] `--cert` - Optional client certificate

---

## Phase 2: QUIC Client Implementation

### 2.1 Connection Setup
- [ ] Create quiche client configuration
- [ ] Connect to Intermediate System via UDP
- [ ] Handle QUIC handshake
- [ ] Enable DATAGRAM support

### 2.2 Registration Protocol
- [ ] Send registration message after handshake
- [ ] Include service_id in registration
- [ ] Confirm registration success

### 2.3 Connection Maintenance
- [ ] Handle keepalive/timeouts
- [ ] Reconnect on disconnect
- [ ] Log connection state changes

---

## Phase 3: QAD Handling

### 3.1 Receive OBSERVED_ADDRESS
- [ ] Parse QAD message from Intermediate
- [ ] Store observed public address
- [ ] Log for debugging

---

## Phase 4: DATAGRAM Processing

### 4.1 Receive DATAGRAMs
- [ ] Poll for incoming DATAGRAMs from QUIC connection
- [ ] Parse encapsulated IP packet

### 4.2 Packet Decapsulation
- [ ] Extract IP header
- [ ] Determine protocol (TCP/UDP/ICMP)
- [ ] Extract payload and ports

### 4.3 Forward to Local Service
- [ ] For TCP: Connect to local service, forward payload
- [ ] For UDP: Send to local service
- [ ] For ICMP: Handle ping responses

---

## Phase 5: Return Traffic

### 5.1 Capture Responses
- [ ] Receive response from local service
- [ ] Re-encapsulate in IP packet format

### 5.2 Send via DATAGRAM
- [ ] Package as QUIC DATAGRAM
- [ ] Send to Intermediate for relay to Agent

---

## Phase 6: Testing

### 6.1 Local Testing
- [ ] Run Intermediate (Task 002)
- [ ] Run App Connector pointing to local service
- [ ] Test with netcat or simple HTTP server
- [ ] Verify end-to-end data flow

### 6.2 With Agent
- [ ] Start Agent (Task 001)
- [ ] Verify ping reaches Connector
- [ ] Verify response returns to Agent

---

## Success Criteria

1. [ ] Connector starts and connects to Intermediate
2. [ ] Connector receives OBSERVED_ADDRESS (QAD)
3. [ ] Connector receives DATAGRAM from relayed Agent traffic
4. [ ] Traffic forwarded to local service
5. [ ] Response returns through tunnel

---

## File Structure

```
app-connector/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI, main loop
│   ├── client.rs         # QUIC client implementation
│   ├── registry.rs       # Service registration
│   ├── decapsulate.rs    # IP packet decapsulation
│   ├── forward.rs        # Local service forwarding
│   └── qad.rs            # QAD message handling
└── config/
    └── example.toml      # Example configuration
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Raw socket permissions | Document required capabilities, provide alternatives |
| TCP connection state | Start with UDP-only, add TCP later |
| Port mapping complexity | Simple 1:1 mapping for MVP |

---

## References

- [quiche client example](https://github.com/cloudflare/quiche/blob/master/quiche/examples/client.rs)
- Task 001 Agent code for QUIC client patterns
