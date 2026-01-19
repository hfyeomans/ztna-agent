# Component Status & Dependencies

**Last Updated:** 2026-01-18

---

## Component Status

### 001: Agent QUIC Client ✅ COMPLETE

**Location:** `core/packet_processor/` + `ios-macos/ZtnaAgent/Extension/`

| Milestone | Status | Commit |
|-----------|--------|--------|
| Phase 1: Rust QUIC Client | ✅ Done | `958ce3f` |
| Phase 1.5: Code Quality | ✅ Done | `229448b` |
| Phase 2: Swift UDP Integration | ✅ Done | `286df2a` |

**Capabilities:**
- Creates QUIC connections via quiche
- Sends/receives QUIC DATAGRAMs
- Parses QAD OBSERVED_ADDRESS messages
- Tunnels intercepted IP packets
- Thread-safe state management

**Waiting on:** Intermediate Server (002) for testing

---

### 002: Intermediate Server ✅ COMPLETE

**Location:** `intermediate-server/`

**Capabilities:**
- QUIC server accepting connections (mio event loop)
- QAD: report observed address to clients (7-byte format)
- DATAGRAM relay between agent/connector pairs
- Client registry for routing (connection-based)
- Integration test (handshake + QAD verified)

**Critical Compatibility:**
- ALPN: `b"ztna-v1"` (matches Agent)
- QAD: DATAGRAM only, 7-byte IPv4 format

---

### 003: App Connector 🔲 NOT STARTED

**Location:** `app-connector/` (to be created)

**Dependencies:** 002 (Intermediate Server)

**Capabilities needed:**
- QUIC client connecting to Intermediate
- Receive DATAGRAMs, decapsulate IP packets
- Forward to local application (TCP/UDP)
- Handle return traffic

---

### 004: E2E Relay Testing 🔲 NOT STARTED

**Location:** Test scripts + documentation

**Dependencies:** 002, 003

**Capabilities needed:**
- Local test setup (all components on localhost)
- NAT testing (Intermediate on cloud)
- Latency measurement
- Failure scenario testing

---

### 005: P2P Hole Punching 🔲 NOT STARTED

**Location:** Updates to Agent + Connector

**Dependencies:** 002, 003, 004 (relay working first)

**Capabilities needed:**
- Address exchange via Intermediate
- Simultaneous open (hole punch)
- QUIC connection migration
- Path selection (prefer direct)
- Fallback to relay

---

## Dependency Graph

```
                    ┌─────────────────────────┐
                    │  001: Agent Client      │
                    │  ✅ COMPLETE            │
                    └───────────┬─────────────┘
                                │
                                │ requires server to test
                                ▼
                    ┌─────────────────────────┐
                    │  002: Intermediate      │
                    │  Server                 │
                    │  🔄 IN PROGRESS         │
                    └───────────┬─────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
    ┌─────────────────────────┐   ┌─────────────────────────┐
    │  003: App Connector     │   │  004: E2E Testing       │
    │  🔲 NOT STARTED         │   │  🔲 NOT STARTED         │
    └───────────┬─────────────┘   └───────────┬─────────────┘
                │                             │
                └─────────────┬───────────────┘
                              │
                              │ relay working
                              ▼
                    ┌─────────────────────────┐
                    │  005: P2P Hole Punching │
                    │  🔲 NOT STARTED         │
                    │  ★ PRIMARY GOAL ★       │
                    └─────────────────────────┘
```

---

## Critical Path

**Shortest path to working relay:**
1. ✅ 001: Agent Client (done)
2. 🔄 002: Intermediate Server (in progress)
3. 🔲 003: App Connector
4. 🔲 004: E2E Testing

**Path to P2P (primary goal):**
- All of above + 005: P2P Hole Punching

---

## Inter-Component Communication

| From | To | Protocol | Port |
|------|----|----------|------|
| Agent | Intermediate | QUIC/UDP | 4433 |
| Connector | Intermediate | QUIC/UDP | 4433 |
| Agent | Connector (P2P) | QUIC/UDP | dynamic |
| Connector | Local App | TCP/UDP | configurable |

---

## Shared Code

| Module | Used By | Location |
|--------|---------|----------|
| QAD message format | Agent, Intermediate, Connector | TBD (shared crate) |
| QUIC config | All Rust components | TBD (shared crate) |
