# Component Status & Dependencies

**Last Updated:** 2026-01-20 (Task 005 Phase 0-5 Complete)

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

### 003: App Connector ✅ COMPLETE

**Location:** `app-connector/`

**Dependencies:** 002 (Intermediate Server)

| Milestone | Status | Commit |
|-----------|--------|--------|
| Phase 1: QUIC Client + UDP Forwarding | ✅ Done | `7ec1708` |

**Capabilities:**
- QUIC client via quiche (mio event loop, not tokio)
- Registers as Connector (0x11 protocol)
- Parses QAD OBSERVED_ADDRESS messages
- Decapsulates IPv4/UDP packets from DATAGRAMs
- Forwards UDP payload to configurable local service
- Constructs return IP/UDP packets with proper checksums
- Integration test (handshake + QAD + registration verified)

**Critical Compatibility:**
- ALPN: `b"ztna-v1"` (matches Agent/Intermediate)
- MAX_DATAGRAM_SIZE: 1350
- Registration: `[0x11][len][service_id]`
- QAD: 7-byte IPv4 format (0x01 + IP + port)

**Key Design Decisions:**
- **mio over tokio**: Matches Intermediate Server's sans-IO model
- **UDP-only for MVP**: TCP requires TUN/TAP or TCP state tracking (deferred)
- **No registration ACK**: Server doesn't acknowledge; treat as best-effort

**Deferred to Post-MVP:**
- TCP support (requires TUN/TAP)
- ICMP support
- Automatic reconnection
- Config file (TOML)

---

### 004: E2E Relay Testing ✅ COMPLETE

**Location:** `tests/e2e/`

**Dependencies:** 002, 003

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Infrastructure | ✅ Done | 14 tests passing (component startup, direct echo) |
| Phase 1.5: QUIC Test Client | ✅ Done | IP/UDP packet construction, E2E relay VERIFIED |
| Phase 2: Protocol Validation | ✅ Done | 8 tests: ALPN, registration, DATAGRAM size, payloads |
| Phase 3: Relay Validation | ✅ Done | Full relay path verified |
| Phase 3.5: Coverage Gaps | ✅ Done | 6 tests: connector reg, service ID edge cases, malformed headers |
| Phase 4: Advanced UDP | ✅ Done | 11 tests: payload patterns, concurrent flows, burst, idle timeout |
| Phase 5: Reliability | ✅ Done | 11 tests: component restart, error conditions, rapid reconnect |
| Phase 6: Performance | ✅ Done | Latency (53µs baseline, 312µs tunneled), throughput (295K PPS), handshake (802µs) |

**Capabilities Built:**
- Test framework (`lib/common.sh`) with component lifecycle
- UDP echo server fixture (`fixtures/echo-server/`)
- **QUIC test client** (`fixtures/quic-client/`) for sending DATAGRAMs
  - Agent registration (`--service <id>`)
  - IP/UDP packet construction (`--send-udp --dst ip:port`)
  - IPv4 header checksum calculation (RFC 1071)
  - **Phase 2:** Protocol validation (`--alpn`, `--payload-size`, `--expect-fail`)
  - **Phase 3.5:** Programmatic DATAGRAM sizing (`--query-max-size`, `max`, `max-1`, `max+1`)
  - **Phase 4:** Payload patterns (`--payload-pattern zeros|ones|sequential|random`)
  - **Phase 4:** Multi-packet (`--repeat`, `--delay`, `--burst`)
  - **Phase 4:** Echo verification (`--verify-echo`)
  - **Phase 6:** RTT measurement (`--measure-rtt`, `--rtt-count`)
  - **Phase 6:** Handshake timing (`--measure-handshake`)
- Test scenarios for connectivity, echo, boundary conditions
- Protocol validation test suite (`scenarios/protocol-validation.sh`) - 14 tests
- Advanced UDP test suite (`scenarios/udp-advanced.sh`) - 11 tests
- Reliability test suite (`scenarios/reliability-tests.sh`) - 11 tests
- Performance metrics suite (`scenarios/performance-metrics.sh`) - latency, throughput, timing
- Comprehensive testing guide (`tasks/_context/testing-guide.md`)
- Architecture documentation (`tests/e2e/README.md`)

**Key Protocol Discovery (Phase 2):**
- Effective QUIC DATAGRAM limit is **~1307 bytes**, not 1350
- QUIC overhead (headers, encryption, framing) reduces usable payload
- Test verified: 1306 bytes OK, 1308 bytes → BufferTooShort

**E2E Relay Verified (2026-01-19):**
```
QUIC Client → Intermediate → Connector → Echo Server → back
✅ Full round-trip: 42-byte IP/UDP packet, 14-byte payload echoed
```

**Bug Fixes Applied:**
- App Connector: Initial QUIC handshake not sent (added `send_pending()`)
- App Connector: Local socket not registered with mio poll (return traffic lost)

**Important Distinction:**
- Task 001 Agent = Production macOS NetworkExtension (intercepts system packets)
- QUIC Test Client = Test harness CLI (sends arbitrary DATAGRAMs from scripts)

**Total Tests: 61+** (Phases 1-6 complete)

**Capabilities Needed (Remaining):**
- NAT testing (Intermediate on cloud)
- Network impairment testing (requires root/pfctl)

---

### 005: P2P Hole Punching 🔄 IN PROGRESS

**Location:** `core/packet_processor/src/p2p/`, `intermediate-server/src/signaling.rs`, `app-connector/`

**Dependencies:** 002, 003, 004 (relay working first) ✅ All complete

**Branch:** `feature/005-p2p-hole-punching`

**Status:**

| Phase | Status | Commit | Tests |
|-------|--------|--------|-------|
| Phase 0: Socket Architecture | ✅ Done | `c7d2aa7` | Agent multi-conn, Connector dual-mode |
| Phase 1: Candidate Gathering | ✅ Done | `672129c` | 11 tests (candidate types, gathering) |
| Phase 2: Signaling Infrastructure | ✅ Done | `d415d90` | 19 tests (messages, framing, sessions) |
| Phase 3: Direct Path Establishment | ✅ Done | `b64190c` | 17 tests (binding, pairs, check list) |
| Phase 4: Hole Punch Coordination | ✅ Done | | 17 tests (coordinator, path selection) |
| Phase 5: Resilience | ✅ Done | `604da7c` | 12 tests (keepalive, fallback) |
| Phase 6: Testing | 🔄 In Progress | `5b1c996` | 6 E2E tests, unit verification |
| Phase 7: Documentation | 🔄 In Progress | | architecture.md updated |
| Phase 8: PR & Merge | 🔲 Planned | | |

**Modules Created:**
- `p2p/candidate.rs` - ICE candidate types, RFC 8445 priority
- `p2p/signaling.rs` - CandidateOffer/Answer/StartPunching messages
- `p2p/connectivity.rs` - BindingRequest/Response, CandidatePair, CheckList
- `p2p/hole_punch.rs` - HolePunchCoordinator, path selection
- `p2p/resilience.rs` - PathManager, keepalive, fallback logic
- `intermediate-server/signaling.rs` - Session management for relay

**Key Architecture Decisions:**
- P2P = NEW QUIC connection (not path migration)
- Connector dual-mode: client (to Intermediate) + server (for Agents)
- Single socket reuse for NAT mapping preservation
- RFC 8445 pair priority: `2^32*MIN(G,D) + 2*MAX(G,D) + (G>D?1:0)`
- Exponential backoff: 100ms → 1600ms (max 5 retransmits)
- Keepalive: 15s interval, 3 missed = failed, auto fallback to relay

**Test Count:** 79 tests in packet_processor (Phase 0-5 complete)

---

### 005a: Swift Agent Integration 🔲 NOT STARTED

**Location:** `ios-macos/ZtnaAgent/`, `ios-macos/Shared/`

**Dependencies:** 005 (P2P Hole Punching - FFI functions available)

**Branch:** `feature/005a-swift-agent-integration`

**Purpose:**
- Update macOS ZtnaAgent app to use new QUIC Agent FFI
- Replace old `process_packet()` with Agent struct
- Enable real QUIC connections and packet tunneling
- Foundation for P2P hole punching testing

**Current State:**
| Component | Status | Notes |
|-----------|--------|-------|
| SwiftUI App | ✅ Works | Start/Stop buttons functional |
| VPNManager | ✅ Works | Configures NETunnelProviderManager |
| PacketTunnelProvider | ⚠️ Outdated | Uses old `process_packet()` API |
| Bridging Header | ⚠️ Incomplete | Missing P2P/resilience FFI |
| AgentWrapper.swift | ❌ Missing | Need Swift FFI wrapper |

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Bridging Header | 🔲 Not Started | Add all FFI declarations |
| Phase 2: Swift Wrapper | 🔲 Not Started | Create AgentWrapper.swift |
| Phase 3: PacketTunnelProvider | 🔲 Not Started | Full rewrite with QUIC |
| Phase 4: Build Configuration | 🔲 Not Started | Xcode + Cargo |
| Phase 5: Testing | 🔲 Not Started | Local E2E |
| Phase 6: Documentation | 🔲 Not Started | |
| Phase 7: PR & Merge | 🔲 Not Started | |

**Key Files:**
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - C FFI declarations
- `ios-macos/Shared/AgentWrapper.swift` - Swift wrapper (to create)
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Tunnel logic

**Outcome:** macOS Agent connects to Intermediate Server, tunnels packets via QUIC, enabling E2E testing with real Agent app.

---

### 006: Cloud Deployment 🔲 NOT STARTED

**Location:** Cloud infrastructure + deployment scripts

**Dependencies:** 004 (E2E Testing - local validation first)

**Purpose:**
- Deploy Intermediate Server and App Connector to cloud
- Enable NAT testing with real public IPs
- Validate QAD with actual network conditions
- Prepare infrastructure for production

**Deployment Targets:**
| Component | Target |
|-----------|--------|
| Intermediate Server | Cloud VM with public IP |
| App Connector | Cloud VM (same or separate) |
| Test Service | Cloud VM (localhost) |

**Capabilities needed:**
- Cloud VM provisioning (DigitalOcean/AWS/Vultr/GCP)
- TLS certificate management (self-signed or Let's Encrypt)
- Systemd service configuration
- Firewall rules (UDP 4433)
- Remote Agent testing (NAT traversal)

**Key Decisions (TBD):**
| Decision | Options | Status |
|----------|---------|--------|
| Cloud Provider | DO, AWS, Vultr, GCP | TBD |
| Deployment | Single VM vs Separate VMs | TBD |
| Certificates | Self-signed vs Let's Encrypt | TBD |
| Automation | Manual, Terraform, Ansible | TBD |

**P2P Testing Plan (from Task 005):**

The following P2P tests require cloud deployment with real NAT:

| Test | Description | Validation |
|------|-------------|------------|
| NAT hole punching | Agent behind home NAT, Connector on cloud | Direct path established |
| Reflexive address accuracy | QAD returns real public IP | Compare with ifconfig.me |
| NAT type detection | Test against Full Cone, Symmetric NAT | Appropriate fallback behavior |
| Cross-network latency | Compare direct vs relay RTT | Direct < Relay |
| Mobile handoff | WiFi → Cellular → WiFi | Connection survives |
| Keepalive over WAN | 15s interval over internet | Path stays active |
| Fallback under load | Stress test during path failure | Graceful relay switch |

**Test Environment Setup:**
1. Intermediate Server on cloud VM with public IP
2. App Connector on same or separate cloud VM
3. Echo server as test backend
4. iOS/macOS Agent on home/office NAT
5. Network impairment simulation (tc/netem)

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
                    │  ✅ COMPLETE            │
                    └───────────┬─────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
    ┌─────────────────────────┐   ┌─────────────────────────┐
    │  003: App Connector     │   │  004: E2E Testing       │
    │  ✅ COMPLETE            │   │  ✅ COMPLETE            │
    └───────────┬─────────────┘   └───────────┬─────────────┘
                │                             │
                └─────────────┬───────────────┘
                              │
                              │ relay working locally ✅
                              ▼
                    ┌─────────────────────────┐
                    │  005: P2P Hole Punching │
                    │  🔄 IN PROGRESS         │
                    │  ★ PRIMARY GOAL ★       │
                    └───────────┬─────────────┘
                                │
                                │ FFI functions available
                                ▼
                    ┌─────────────────────────┐
                    │  005a: Swift Agent      │
                    │  Integration            │
                    │  🔲 NOT STARTED         │
                    │  (macOS Agent + QUIC)   │
                    └───────────┬─────────────┘
                                │
                                │ enables real E2E testing
                                ▼
                    ┌─────────────────────────┐
                    │  006: Cloud Deployment  │
                    │  🔲 NOT STARTED         │
                    │  (NAT testing, prod)    │
                    └─────────────────────────┘
```

---

## Critical Path

**Shortest path to working relay (local):**
1. ✅ 001: Agent Client (done)
2. ✅ 002: Intermediate Server (done)
3. ✅ 003: App Connector (done)
4. ✅ 004: E2E Testing (Phases 1-6 complete, ready for PR)

**Path to P2P (primary goal):**
- All of above + 005: P2P Hole Punching

**Path to real macOS Agent E2E testing:**
- All of above + 005a: Swift Agent Integration (wire up macOS Agent app with QUIC FFI)

**Path to production deployment:**
- All of above + 006: Cloud Deployment (NAT testing, production readiness)

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
