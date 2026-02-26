# Component Status & Dependencies

**Last Updated:** 2026-02-21 (Task 006 PR #7 merged to master. Swift 6 modernization + linting infra complete.)

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
- Registers for target service (0x10 protocol)
- Tunnels intercepted IP packets (outgoing via `agent_send_datagram()`)
- **Receives return packets via `agent_recv_datagram()` FFI** ← NEW (2026-01-31)
- **Queues received DATAGRAMs in `VecDeque<Vec<u8>>`** ← NEW
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

**P2P Server Mode (Port 4434):**
- Dual-mode: QUIC client (to Intermediate on 4433) + QUIC server (for Agents on 4434)
- Packet demux on port 4434: QUIC packets (first byte & 0xC0 != 0) vs Control packets
- Control packet types: Binding messages (bincode variant 0x00/0x01), Keepalive (0x10 request / 0x11 response)
- `process_p2p_control_packet()` handles binding requests and keepalive echo
- Keepalive protocol: raw UDP, 5 bytes: `[type_byte, sequence_u32_le]`

**Multi-Service Architecture (Phase 7):**
- Per-service routing requires separate Connector instances (each with own --forward-addr)
- AWS deployment: `ztna-connector.service` (echo-service, port 4434, P2P enabled) + `ztna-connector-web.service` (web-app, port 4435, relay-only)
- Agent registers for multiple services via providerConfiguration `services` array
- Intermediate routes 0x2F datagrams to matching Connector by service ID

**Shared Socket Architecture (Phase 8.5 Discovery):**
- Connector uses SINGLE `quic_socket` on port 4434 for BOTH P2P QUIC and Intermediate relay QUIC
- Blocking port 4434 globally (e.g., `iptables -A INPUT -p udp --dport 4434 -j DROP`) kills both P2P and relay paths
- Interface-specific blocking (`iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP`) correctly isolates P2P while preserving loopback relay
- Future improvement: use separate sockets for P2P and relay connections

**Deferred to Post-MVP:**
- Automatic reconnection (→ Task 008)
- Per-service backend routing, currently single --forward address for all services (→ Task 009)
- TCP window flow control, currently simple ACK-per-segment (→ Task 011)
- Separate P2P and relay sockets in Connector (→ Task 011)

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
| Bridging Header | ✅ Complete | 23 FFI functions: 11 relay/core + 12 P2P (connections, hole punch, path resilience) |
| AgentWrapper.swift | ⏭️ Deferred | FFI used directly (acceptable) |

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Bridging Header | ✅ Complete | 23 FFI functions (11 core + 12 P2P) |
| Phase 2: Swift Wrapper | ⏭️ Deferred | Using FFI directly instead |
| Phase 3: PacketTunnelProvider | ✅ Complete | Full QUIC + UDP + timeout handling |
| Phase 4: Build Configuration | ✅ Complete | Rust lib + Xcode build working |
| Phase 5: Testing | ✅ Complete | QUIC connection + QAD verified |
| Phase 6: Documentation | ✅ Complete | Demo script + _context/ docs |
| Phase 7: PR & Merge | ✅ Complete | PR #6 merged 2026-01-23 |

**Key Files:**
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - C FFI declarations (23 total: core lifecycle/connect/packet I/O/timeout + agent_register + keepalive + recv_datagram + 12 P2P: connections, hole punch, path resilience)
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Full QUIC integration with service registration, keepalive, and return-path TUN injection (Swift 6, strict concurrency)
- `ios-macos/ZtnaAgent/Extension/AgentFFI.swift` - Extracted FFI boundary types and helper functions
- `ios-macos/ZtnaAgent/ZtnaAgent/TunnelUtilities.swift` - IPv4 parsing, routed datagram builder
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` - SwiftUI + VPNManager + configuration UI

**Service Registration:**
- Calls `agent_register(agent, "echo-service")` after connection established
- Enables relay routing through Intermediate Server

**Keepalive (Added 2026-01-25):**
- 10-second keepalive timer prevents 30s QUIC idle timeout
- Calls `agent_send_intermediate_keepalive()` which sends QUIC PING frame
- Timer starts after successful registration, stops on disconnect

**Return-Path TUN Injection (Added 2026-01-31):**
- `drainIncomingDatagrams()` polls `agent_recv_datagram()` after each `agent_recv()`
- Validates IPv4 version nibble, batches packets
- Injects via `packetFlow.writePackets()` into TUN for kernel delivery
- Enables `ping 10.100.0.1` to receive Echo Replies

**Connection Resilience (Added 2026-01-31):**
- Auto-recovery when QUIC connection drops (server restart, network change, timeout)
- `NWPathMonitor` detects WiFi → Cellular transitions, triggers reconnect
- Exponential backoff: 1s → 2s → 4s → 8s → 16s → 30s (cap), reset on success
- Three detection paths: NWConnection `.failed`, `updateAgentState()` transitions, keepalive `NotConnected`
- `attemptReconnect()` reuses existing Agent — calls `agent_connect()` again (no destroy/recreate)
- State transition tracking prevents duplicate reconnect scheduling
- P2P state fully reset during reconnection (timers, connections, flags)

**P2P Swift Integration (Added 2026-01-31):**
- 12 P2P FFI functions wired into PacketTunnelProvider (hole punch, binding, P2P QUIC, routing, keepalive)
- Three NWConnection types: relay (udpConnection), binding (per-candidate), P2P (direct to Connector)
- Hole punch auto-starts after service registration
- Packet routing via `agent_get_active_path()`: Direct (P2P) or Relay
- P2P keepalive timer (15s interval, 5-byte messages via `agent_poll_keepalive`)
- Fallback detection via `agent_is_in_fallback()`
- Path stats logging via `agent_get_path_stats()`

**P2P NAT Testing Results (2026-01-31):**
- Direct P2P QUIC path achieved: macOS (home NAT) → AWS Connector (port 4434)
- Hole punch succeeds via server-reflexive candidate (3.128.36.92:4434)
- 0.0.0.0:4434 candidate fails as expected (non-routable)
- P2P keepalive stable for 3.5+ minutes (14 consecutive checks, zero missed)
- ~1.8s warm-up window from tunnel start to P2P data flow (relay handles traffic during)
- **Bug fix:** Rust `recv()` added keepalive demux — raw 5-byte keepalive (0x10/0x11) intercepted before `quiche::recv()` which would reject non-QUIC data

**Deferred QUIC Enhancements (Post-MVP):**
- True QUIC connection migration (quiche doesn't support — full reconnect instead)
- 0-RTT reconnection (requires session ticket storage in quiche)
- Multiplexed QUIC streams (DATAGRAMs sufficient for current relay needs)

**Test Automation Features:**
- `--auto-start` - Automatically start VPN on app launch
- `--auto-stop N` - Stop VPN after N seconds
- `--exit-after-stop` - Quit app after VPN stops

**Demo Script:** `tests/e2e/scenarios/macos-agent-demo.sh`

**Outcome:** ✅ macOS Agent connects to Intermediate Server, tunnels packets via QUIC, QAD working. Ready for packet flow and cloud testing.

---

### 006: Cloud Deployment ✅ COMPLETE (MVP)

**Location:** `deploy/docker-nat-sim/`, `deploy/k8s/` + Cloud infrastructure

**Dependencies:** 004 (E2E Testing), 005 (P2P), 005a (Swift Integration) ✅ All complete

**Branch:** `master` (PR #7 merged 2026-02-21)

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
| Phase 4.6: Return-Path (DATAGRAM→TUN) | ✅ Done | `agent_recv_datagram()` FFI + `drainIncomingDatagrams()` + `writePackets()` |
| Phase 4.7: Registry Connector Replacement Fix | ✅ Done | `unregister()` guard prevents clobbering new registrations |
| Phase 4.9: Connection Resilience | ✅ Done | Auto-recovery, NWPathMonitor, exponential backoff |
| Phase 3: TLS & Security | → Task 007 | Self-signed → Let's Encrypt |
| Phase 6: P2P Swift Integration | ✅ Done | 12 P2P FFI wired into PacketTunnelProvider (hole punch, binding, P2P QUIC, routing, keepalive) |
| **Phase 6.8: P2P NAT Testing** | ✅ **DONE** | Direct P2P path achieved: macOS (home NAT) → AWS Connector. Keepalive stable 3.5+ min. |
| **Phase 7: HTTP App Validation** | ✅ **DONE** | HTTP through tunnel via second connector (web-app, 10.100.0.2). Multi-service routing verified. |
| **Phase 8: Performance Metrics** | ✅ **DONE** | P2P 32.6ms vs Relay 76ms (2.3x faster). 10-min stability: 600/600, 0% loss. |
| **Phase 8.5: P2P→Relay Failover** | ✅ **DONE** | Interface-specific iptables test: 180/180, 0% loss, seamless per-packet failover. |

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

### 013: Swift Modernization ✅ COMPLETE

**Location:** `ios-macos/ZtnaAgent/`, `.github/workflows/`, `.pre-commit-config.yaml`, `.swiftlint.yml`

**Branch:** `master` (merged in PR #7)

**Completed:**
- Swift 6 language mode with `SWIFT_STRICT_CONCURRENCY = complete`
- macOS deployment target aligned to 26.2
- Extracted `AgentFFI.swift` (FFI boundary) and `TunnelUtilities.swift` (IP parsing utilities)
- Added `TunnelUtilitiesTests.swift` and `VPNManagerTests.swift` (Swift Testing framework)
- Removed deprecated `Persistence.swift`, duplicate `App/ContentView.swift` and `Extension/PacketTunnelProvider.swift`

**Linting Infrastructure:**
- GitHub Actions CI: 3 parallel jobs (Rust 5-crate matrix, SwiftLint, ShellCheck)
- Pre-commit hooks: 12 hooks (5 rustfmt, 5 clippy, 1 shellcheck, 1 swiftlint)
- All existing violations fixed across Rust (45+ clippy), Swift (10), Shell (5)
- `.swiftlint.yml` with pragmatic disabled rules for existing codebase

**Key Files:**
- `ios-macos/ZtnaAgent/Extension/AgentFFI.swift` — Extracted FFI boundary types and functions
- `ios-macos/ZtnaAgent/ZtnaAgent/TunnelUtilities.swift` — IPv4 parsing, routed datagram builder
- `.github/workflows/lint.yml` — CI lint workflow
- `.pre-commit-config.yaml` — Local pre-commit hooks
- `.swiftlint.yml` — SwiftLint configuration

---

### 007: Security Hardening ✅ COMPLETE

**Location:** `intermediate-server/`, `app-connector/`, `core/packet_processor/`, `scripts/`, `deploy/`

**Branch:** `feature/007-security-hardening` (PR #8)

**Scope:** 26 security findings (1 Critical, 4 High, 8 Medium, 9 Low, 4 Info) + 6 deferred items

**Phases 1-5 (Initial Hardening — 26 findings):**
- TLS `verify_peer(true)` on all Rust crates + CA cert loading (C1)
- K8s secrets validation script, no placeholder certs (H4)
- Connector registration replacement warning (H2), sender authorization (M3)
- TCP SYN rate limiting per source IP, destination IP validation (H3)
- TCP half-close draining (L6), queue depth cap (H1)
- ZTNA_MAGIC keepalive prefix (M2/M6), `ip_len` FFI validation (M5)
- 15 config/ops items: hardcoded IPs, Docker caps, logging levels, cert paths, etc.

**Phases 6-8 (6 Deferred Items — All Complete):**
- **Phase 6A: mTLS Client Authentication** — `auth.rs` module, x509-parser SAN extraction, `--require-client-cert` flag, cert generation script, peer cert extraction after handshake, service authorization
- **Phase 6B: Certificate Auto-Renewal** — SIGHUP hot-reload via signal-hook, certbot Route53 DNS-01, systemd timer, k8s cert-manager CRDs
- **Phase 7A: Non-Blocking TCP Proxy** — `mio::net::TcpStream::connect()` replaces blocking 500ms connect, event-driven I/O, 5s connect timeout, duplicate SYN guard
- **Phase 7B: Stateless Retry Tokens** — AEAD (AES-256-GCM) token encryption, `quiche::retry()`, Retry SCID reuse for transport parameter match, `--disable-retry` flag
- **Phase 8A: Registration ACK Protocol** — 0x12 ACK / 0x13 NACK, `RegistrationState` state machine with Denied terminal state, per-service retry (2s timeout, 3 max), multi-ACK Vec accumulation
- **Phase 8B: Connection ID Rotation** — 5-min timer, `cid_aliases` HashMap (max 4 per connection), cleanup on connection close, client-side rotation in Agent + Connector

**Key Files Created:**
- `intermediate-server/src/auth.rs` — mTLS auth module (ClientIdentity, SAN extraction, authorization)
- `scripts/generate-client-certs.sh` — CA + client cert generation for dev/test
- `scripts/resolve-pr-comments.sh` — PR comment resolution tool
- `deploy/aws/setup-certbot.sh` — Route53 DNS-01 cert issuance
- `deploy/aws/ztna-cert-renew.{service,timer}` — Systemd renewal
- `deploy/k8s/overlays/cert-manager/` — k8s cert-manager CRDs

**Test Count:** 143 tests passing (39+1 intermediate-server, 83 packet_processor, 18+2 app-connector)

**Review Rounds:** 3 Oracle reviews (12 findings fixed) + 1 CodeRabbit/Gemini review (16 threads, 4 actionable fixes)

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
                    │  ✅ COMPLETE (MVP)      │
                    └───────────┬─────────────┘
                                │
                     ┌──────────┼──────────────────┐
                     ▼          ▼                   ▼
               007 (Security) 009 (Multi-Svc)  011 (Protocol)
               P1             P2               P3
                     │          │
                     ▼          ▼
               008 (Prod Ops) 010 (Dashboard)  012 (Multi-Env)
               P2             P3               P3
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
- ✅ All of above + **006: Cloud Deployment** (COMPLETE — MVP)
  - ✅ Config files, multi-service routing, TCP/ICMP support
  - ✅ Return-path DATAGRAM→TUN injection (Agent side) - ICMP ping works E2E
  - ✅ Registry Connector replacement bug fix
  - ✅ P2P Swift Integration (12 FFI functions wired into PacketTunnelProvider)
  - ✅ P2P NAT Testing — direct path achieved, keepalive stable, Rust `recv()` keepalive demux fix
  - ✅ HTTP app validation — multi-service (echo + web-app) through tunnel
  - ✅ Performance metrics — P2P 32.6ms vs Relay 76ms, 10-min 0% loss
  - ✅ P2P→Relay failover — seamless per-packet fallback, 180/180 0% loss

**Path to production (post-MVP):**
- ✅ Task 007: Security Hardening (P1) — Complete (Phases 1-8, 26 findings + 6 deferred items)
- 🔲 Task 008: Production Operations (P2) — Monitoring, CI/CD, automation
- 🔲 Task 009: Multi-Service Architecture (P2) — Per-service backends, discovery
- 🔲 Task 010: Admin Dashboard (P3) — Web UI for management
- 🔲 Task 011: Protocol Improvements (P3) — IPv6, TCP flow, QUIC migration
- 🔲 Task 012: Multi-Environment Testing (P3) — DO, multi-region, NAT diversity

---

## Inter-Component Communication

| From | To | Protocol | Port |
|------|----|----------|------|
| Agent | Intermediate | QUIC/UDP | 4433 |
| Connector | Intermediate | QUIC/UDP | 4433 |
| Agent | Connector (P2P) | QUIC/UDP | 4434 (Connector P2P listen port) |
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
// Relay / Core FFI
pub unsafe extern "C" fn agent_register(agent: *mut Agent, service_id: *const c_char) -> AgentResult;
pub unsafe extern "C" fn agent_send_datagram(agent: *mut Agent, buf: *const u8, len: usize) -> AgentResult;
pub unsafe extern "C" fn agent_recv_datagram(agent: *mut Agent, out_data: *mut u8, out_len: *mut usize) -> AgentResult;

// P2P Connection FFI
pub unsafe extern "C" fn agent_connect_p2p(agent: *mut Agent, host: *const c_char, port: u16) -> AgentResult;
pub unsafe extern "C" fn agent_is_p2p_connected(agent: *const Agent, host: *const c_char, port: u16) -> bool;
pub unsafe extern "C" fn agent_poll_p2p(agent: *mut Agent, out_data: *mut u8, out_len: *mut usize, out_ip: *mut u8, out_port: *mut u16) -> AgentResult;
pub unsafe extern "C" fn agent_send_datagram_p2p(agent: *mut Agent, data: *const u8, len: usize, dest_ip: *const u8, dest_port: u16) -> AgentResult;

// Hole Punching FFI
pub unsafe extern "C" fn agent_start_hole_punch(agent: *mut Agent, service_id: *const c_char) -> AgentResult;
pub unsafe extern "C" fn agent_poll_hole_punch(agent: *mut Agent, out_ip: *mut u8, out_port: *mut u16, out_complete: *mut u8) -> AgentResult;
pub unsafe extern "C" fn agent_poll_binding_request(agent: *mut Agent, out_data: *mut u8, out_len: *mut usize, out_ip: *mut u8, out_port: *mut u16) -> AgentResult;
pub unsafe extern "C" fn agent_process_binding_response(agent: *mut Agent, data: *const u8, len: usize, from_ip: *const u8, from_port: u16) -> AgentResult;

// Path Resilience FFI
pub unsafe extern "C" fn agent_poll_keepalive(agent: *mut Agent, out_ip: *mut u8, out_port: *mut u16, out_data: *mut u8) -> AgentResult;
pub unsafe extern "C" fn agent_get_active_path(agent: *const Agent) -> u8;  // 0=Direct, 1=Relay, 2=None
pub unsafe extern "C" fn agent_is_in_fallback(agent: *const Agent) -> bool;
pub unsafe extern "C" fn agent_get_path_stats(agent: *const Agent, out_missed: *mut u32, out_rtt: *mut u64, out_fallback: *mut u8) -> AgentResult;
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

// P2P packet routing (checks active path before sending)
if isP2PActive, agent_get_active_path(agent) == 0 {  // 0 = Direct
    sendP2PDatagram(agent: agent, packet: data)       // via agent_send_datagram_p2p
} else {
    sendRoutedDatagram(agent: agent, ...)              // via relay
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
