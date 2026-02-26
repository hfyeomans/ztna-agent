# Implementation Plan: P2P Hole Punching ✅ COMPLETE

**Task ID:** 005-p2p-hole-punching
**Status:** ✅ COMPLETE - Merged to Master
**Branch:** `master` (merged from `feature/005-p2p-hole-punching`)
**PR:** https://github.com/hfyeomans/ztna-agent/pull/5
**Depends On:** 002, 003, 004
**Last Updated:** 2026-01-23
**Oracle Review:** 2026-01-20 (see recommendations applied below)

---

## Goal

Implement direct peer-to-peer connectivity via NAT hole punching. This is the **primary architectural goal** - relay is only used when direct connection fails.

---

## Critical Clarification: P2P vs Path Migration

**Important:** P2P hole punching establishes a **NEW QUIC connection** directly between Agent and Connector. This is different from QUIC path migration (same connection, different network path).

```
RELAY PATH (existing):
  Agent ──QUIC Connection A──► Intermediate ◄──QUIC Connection B── Connector

P2P PATH (new):
  Agent ──QUIC Connection C──────────────────────────────────────► Connector
           (bypasses Intermediate entirely)
```

The Intermediate coordinates hole punching, but the resulting direct connection is independent.

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/005-p2p-hole-punching

# While working:
git add . && git commit -m "005: descriptive message"

# When complete:
git push -u origin feature/005-p2p-hole-punching
# Create PR → Review → Merge to master
```

---

## Phase 0: Socket Architecture (Foundation)

> **Critical:** Single socket reuse is required for hole punching to work.

### 0.1 Problem Statement

For hole punching, the reflexive address discovered via QAD must match the port used for direct P2P communication. If Agent/Connector use different sockets:
- QAD reports port 50000 (from Intermediate connection socket)
- Direct P2P attempts on port 50001 (new socket)
- NAT mapping doesn't exist for 50001 → hole punch fails

### 0.2 Socket Strategy

```rust
// CORRECT: Single socket for both Intermediate AND P2P
let socket = UdpSocket::bind("0.0.0.0:0")?;  // OS assigns port
// Use same socket for:
// 1. QUIC connection to Intermediate
// 2. Direct P2P QUIC connection to peer

// WRONG: Separate sockets
let intermediate_socket = UdpSocket::bind("0.0.0.0:0")?;  // Port 50000
let p2p_socket = UdpSocket::bind("0.0.0.0:0")?;           // Port 50001 ← NAT mapping doesn't exist!
```

### 0.3 Connector as QUIC Server

For P2P, the Connector must accept incoming QUIC connections from the Agent:
- Connector runs QUIC server listener (in addition to client to Intermediate)
- Requires TLS certificate/key for server mode
- Uses same socket for both roles

---

## Phase 0: Implementation Details (Completed)

> **Status:** ✅ Complete as of 2026-01-20

### Agent Multi-Connection Architecture

The Agent (`core/packet_processor/src/lib.rs`) now supports multiple QUIC connections simultaneously:

```rust
// Connection wrapper for P2P peers
struct P2PConnection {
    conn: Connection,
    last_activity: Instant,
}

// Agent struct with multi-connection support
pub struct Agent {
    config: Config,
    intermediate_conn: Option<Connection>,           // Connection to Intermediate Server
    intermediate_addr: Option<SocketAddr>,           // Intermediate Server address
    p2p_conns: HashMap<SocketAddr, P2PConnection>,   // P2P connections to Connectors
    local_addr: Option<SocketAddr>,
    state: AgentState,
    last_activity: Instant,
    pub observed_address: Option<SocketAddr>,
    scratch_buffer: Vec<u8>,
}
```

**Key Design Decisions:**

1. **Separate Connection Tracking:** `intermediate_conn` for Intermediate Server, `p2p_conns` HashMap for multiple P2P Connectors
2. **Address-Based Routing:** Incoming packets routed by source address (`recv()` method)
3. **Unified Timeout Handling:** `timeout()` returns minimum across all connections
4. **Last Activity Tracking:** Each P2P connection tracks its last activity for keepalive

**New FFI Functions:**

| Function | Purpose |
|----------|---------|
| `agent_connect_p2p(host, port)` | Establish P2P connection to Connector |
| `agent_is_p2p_connected(host, port)` | Check if P2P connection is established |
| `agent_poll_p2p()` | Get next P2P packet to send |
| `agent_send_datagram_p2p(data, dest)` | Send datagram via P2P connection |

**Packet Routing Logic:**

```rust
fn recv(&mut self, data: &[u8], from: SocketAddr) -> Result<(), quiche::Error> {
    if Some(from) == self.intermediate_addr {
        // Packet from Intermediate Server → route to intermediate_conn
        self.intermediate_conn.as_mut()?.recv(...)?;
    } else if let Some(p2p) = self.p2p_conns.get_mut(&from) {
        // Packet from P2P Connector → route to p2p_conns[from]
        p2p.conn.recv(...)?;
    } else {
        return Err(quiche::Error::InvalidState);
    }
    Ok(())
}
```

### Connector Dual-Mode Architecture

The Connector (`app-connector/src/main.rs`) now operates in dual mode:

```rust
struct Connector {
    intermediate_conn: Option<quiche::Connection>,           // Client to Intermediate
    p2p_clients: HashMap<quiche::ConnectionId<'static>, P2PClient>, // Server for Agents
    client_config: quiche::Config,   // Client mode (no TLS server certs)
    server_config: quiche::Config,   // Server mode (TLS server certs loaded)
    // ...
}

struct P2PClient {
    conn: quiche::Connection,
    addr: SocketAddr,
}
```

**Dual-Mode Operation:**

1. **Client Mode:** Connects to Intermediate Server using `client_config`
2. **Server Mode:** Accepts incoming connections from Agents using `server_config`
3. **Same Socket:** Both modes use `quic_socket` (critical for hole punching)

**Packet Routing Logic:**

```rust
fn process_quic_socket(&mut self) {
    let hdr = quiche::Header::from_slice(...)?;

    if from == self.server_addr {
        // From Intermediate → use intermediate_conn
        self.intermediate_conn.as_mut()?.recv(...)?;
    } else if self.p2p_clients.contains_key(&hdr.dcid) {
        // Known P2P Agent → use p2p_clients[dcid]
        self.p2p_clients.get_mut(&hdr.dcid)?.conn.recv(...)?;
    } else if hdr.ty == quiche::Type::Initial {
        // New P2P connection → quiche::accept()
        self.handle_p2p_connection(&hdr, from, pkt_buf)?;
    }
}
```

**TLS Certificates:**

- Location: `app-connector/certs/connector-cert.pem`, `app-connector/certs/connector-key.pem`
- Self-signed, valid for 1 year
- Common Name: `ztna-connector`
- CLI args: `--p2p-cert <path>` and `--p2p-key <path>`

### QAD Extensions

Added `build_observed_address()` to `app-connector/src/qad.rs`:

```rust
/// Build an OBSERVED_ADDRESS QAD message
/// Format: [0x01, IPv4(4 bytes), port(2 bytes BE)]
pub fn build_observed_address(addr: SocketAddr) -> Vec<u8>
```

This allows Connector to send observed address back to Agent for P2P QAD support.

### Test Coverage

| Component | Tests | Status |
|-----------|-------|--------|
| Agent multi-connection | 5 unit tests | ✅ Pass |
| Connector P2P mode | 8 unit + 2 integration | ✅ Pass |
| QAD build_observed_address | 3 unit tests | ✅ Pass |

---

## Phase 1: Candidate Gathering

### 1.1 Candidate Types
- [ ] Host candidates (local IPs from all interfaces)
- [ ] Reflexive candidates (from QAD response)
- [ ] Relay candidates (Intermediate server address as fallback)

### 1.2 Candidate Format
```rust
struct Candidate {
    candidate_type: CandidateType,
    address: SocketAddr,
    priority: u32,
    foundation: String,
}

enum CandidateType {
    Host,           // Local IP (highest priority)
    ServerReflexive, // Public IP from QAD (medium)
    Relay,          // Via Intermediate (lowest)
}
```

### 1.3 Priority Calculation (RFC 8445)
```rust
fn calculate_priority(type_pref: u32, local_pref: u32, component: u32) -> u32 {
    (type_pref << 24) + (local_pref << 8) + (256 - component)
}

const HOST_TYPE_PREF: u32 = 126;
const SRFLX_TYPE_PREF: u32 = 100;
const RELAY_TYPE_PREF: u32 = 0;
```

---

## Phase 2: Signaling Infrastructure

### 2.1 Signaling via Intermediate

The Intermediate Server relays candidate information between Agent and Connector.

```
Agent                 Intermediate               Connector
  │                        │                          │
  │─── CandidateOffer ────►│                          │
  │                        │──── Relay CandidateOffer ─►
  │                        │                          │
  │                        │◄─── CandidateAnswer ─────│
  │◄─── Relay CandidateAnswer ─│                      │
  │                        │                          │
  │◄─── StartPunching ─────│──── StartPunching ──────►│
```

### 2.2 Message Format
```rust
enum SignalingMessage {
    CandidateOffer {
        session_id: u64,
        candidates: Vec<Candidate>,
    },
    CandidateAnswer {
        session_id: u64,
        candidates: Vec<Candidate>,
    },
    StartPunching {
        target_time_ms: u64,  // Relative: start in N ms
        peer_candidates: Vec<Candidate>,
    },
}
```

### 2.3 Protocol Details
- **Transport:** QUIC stream (reliable, ordered) - NOT DATAGRAM
- **Stream ID:** Dedicated signaling stream (e.g., stream 2)
- **Framing:** Length-prefixed messages (4-byte length + payload)
- **Serialization:** bincode for compact encoding
- **Timeout:** 5 seconds for signaling exchange

---

## Phase 3: Direct Path Establishment

### 3.1 Connectivity Check Flow

Both Agent and Connector attempt to establish connectivity by sending UDP packets (via QUIC) to each other's candidates.

```rust
struct BindingRequest {
    transaction_id: [u8; 12],
    priority: u32,
}

struct BindingResponse {
    transaction_id: [u8; 12],
    success: bool,
    mapped_address: Option<SocketAddr>,  // Reflexive discovery
}
```

### 3.2 Hole Punching Coordination

```
Agent NAT                                          Connector NAT
    │                                                    │
    │─── UDP to Connector reflexive ──► X (blocked)      │
    │                                                    │
    │      X (blocked) ◄── UDP to Agent reflexive ───────│
    │                                                    │
    │   (NAT mappings now created on both sides)         │
    │                                                    │
    │─── UDP retransmit ──────────────────────────────►  │ SUCCESS!
    │◄────────────────────────────────── UDP response ───│ SUCCESS!
```

### 3.3 Timing
- **Coordinated start:** Intermediate sends "start in 100ms" to both
- **Initial interval:** 20ms between attempts
- **Retransmit timeout:** 100ms
- **Max retransmits:** 5 per candidate pair
- **Total timeout:** 5 seconds before declaring failure

---

## Phase 4: QUIC Connection and Path Selection

### 4.1 quiche API (Corrected)

The research.md had incorrect API signatures. Actual quiche 0.22+ API:

```rust
// Probe a new path (triggers PATH_CHALLENGE)
pub fn probe_path(
    &mut self,
    local_addr: SocketAddr,
    peer_addr: SocketAddr
) -> Result<u64>

// Check if path is validated
pub fn is_path_validated(
    &self,
    from: SocketAddr,
    to: SocketAddr
) -> Result<bool>

// Migrate to validated path
pub fn migrate(
    &mut self,
    local_addr: SocketAddr,
    peer_addr: SocketAddr
) -> Result<u64>

// Send on specific path
pub fn send_on_path(
    &mut self,
    out: &mut [u8],
    from: Option<SocketAddr>,
    to: Option<SocketAddr>
) -> Result<(usize, SendInfo)>

// Process path events
pub fn path_event_next(&mut self) -> Option<PathEvent>

enum PathEvent {
    New(SocketAddr, SocketAddr),
    Validated(SocketAddr, SocketAddr),
    FailedValidation(SocketAddr, SocketAddr),
    Closed(SocketAddr, SocketAddr),
    ReusedSourceConnectionId(u64, SocketAddr, SocketAddr),
    PeerMigrated(SocketAddr, SocketAddr),
}
```

**Important:** Connection migration can only be initiated by the client. Connector must be a QUIC server for direct connections.

### 4.2 Path Selection Logic
```rust
fn should_use_direct(direct_rtt: Option<Duration>, relay_rtt: Duration) -> bool {
    match direct_rtt {
        Some(direct) => direct < relay_rtt * 70 / 100,  // Direct is 30%+ faster
        None => false,  // No direct path available
    }
}
```

---

## Phase 5: Resilience

### 5.1 NAT Keepalive
```rust
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const MISSED_KEEPALIVES_THRESHOLD: u32 = 3;
```

### 5.2 Path Failure Detection
- 3 consecutive missed keepalives → path failed
- Trigger fallback to relay

### 5.3 Fallback Decision Criteria

Hole punching is considered "failed" when:
- All candidate pairs exhausted
- Total timeout (5 seconds) exceeded
- All direct paths failed validation

Action: Fall back to relay path (always available)

### 5.4 Symmetric NAT Handling
- Detect via QAD to multiple servers (different reflexive ports)
- If symmetric: skip reflexive candidates, use relay
- Port prediction: deferred (complex, unreliable)

---

## Local Testing Strategy

### What CAN Be Tested Locally (All on localhost)

| Test Case | Validates | How |
|-----------|-----------|-----|
| Host candidate discovery | Interface enumeration | Unit test |
| Candidate pair formation | Pairing algorithm | Unit test |
| Binding request/response | Protocol encoding | Unit test |
| Signaling protocol | Message exchange | Integration test via Intermediate |
| QUIC path probing | quiche API usage | Probe localhost:different ports |
| QUIC connection to peer | New connection works | Agent → Connector direct (localhost) |
| Fallback logic | Relay used when direct fails | Simulate failure |
| Keepalive mechanism | Send/receive, timeout | Unit test with mock |

### What CANNOT Be Tested Locally

| Test Case | Why | Resolution |
|-----------|-----|------------|
| Actual NAT hole punching | No NAT on localhost | Task 006 (Cloud) |
| Reflexive address discovery | QAD returns 127.0.0.1 | Mock or Cloud |
| NAT binding timeout | No real NAT | Simulate |
| Symmetric NAT detection | Requires real NAT | Cloud |
| Cross-network latency | All localhost is <1ms | Cloud |

### Simulated NAT Testing (Localhost)

Use different loopback addresses to simulate separate "hosts":
```bash
# Agent binds to 127.0.0.2:50000
# Connector binds to 127.0.0.3:50001
# Intermediate on 127.0.0.1:4433
# Test direct path between 127.0.0.2 ↔ 127.0.0.3
```

This validates protocol correctness without real NAT.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      High-Level Flow                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. BOOTSTRAP (Existing Relay)                                   │
│     Agent ◄──QUIC──► Intermediate ◄──QUIC──► Connector           │
│                                                                  │
│  2. CANDIDATE EXCHANGE (via Intermediate signaling)              │
│     Agent ──candidates──► Intermediate ──relay──► Connector      │
│     Agent ◄──candidates── Intermediate ◄──relay── Connector      │
│                                                                  │
│  3. HOLE PUNCHING (simultaneous UDP)                             │
│     Agent ──UDP──► NAT A ──────X NAT B ◄──UDP── Connector        │
│     Agent ◄──UDP── NAT A ◄────── NAT B ──UDP──► Connector        │
│                    (mappings created)                            │
│                                                                  │
│  4. DIRECT PATH (new QUIC connection)                            │
│     Agent ◄════════════ QUIC (direct) ════════════► Connector    │
│                                                                  │
│  5. FALLBACK (if direct fails)                                   │
│     Continue using relay path                                    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## MVP Scope (First Iteration)

**Include:**
- Host candidates only (defer reflexive for MVP)
- Single service ID
- UDP-only (existing limitation)
- Simple round-robin hole punching
- Binary success/failure (no RTT optimization)
- Basic relay fallback

**Defer:**
- Reflexive candidates (requires real NAT testing)
- Port prediction for symmetric NAT
- Multiple simultaneous paths (QUIC multipath)
- IPv6 support
- UPnP/NAT-PMP

---

## Success Criteria

1. [ ] Host candidates gathered from local interfaces
2. [ ] Candidate exchange via Intermediate works
3. [ ] Direct QUIC connection Agent → Connector (localhost test)
4. [ ] Fallback to relay when direct unavailable
5. [ ] No data loss during path selection
6. [ ] Latency improvement measurable (when on separate networks)

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Connector as QUIC server** | High | Add TLS cert generation, server listener |
| **Single socket constraint** | Medium | Implement socket reuse architecture first |
| **quiche API differences** | Medium | Validate against actual quiche docs |
| **Symmetric NAT** | Medium | Relay fallback always works |
| **Clock skew** | Low | Use relative timing ("start in N ms") |
| **Firewall interference** | Medium | Multiple retries, document limitation |

---

## File Structure

```
core/packet_processor/src/
├── p2p/
│   ├── mod.rs           # P2P module
│   ├── candidate.rs     # Candidate types and gathering
│   ├── signaling.rs     # Candidate exchange protocol
│   ├── connectivity.rs  # Binding request/response
│   ├── hole_punch.rs    # Hole punching coordination
│   └── path_select.rs   # Path selection logic

app-connector/src/
├── server.rs            # NEW: QUIC server for P2P connections
├── p2p.rs              # NEW: P2P connection handling

intermediate-server/src/
├── signaling.rs        # NEW: Signaling message relay
```

---

## References

- [RFC 8445 - ICE](https://tools.ietf.org/html/rfc8445) - Interactive Connectivity Establishment
- [RFC 5389 - STUN](https://tools.ietf.org/html/rfc5389) - Session Traversal Utilities for NAT
- [RFC 9000 - QUIC](https://www.rfc-editor.org/rfc/rfc9000#section-9) - Connection Migration
- [quiche Connection docs](https://docs.quic.tech/quiche/struct.Connection.html) - Actual API
- [Tailscale NAT Traversal](https://tailscale.com/blog/how-nat-traversal-works/) - Excellent overview
