# Component Status & Dependencies

**Last Updated:** 2026-01-25 (Task 006 Phase 1 complete - Pi k8s Deployment)

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
- **Registers for target service (0x10 protocol)** ← NEW
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

### 005: P2P Hole Punching ✅ COMPLETE

**Location:** `core/packet_processor/src/p2p/`, `intermediate-server/src/signaling.rs`, `app-connector/`

**Dependencies:** 002, 003, 004 (relay working first) ✅ All complete

**Branch:** `master` (merged from `feature/005-p2p-hole-punching`)

**PR:** https://github.com/hfyeomans/ztna-agent/pull/5

**Status:**

| Phase | Status | Commit | Tests |
|-------|--------|--------|-------|
| Phase 0: Socket Architecture | ✅ Done | `c7d2aa7` | Agent multi-conn, Connector dual-mode |
| Phase 1: Candidate Gathering | ✅ Done | `672129c` | 11 tests (candidate types, gathering) |
| Phase 2: Signaling Infrastructure | ✅ Done | `d415d90` | 19 tests (messages, framing, sessions) |
| Phase 3: Direct Path Establishment | ✅ Done | `b64190c` | 17 tests (binding, pairs, check list) |
| Phase 4: Hole Punch Coordination | ✅ Done | `7754d7b` | 17 tests (coordinator, path selection) |
| Phase 5: Resilience | ✅ Done | `604da7c` | 12 tests (keepalive, fallback) |
| Phase 6: Testing | ✅ Done | `5b1c996` | 6 E2E tests, 79 unit tests |
| Phase 7: Documentation | ✅ Done | `31bfd93` | architecture.md, Task 005a created |
| Phase 8: PR & Merge | ✅ Done | `4db3e9b` | PR #5 merged 2026-01-20 |

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

### 005a: Swift Agent Integration ✅ COMPLETE

**Location:** `ios-macos/ZtnaAgent/`, `ios-macos/Shared/`

**Dependencies:** 005 (P2P Hole Punching - FFI functions available)

**Branch:** `master` (PR #6 merged 2026-01-23)

**Purpose:**
- Update macOS ZtnaAgent app to use new QUIC Agent FFI
- Replace old `process_packet()` with Agent struct
- Enable real QUIC connections and packet tunneling
- Foundation for P2P hole punching testing

**Current State:**
| Component | Status | Notes |
|-----------|--------|-------|
| SwiftUI App | ✅ Works | Start/Stop + auto-start/stop for testing |
| VPNManager | ✅ Works | Retry logic for first-time config |
| PacketTunnelProvider | ✅ Rewritten | Full QUIC integration via FFI |
| Bridging Header | ✅ Basic done | P2P/resilience FFI deferred (post-MVP) |
| AgentWrapper.swift | ⏭️ Deferred | FFI used directly (acceptable) |

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Bridging Header | ✅ Complete | Basic FFI (11 functions), P2P deferred |
| Phase 2: Swift Wrapper | ⏭️ Deferred | Using FFI directly instead |
| Phase 3: PacketTunnelProvider | ✅ Complete | Full QUIC + UDP + timeout handling |
| Phase 4: Build Configuration | ✅ Complete | Rust lib + Xcode build working |
| Phase 5: Testing | ✅ Complete | QUIC connection + QAD verified |
| Phase 6: Documentation | ✅ Complete | Demo script + _context/ docs |
| Phase 7: PR & Merge | ✅ Complete | PR #6 merged 2026-01-23 |

**Key Files:**
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - C FFI declarations (basic set + `agent_register`)
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Full QUIC integration with service registration
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` - SwiftUI + VPNManager

**Service Registration:**
- Calls `agent_register(agent, "echo-service")` after connection established
- Enables relay routing through Intermediate Server

**Test Automation Features:**
- `--auto-start` - Automatically start VPN on app launch
- `--auto-stop N` - Stop VPN after N seconds
- `--exit-after-stop` - Quit app after VPN stops

**Demo Script:** `tests/e2e/scenarios/macos-agent-demo.sh`

**Outcome:** ✅ macOS Agent connects to Intermediate Server, tunnels packets via QUIC, QAD working. Ready for packet flow and cloud testing.

---

### 006: Cloud Deployment 🔄 IN PROGRESS

**Location:** `deploy/docker-nat-sim/`, `deploy/k8s/` + Cloud infrastructure

**Dependencies:** 004 (E2E Testing), 005 (P2P), 005a (Swift Integration) ✅ All complete

**Branch:** `feature/006-cloud-deployment`

**Purpose:**
- Deploy Intermediate Server and App Connector to cloud
- Enable NAT testing with real public IPs
- Validate P2P hole punching with real NATs
- Prepare infrastructure for production

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 0: Docker NAT Simulation | ✅ Done | Local NAT testing environment |
| Phase 1: Pi k8s Deployment | ✅ Done | Home cluster with Cilium L2 |
| Phase 2: Cloud VM Deployment | 🔲 Pending | AWS/DigitalOcean |
| Phase 3: TLS & Security | 🔲 Pending | Self-signed → Let's Encrypt |
| Phase 4: Real NAT Testing | 🔲 Pending | Home network → Cloud |

**Phase 0 Completed (Docker NAT Simulation):**

Docker-based NAT simulation for local P2P testing:
```
Agent (172.21.0.10) --NAT--> 172.20.0.2 --\
                                           +--> Intermediate (172.20.0.10)
Connector (172.22.0.10) --NAT--> 172.20.0.3 --/
```

**Phase 1 Completed (Pi k8s Deployment):**

Kubernetes deployment to home Pi cluster with Cilium L2:
```
macOS (10.0.150.x) --QUIC--> LoadBalancer (10.0.150.205:4433)
                                   │
                                   └─► Intermediate Server (k8s)
                                           │
                                           └─► App Connector → Echo Server
```

**k8s Components Verified Working:**
- ✅ intermediate-server: Running, accepts QUIC connections
- ✅ app-connector: Running, registers for 'echo-service', receives QAD (30s idle timeout = CrashLoopBackOff is expected)
- ✅ echo-server: Running, test service
- ✅ LoadBalancer: 10.0.150.205:4433/UDP via Cilium L2
- ✅ macOS → k8s connection: QUIC connection successful

**Key Files Created (Phase 0):**
- `deploy/docker-nat-sim/docker-compose.yml` - 3-network topology
- `deploy/docker-nat-sim/Dockerfile.*` - Component images (4)
- `deploy/docker-nat-sim/watch-logs.sh` - Multi-terminal log viewer
- `tests/e2e/scenarios/docker-nat-demo.sh` - One-command demo

**Key Files Created (Phase 1):**
- `deploy/k8s/base/` - Kustomize base manifests
- `deploy/k8s/overlays/pi-home/` - Pi cluster overlay with Cilium L2
- `deploy/k8s/build-push.sh` - Multi-arch image builder
- `deploy/k8s/k8s-deploy-skill.md` - Comprehensive deployment guide

**Test Results (Phase 0):**
- ✅ Agent observed through NAT as 172.20.0.2
- ✅ Connector observed through NAT as 172.20.0.3
- ✅ UDP relay through Intermediate working
- ✅ Echo response received through tunnel

**Test Results (Phase 1):**
- ✅ k8s pods running on Pi cluster (arm64)
- ✅ Cilium L2 LoadBalancer IP assigned and accessible
- ✅ macOS app-connector connects to k8s intermediate-server
- ✅ QUIC registration + QAD working across network
- ✅ externalTrafficPolicy: Cluster required for L2 (lesson learned)

**Deployment Targets:**
| Component | Target |
|-----------|--------|
| Intermediate Server | Cloud VM with public IP |
| App Connector | Cloud VM (same VM for MVP) |
| Test Service | Cloud VM (localhost) |

**Capabilities needed:**
- Cloud VM provisioning (**Vultr or DigitalOcean recommended**)
- TLS certificate management (self-signed or Let's Encrypt)
- Systemd service configuration
- Firewall rules (UDP 4433, 4434)
- Remote Agent testing (from home NAT)

**Key Decisions:**
| Decision | Options | Status |
|----------|---------|--------|
| AWS VPC | New vs Existing | ✅ NEW VPC "ztna-test" |
| P2P Port | Ephemeral vs Fixed | ✅ Fixed port 4434 |
| Cloud Provider | Vultr, DigitalOcean | ✅ Decided (either) |
| Deployment | Single VM vs Separate VMs | Single VM (MVP) |
| Certificates | Self-signed vs Let's Encrypt | Self-signed (MVP) |
| Home k8s | Pi cluster | ✅ 10.0.150.101-108 available |

**⚠️ Critical Testing Insight:**
> Cloud VMs have **direct public IPs** - they are NOT behind NAT.
> To test P2P hole punching, the **Agent must be behind real NAT** (home network).

**P2P Testing Plan (from Task 005):**

| Test | Description | Requires Home NAT? |
|------|-------------|-------------------|
| DATAGRAM relay | Agent → Intermediate → Connector | No |
| QAD public IP | Correct external IP returned | No |
| **NAT hole punching** | Agent behind NAT, direct path to cloud | **Yes** |
| **Reflexive address accuracy** | QAD from home NAT | **Yes** |
| **NAT type behavior** | Full Cone, Restricted, Symmetric | **Yes** |
| Cross-network latency | Compare direct vs relay RTT | **Yes** |
| Keepalive over WAN | 15s interval over internet | **Yes** |

**Test Environment Setup:**
1. Intermediate Server + App Connector on cloud VM (single VM)
2. Echo server as test backend (localhost)
3. macOS Agent on home/office NAT ← **Required for P2P testing**
4. Optional: Mobile hotspot for CGNAT testing

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
                    │  ✅ COMPLETE            │
                    │  ★ PRIMARY GOAL ★       │
                    └───────────┬─────────────┘
                                │
                                │ FFI functions available
                                ▼
                    ┌─────────────────────────┐
                    │  005a: Swift Agent      │
                    │  Integration            │
                    │  ✅ COMPLETE            │
                    │  (macOS Agent + QUIC)   │
                    └───────────┬─────────────┘
                                │
                                │ enables real E2E testing
                                ▼
                    ┌─────────────────────────┐
                    │  006: Cloud Deployment  │
                    │  🔄 IN PROGRESS         │
                    │  (NAT testing, prod)    │
                    └─────────────────────────┘
```

---

## Critical Path

**Shortest path to working relay (local):**
1. ✅ 001: Agent Client (done)
2. ✅ 002: Intermediate Server (done)
3. ✅ 003: App Connector (done)
4. ✅ 004: E2E Testing (done - 61+ tests)

**Path to P2P (primary goal):**
- ✅ All of above + 005: P2P Hole Punching (done - 79 tests)

**Path to real macOS Agent E2E testing:**
- ✅ All of above + 005a: Swift Agent Integration (done - macOS Agent + QUIC working)

**Path to production deployment:**
- 🔄 All of above + **006: Cloud Deployment** (IN PROGRESS - NAT testing, production readiness)

---

## Inter-Component Communication

| From | To | Protocol | Port |
|------|----|----------|------|
| Agent | Intermediate | QUIC/UDP | 4433 |
| Connector | Intermediate | QUIC/UDP | 4433 |
| Agent | Connector (P2P) | QUIC/UDP | dynamic |
| Connector | Local App | TCP/UDP | configurable |

---

## Service Registration Protocol

The Intermediate Server routes traffic between Agents and Connectors using a **service-based routing model**. Both Agents and Connectors must register with a service ID to enable relay routing.

### Registration Message Format

```
┌────────────────┬──────────────────┬─────────────────────┐
│ Type (1 byte)  │ Length (1 byte)  │ Service ID (N bytes)│
└────────────────┴──────────────────┴─────────────────────┘
```

**Type Byte Values:**
- `0x10` = Agent registration (targeting a service)
- `0x11` = Connector registration (providing a service)

**Example:**
```
Register as Agent for "echo-service":
  [0x10] [0x0c] [echo-service]  (0x0c = 12 = length of "echo-service")

Register as Connector for "echo-service":
  [0x11] [0x0c] [echo-service]
```

### Registration Flow

```
1. Agent connects to Intermediate (QUIC handshake)
2. Agent receives QAD observed address (0x01 message)
3. Agent sends registration DATAGRAM: [0x10, len, service_id]
   - "I want to reach service 'echo-service'"

4. Connector connects to Intermediate (QUIC handshake)
5. Connector receives QAD observed address
6. Connector sends registration DATAGRAM: [0x11, len, service_id]
   - "I provide service 'echo-service'"

7. Intermediate Server registry maps:
   - connectors: service_id → connector_conn_id
   - agent_targets: agent_conn_id → target_service_id

8. When Agent sends data DATAGRAM:
   - Intermediate looks up Agent's target service
   - Finds Connector for that service
   - Relays to Connector

9. When Connector sends response DATAGRAM:
   - Intermediate looks up Connector's service
   - Finds Agent(s) targeting that service
   - Relays back to Agent
```

### FFI Functions

**Rust (`core/packet_processor/src/lib.rs`):**
```rust
// Agent registration constant
const REG_TYPE_AGENT: u8 = 0x10;

// FFI function
pub unsafe extern "C" fn agent_register(
    agent: *mut Agent,
    service_id: *const libc::c_char,
) -> AgentResult;
```

**Swift (`PacketTunnelProvider.swift`):**
```swift
// Call after connection established
let result = targetServiceId.withCString { servicePtr in
    agent_register(agent, servicePtr)
}
```

### Important Notes

1. **Service ID must match**: Agent's target service ID must exactly match a registered Connector's service ID
2. **No ACK**: Registration is fire-and-forget; server doesn't acknowledge
3. **Re-register on reconnect**: Registration is connection-scoped; lost on disconnect
4. **MVP limitation**: Agent can only target one service per connection

---

## Shared Code

| Module | Used By | Location |
|--------|---------|----------|
| QAD message format | Agent, Intermediate, Connector | TBD (shared crate) |
| QUIC config | All Rust components | TBD (shared crate) |
