# Task State: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Status:** In Progress - Phase 0 Complete, Ready for Phase 1
**Branch:** `feature/005-p2p-hole-punching`
**Last Updated:** 2026-01-20

---

## Overview

Implement direct peer-to-peer connectivity using NAT hole punching. This is the **primary connectivity goal** of the architecture - relay through the Intermediate is only a fallback.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 1 (Candidate Gathering)

### Prerequisites ✅ COMPLETE
- [x] Task 002 complete (Intermediate Server with QAD)
- [x] Task 003 complete (App Connector with QAD)
- [x] Task 004 complete (E2E relay testing validated - 61+ tests)
- [x] Feature branch created

### Socket Architecture Audit ✅ COMPLETE

#### Agent (`core/packet_processor/src/lib.rs`)

**Architecture:** Sans-IO FFI Library (no socket ownership)

| Aspect | Current State | P2P Impact |
|--------|---------------|------------|
| Socket Ownership | Swift NetworkExtension owns socket | ✅ Good - single socket naturally |
| Data Flow | FFI via `agent_recv()` / `agent_poll()` | Needs multi-connection support |
| QUIC Mode | Client only (`quiche::connect()`) | ✅ No change needed (connects to Connector) |
| Connections | Single connection tracked | **Needs expansion** for P2P |
| Local Address | Optional, set after first recv | ✅ Works for P2P |

**Key Code References:**
- `lib.rs:74-91`: Agent struct with single `conn: Option<Connection>`
- `lib.rs:116`: `config.set_disable_active_migration(true)` - disabled
- `lib.rs:132-152`: `connect()` creates client connection

**P2P Changes Required:**
1. Track multiple QUIC connections (Intermediate + Connector direct)
2. Add method to establish P2P connection to Connector
3. Route incoming data to correct connection based on source address

#### Connector (`app-connector/src/main.rs`)

**Architecture:** Standalone mio event-loop binary

| Aspect | Current State | P2P Impact |
|--------|---------------|------------|
| QUIC Socket | `quic_socket` on `0.0.0.0:0` (ephemeral) | ✅ Reusable for P2P |
| Local Socket | `local_socket` for forwarding | No change |
| QUIC Mode | Client only (`quiche::connect()`) | **Must add server mode** |
| TLS Certs | None (client doesn't need) | **Must generate/load** |
| Connections | Single connection to Intermediate | **Must track multiple** |

**Key Code References:**
- `main.rs:104-135`: Connector struct with single `conn: Option<quiche::Connection>`
- `main.rs:168-179`: Two sockets created, both registered with mio
- `main.rs:260-284`: `connect()` uses `quiche::connect()` (client mode)
- `main.rs:162-163`: Client-only config (`verify_peer(false)`)

**P2P Changes Required:**
1. **Add QUIC server capability** - Accept incoming connections from Agent
2. **Load TLS certificate/key** - Required for QUIC server mode
3. **Track multiple connections** - HashMap like Intermediate Server
4. **Dual-mode operation** - Client to Intermediate + Server for Agent
5. **Same socket** - Server must listen on `quic_socket`

#### Intermediate Server (Reference)

**Architecture:** QUIC server with `quiche::accept()`

**Key Patterns to Copy:**
- `main.rs:99-106`: TLS cert loading via `config.load_cert_chain_from_pem_file()` / `config.load_priv_key_from_pem_file()`
- `main.rs:278-322`: `handle_new_connection()` uses `quiche::accept()`
- `main.rs:87`: Multiple connections via `HashMap<ConnectionId, Client>`

### Oracle Review (2026-01-20)

Key findings and recommendations applied to plan.md and todo.md:

1. **P2P vs Path Migration Clarification**
   - P2P = NEW QUIC connection directly to Connector
   - Path Migration = same connection, different network path
   - These are different concepts (plan was conflating them)

2. **Socket Architecture (New Phase 0)**
   - Single socket reuse required for hole punching
   - QAD reflexive address must match P2P socket
   - Added as critical foundation phase

3. **Connector as QUIC Server**
   - Connector must accept incoming connections (currently client-only)
   - Requires TLS certificate for server mode
   - Major architectural change identified

4. **quiche API Corrections**
   - `probe_path()`, `migrate()`, `is_path_validated()` take `SocketAddr` pairs
   - `path_event_next()` for handling PathEvent variants
   - Connection migration only from client side

5. **Local Testing Strategy**
   - Host candidates testable locally
   - Signaling protocol testable locally
   - Direct QUIC connection testable (localhost)
   - Actual NAT hole punching requires Task 006 (Cloud)

### Single-Socket Architecture Design ✅ COMPLETE

#### Design Principle

For NAT hole punching to work, the QAD-discovered reflexive address must match the source address used for P2P connections. This requires **single-socket reuse**.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SINGLE SOCKET ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  AGENT (Swift manages socket)                                               │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │  UDP Socket (port X) ← Swift NetworkExtension                       │    │
│  │       │                                                             │    │
│  │       ├───► QUIC Connection 1 (to Intermediate) ← signaling, relay │    │
│  │       │                                                             │    │
│  │       └───► QUIC Connection 2 (to Connector) ← P2P direct          │    │
│  │                                                                     │    │
│  │  Both use same local port X → same reflexive address               │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  CONNECTOR (quic_socket)                                                    │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │  UDP Socket (port Y) ← quic_socket (0.0.0.0:0 → ephemeral)         │    │
│  │       │                                                             │    │
│  │       ├───► QUIC CLIENT (to Intermediate) ← signaling, relay       │    │
│  │       │                                                             │    │
│  │       └───► QUIC SERVER (accepts Agent) ← P2P direct               │    │
│  │                                                                     │    │
│  │  Dual-mode: Client + Server on same socket                         │    │
│  │  Requires: TLS certificate for server mode                         │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Agent Design (Sans-IO Model)

```rust
// BEFORE (single connection)
struct Agent {
    conn: Option<Connection>,  // Single connection to Intermediate
    // ...
}

// AFTER (multi-connection support)
struct Agent {
    intermediate_conn: Option<Connection>,     // Always connect to Intermediate
    p2p_conns: HashMap<SocketAddr, Connection>, // P2P connections to Connectors
    // ...
}
```

**Key Changes:**
1. Rename `conn` to `intermediate_conn` for clarity
2. Add `p2p_conns` HashMap to track direct Connector connections
3. Update `recv()` to route packets to correct connection by source address
4. Add `connect_p2p(connector_addr)` method
5. Add FFI function `agent_connect_p2p(host, port)`

#### Connector Design (Dual-Mode QUIC)

```rust
// BEFORE (client only)
struct Connector {
    conn: Option<quiche::Connection>,  // Client to Intermediate
    config: quiche::Config,            // Client config (no TLS certs)
    // ...
}

// AFTER (client + server)
struct Connector {
    intermediate_conn: Option<quiche::Connection>,  // Client to Intermediate
    p2p_clients: HashMap<quiche::ConnectionId<'static>, P2PClient>, // Agent connections
    client_config: quiche::Config,   // Config for client mode (no TLS server certs)
    server_config: quiche::Config,   // Config for server mode (TLS server certs)
    // ...
}

struct P2PClient {
    conn: quiche::Connection,
    addr: SocketAddr,
}
```

**Key Changes:**
1. Add `server_config` with TLS certificate loaded
2. Add `p2p_clients` HashMap for incoming Agent connections
3. Update `process_quic_socket()` to detect Initial packets and call `quiche::accept()`
4. Add `handle_p2p_connection()` method (pattern from Intermediate Server)
5. CLI args: `--p2p-cert` and `--p2p-key` for TLS certificate

#### Packet Routing Logic

**Agent:**
```rust
fn recv(&mut self, data: &[u8], from: SocketAddr) {
    // Route to correct connection based on source address
    if Some(self.server_addr) == Some(from) {
        // From Intermediate Server
        self.intermediate_conn.as_mut()?.recv(...);
    } else if let Some(p2p_conn) = self.p2p_conns.get_mut(&from) {
        // From P2P Connector
        p2p_conn.recv(...);
    }
}
```

**Connector:**
```rust
fn process_quic_socket(&mut self) {
    // For each received packet:
    let hdr = quiche::Header::from_slice(...);

    if from == self.server_addr {
        // From Intermediate Server - use client connection
        self.intermediate_conn.as_mut()?.recv(...);
    } else if self.p2p_clients.contains_key(&hdr.dcid) {
        // Known P2P client
        self.p2p_clients.get_mut(&hdr.dcid)?.conn.recv(...);
    } else if hdr.ty == quiche::Type::Initial {
        // New P2P connection from Agent
        self.handle_p2p_connection(&hdr, from, pkt_buf)?;
    }
}
```

---

## What's Done

### Phase 0: Socket Architecture ✅ COMPLETE
- [x] Research documented (research.md)
- [x] Initial plan created (plan.md)
- [x] Initial todo created (todo.md)
- [x] Oracle review completed
- [x] Plan updated with Oracle recommendations
- [x] Todo reordered with new Phase 0
- [x] Feature branch created
- [x] Socket architecture audit completed
- [x] Single-socket architecture designed
- [x] TLS certificates generated for Connector P2P (`app-connector/certs/connector-cert.pem`, `app-connector/certs/connector-key.pem`)
- [x] QUIC server mode implemented for Connector (dual-mode: client + server)
- [x] `build_observed_address()` added to `qad.rs` for P2P QAD support
- [x] Integration test: Connector P2P mode starts successfully
- [x] Multi-connection support added to Agent (`core/packet_processor/src/lib.rs`)
  - Refactored Agent struct: `intermediate_conn` + `p2p_conns` HashMap
  - New FFI functions: `agent_connect_p2p()`, `agent_is_p2p_connected()`, `agent_poll_p2p()`, `agent_send_datagram_p2p()`
  - Address-based packet routing in `recv()` method
  - 5 unit tests passing
- [x] Socket architecture documented in plan.md

---

## What's Next

1. **Phase 1: Candidate Gathering (Starting)**
   - [ ] Create `p2p/` module in `core/packet_processor/src/`
   - [ ] Implement `Candidate` struct with type, address, priority, foundation
   - [ ] Implement `CandidateType` enum (Host, ServerReflexive, Relay)
   - [ ] Implement `calculate_priority()` per RFC 8445
   - [ ] Implement `gather_host_candidates()` - enumerate interfaces
   - [ ] Implement `gather_reflexive_candidate()` from QAD
   - [ ] Implement `gather_relay_candidate()` (Intermediate address)
   - [ ] Unit tests for candidate module

2. **Phase 2: Signaling Infrastructure**
   - Define signaling message format
   - Implement Intermediate Server relay logic
   - Implement Agent/Connector signaling client

---

## Phase Summary

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 0: Socket Architecture | ✅ Complete | Agent multi-conn + Connector dual-mode |
| Phase 1: Candidate Gathering | 🔲 Not Started | Create `p2p/` module |
| Phase 2: Signaling Infrastructure | 🔲 Not Started | |
| Phase 3: Direct Path Establishment | 🔲 Not Started | |
| Phase 4: QUIC Connection & Path Selection | 🔲 Not Started | |
| Phase 5: Resilience | 🔲 Not Started | |
| Phase 6: Testing | 🔲 Not Started | |
| Phase 7: Documentation | 🔲 Not Started | |
| Phase 8: PR & Merge | 🔲 Not Started | |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 002 (Intermediate) | ✅ Complete | QAD provides reflexive addresses |
| Task 003 (App Connector) | ✅ Complete | Needs QUIC server mode added |
| Task 004 (E2E Testing) | ✅ Complete | 61+ tests, relay verified |

---

## Local Testing Constraints

This PoC runs entirely on localhost. Testing limitations:

| Feature | Testable Locally? | Notes |
|---------|-------------------|-------|
| Host candidates | ✅ Yes | Enumerate interfaces |
| Signaling protocol | ✅ Yes | Via Intermediate |
| Direct QUIC connection | ✅ Yes | Agent → Connector localhost |
| Fallback logic | ✅ Yes | Simulate failure |
| **NAT hole punching** | ❌ No | Requires real NAT (Task 006) |
| **Reflexive addresses** | ❌ No | QAD returns 127.0.0.1 locally |
| **NAT type detection** | ❌ No | Requires real NAT |

---

## Key Risks

| Risk | Impact | Status | Mitigation |
|------|--------|--------|------------|
| Connector as QUIC server | High | ✅ Closed | Implemented dual-mode QUIC (client+server) |
| Single socket constraint | Medium | ✅ Closed | Both Agent and Connector reuse single socket |
| quiche API differences | Medium | 🔲 Open | Validate during Phase 4 |
| Symmetric NAT | Medium | 🔲 Open | Relay fallback always works |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read `tasks/_context/components.md` for component status
3. Read this file for task state
4. Read `plan.md` for implementation details
5. Check `todo.md` for current progress
6. Ensure on branch: `feature/005-p2p-hole-punching`
7. Start with Phase 0: Socket Architecture
