# Component Status & Dependencies

**Last Updated:** 2026-01-31 (Task 006 Tasks 1-5 complete: config, routing, TCP, ICMP)

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
- **Multi-protocol packet handling:** UDP, TCP, and ICMP
- Decapsulates IPv4 packets from DATAGRAMs (UDP, TCP, ICMP)
- **UDP forwarding:** Extracts UDP payload, forwards to configurable local service, constructs return IP/UDP packets
- **TCP proxy:** Userspace TCP session tracking with non-blocking TcpStream (SYN→connect, ACK→forward, FIN→close, RST→reset)
- **ICMP Echo Reply:** Responds directly to ping requests (no backend forwarding needed)
- **JSON config file support:** `--config` flag or default paths (`/etc/ztna/connector.json`, `connector.json`)
- **0x2F Service-Routed Datagram support:** Receives `[0x2F, id_len, service_id..., ip_packet...]` from Intermediate
- QUIC keepalive (10s interval prevents 30s idle timeout)
- Integration test (handshake + QAD + registration verified)

**Critical Compatibility:**
- ALPN: `b"ztna-v1"` (matches Agent/Intermediate)
- MAX_DATAGRAM_SIZE: 1350
- Registration: `[0x11][len][service_id]`
- QAD: 7-byte IPv4 format (0x01 + IP + port)
- 0x2F: Service-routed datagram (Intermediate strips wrapper before forwarding)

**Key Design Decisions:**
- **mio over tokio**: Matches Intermediate Server's sans-IO model
- **Userspace TCP proxy**: Session-based tracking avoids TUN/TAP requirement
- **Connector-local ICMP**: Echo Reply generated at Connector, not forwarded to backend
- **No registration ACK**: Server doesn't acknowledge; treat as best-effort
- **JSON config**: Supports CLI arg override for backwards compatibility

**Deferred to Post-MVP:**
- Automatic reconnection
- Per-service backend routing (currently single --forward address for all services)
- TCP window flow control (currently simple ACK-per-segment)

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

**E2E Test Total: 61+** (Phases 1-6 complete)

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

**Test Count:** 81 tests in packet_processor (Phase 0-5 complete, includes agent_register)

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
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - C FFI declarations (basic set + `agent_register` + `agent_send_intermediate_keepalive`)
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Full QUIC integration with service registration and keepalive
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` - SwiftUI + VPNManager

**Service Registration:**
- Calls `agent_register(agent, "echo-service")` after connection established
- Enables relay routing through Intermediate Server

**Keepalive (Added 2026-01-25):**
- 10-second keepalive timer prevents 30s QUIC idle timeout
- Calls `agent_send_intermediate_keepalive()` which sends QUIC PING frame
- Timer starts after successful registration, stops on disconnect

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
| Phase 1/5: Pi k8s Deployment | ✅ Done | Home cluster with Cilium L2, full E2E relay working |
| Phase 5a: E2E Relay Routing | ✅ Done | macOS → k8s Intermediate → Connector → Echo |
| Phase 4: AWS EC2 Deployment | ✅ Done | EC2 t3.micro, Elastic IP 3.128.36.92, systemd services |
| Phase 4.1: AWS E2E Validation | ✅ Done | macOS behind NAT → AWS relay → echo-service |
| Phase 4.2: Config File Mechanism | ✅ Done | JSON configs for all components |
| Phase 4.3: IP→Service Routing | ✅ Done | 0x2F service-routed datagrams, multi-service registration |
| Phase 4.4: TCP Support | ✅ Done | Userspace TCP proxy with session tracking |
| Phase 4.5: ICMP Support | ✅ Done | Connector-local Echo Reply |
| Phase 3: TLS & Security | 🔲 Pending | Self-signed → Let's Encrypt |
| **Phase 6: P2P NAT Testing** | 🔲 **NOT DONE** | Requires Agent behind real NAT, Connector on different network |

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

**Phase 4 Completed (AWS EC2 Deployment):**

AWS EC2 deployment for cloud testing:
```
macOS Agent (anywhere) --QUIC--> Elastic IP (3.128.36.92:4433)
                                        │
                                        └─► EC2 Instance (t3.micro, us-east-2)
                                                │
                                                ├─► Intermediate Server (systemd)
                                                ├─► App Connector → :8080 (localhost)
                                                └─► Echo Server (Python)
```

**AWS Components:**
- ✅ EC2: i-021d9b1765cb49ca7 (ztna-intermediate-server)
- ✅ Elastic IP: 3.128.36.92
- ✅ Security Group: sg-0d15ab7f7b196d540 (UDP 4433, 4434, TCP 22)
- ✅ SSH via Tailscale: 10.0.2.126 (VPC private IP)

**Key Files Created (Phase 4):**
- `deploy/aws/aws-deploy-skill.md` - Comprehensive AWS deployment guide

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
| AWS VPC | New vs Existing | ✅ Using existing masque_proxy-vpc |
| P2P Port | Ephemeral vs Fixed | ✅ Fixed port 4434 |
| Cloud Provider | AWS, Vultr, DigitalOcean | ✅ AWS (EC2 deployed) |
| Deployment | Single VM vs Separate VMs | ✅ Single EC2 (MVP) |
| Certificates | Self-signed vs Let's Encrypt | ✅ Self-signed (from repo) |
| Home k8s | Pi cluster | ✅ 10.0.150.101-108 available |
| SSH Access | Public IP vs Tailscale | ✅ Tailscale (more reliable) |

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
4. ✅ 004: E2E Testing (done - 61+ E2E tests)

**Path to P2P (primary goal):**
- ✅ All of above + 005: P2P Hole Punching (done - 81 unit tests)

**Path to real macOS Agent E2E testing:**
- ✅ All of above + 005a: Swift Agent Integration (done - macOS Agent + QUIC working)

**Path to production deployment:**
- 🔄 All of above + **006: Cloud Deployment** (IN PROGRESS)
  - ✅ Config files, multi-service routing, TCP/ICMP support
  - 🔲 Return-path DATAGRAM→TUN injection (Agent side)
  - 🔲 P2P NAT testing, TLS, production readiness

---

## Inter-Component Communication

| From | To | Protocol | Port |
|------|----|----------|------|
| Agent | Intermediate | QUIC/UDP | 4433 |
| Connector | Intermediate | QUIC/UDP | 4433 |
| Agent | Connector (P2P) | QUIC/UDP | dynamic |
| Connector | Local App | TCP/UDP | configurable |

---

## Service Registration & Routing Protocol

The system uses a **configuration-driven, split-tunnel architecture** where only traffic to configured virtual IPs flows through the QUIC tunnel. All other traffic flows normally through the default gateway.

### Split-Tunnel Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       SPLIT-TUNNEL ROUTING MODEL                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  macOS Agent (NetworkExtension TUN)                                         │
│  ───────────────────────────────────                                        │
│  Routes: 10.100.0.0/24 → utun (ZTNA tunnel)                               │
│          0.0.0.0/0     → default gateway (untouched)                       │
│                                                                              │
│  Traffic to 10.100.0.1 (echo-service) → Captured → QUIC Tunnel             │
│  Traffic to 10.100.0.2 (web-app)      → Captured → QUIC Tunnel             │
│  Traffic to 8.8.8.8 (DNS)             → Normal routing (NOT tunneled)      │
│  Traffic to 93.184.216.34 (web)       → Normal routing (NOT tunneled)      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Configuration → Registration → Routing Flow

**Step 1: Configuration defines what gets tunneled**

Each component loads a JSON config that defines the services it handles:

```
Agent Config (agent.json):              Connector Config (connector.json):
┌──────────────────────────────┐       ┌──────────────────────────────┐
│ services:                     │       │ services:                     │
│   - id: "echo-service"       │       │   - id: "echo-service"       │
│     virtualIp: "10.100.0.1"  │       │     backend: "127.0.0.1:9999"│
│   - id: "web-app"            │       │     protocol: "udp"          │
│     virtualIp: "10.100.0.2"  │       │   - id: "web-app"            │
└──────────────────────────────┘       │     backend: "127.0.0.1:8080"│
                                        │     protocol: "tcp"          │
                                        └──────────────────────────────┘
```

**Step 2: Registration tells the Intermediate who provides/consumes what**

```
Agent connects → registers 0x10 for "echo-service" AND "web-app"
Connector connects → registers 0x11 for "echo-service"

Intermediate registry:
  agent_targets: { agent_conn → {"echo-service", "web-app"} }
  connectors:    { "echo-service" → connector_conn }
```

**Step 3: 0x2F Service-Routed Datagrams carry per-packet routing**

When the Agent intercepts a packet to 10.100.0.1, it looks up the route table (virtualIp → serviceId) and wraps the packet with a 0x2F header:

```
┌────────────┬──────────────────┬─────────────────────┬─────────────────┐
│ 0x2F       │ ID Length (1B)   │ Service ID (N bytes)│ IP Packet       │
│ (1 byte)   │                  │                     │ (remaining)     │
└────────────┴──────────────────┴─────────────────────┴─────────────────┘

Example: ping 10.100.0.1
[0x2F] [0x0c] [echo-service] [45 00 00 54 ... ICMP Echo Request ...]
```

The Intermediate reads the 0x2F header, finds the Connector for "echo-service", strips the wrapper, and forwards the raw IP packet to the Connector.

### Registration Message Format

```
┌────────────────┬──────────────────┬─────────────────────┐
│ Type (1 byte)  │ Length (1 byte)  │ Service ID (N bytes)│
└────────────────┴──────────────────┴─────────────────────┘
```

**Type Byte Values:**
- `0x10` = Agent registration (targeting a service)
- `0x11` = Connector registration (providing a service)
- `0x2F` = Service-routed datagram (per-packet routing)

**Example:**
```
Register as Agent for "echo-service":
  [0x10] [0x0c] [echo-service]  (0x0c = 12 = length of "echo-service")

Register as Connector for "echo-service":
  [0x11] [0x0c] [echo-service]

Send routed datagram to "echo-service":
  [0x2F] [0x0c] [echo-service] [ip_packet_bytes...]
```

### Protocol Support at Connector

The App Connector handles three IP protocols from tunneled packets:

| Protocol | IP Proto | Handling | Backend Required |
|----------|----------|----------|-----------------|
| **UDP** | 17 | Extract payload → forward to backend → encapsulate response | Yes |
| **TCP** | 6 | Userspace proxy: SYN→connect, data→stream, FIN→close | Yes |
| **ICMP** | 1 | Echo Reply generated at Connector (swap src/dst, type 0) | No |

### FFI Functions

**Rust (`core/packet_processor/src/lib.rs`):**
```rust
const REG_TYPE_AGENT: u8 = 0x10;

pub unsafe extern "C" fn agent_register(agent: *mut Agent, service_id: *const c_char) -> AgentResult;
pub unsafe extern "C" fn agent_send_datagram(agent: *mut Agent, buf: *const u8, len: usize) -> AgentResult;
```

**Swift (`PacketTunnelProvider.swift`):**
```swift
// Register for all configured services after connection established
for serviceId in serviceIds {
    serviceId.withCString { servicePtr in agent_register(agent, servicePtr) }
}

// Route table lookup + 0x2F wrapper for outgoing packets
if let serviceId = routeTable[destIp] {
    sendRoutedDatagram(agent: agent, serviceId: serviceId, packet: data)
}
```

### Important Notes

1. **Service ID must match**: Agent's target service ID must exactly match a registered Connector's service ID
2. **No ACK**: Registration is fire-and-forget; server doesn't acknowledge
3. **Re-register on reconnect**: Registration is connection-scoped; lost on disconnect
4. **Multi-service**: Agent can register for multiple services per connection (0x2F routing)
5. **Backward compatible**: Non-0x2F datagrams still use implicit single-service routing

---

## Shared Code

| Module | Used By | Location |
|--------|---------|----------|
| QAD message format | Agent, Intermediate, Connector | TBD (shared crate) |
| QUIC config | All Rust components | TBD (shared crate) |
