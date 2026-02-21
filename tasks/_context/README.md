# ZTNA Agent - Task Context

**Read this first before working on any task.**

---

## Project Overview

Zero Trust Network Access (ZTNA) agent for macOS that intercepts packets, encapsulates them in QUIC tunnels, and routes through an intermediate system to application connectors.

## Architectural Goal: Direct P2P First

**Primary objective:** Establish direct peer-to-peer QUIC connections between Agent and App Connector.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONNECTION PRIORITY                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  PRIORITY 1 (Goal):     Agent ◄────── Direct QUIC ──────► Connector         │
│  PRIORITY 2 (Fallback): Agent ◄──► Intermediate ◄──► Connector              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The Intermediate System serves two purposes:**
1. **Bootstrap:** Initial connection establishment, address discovery (QAD)
2. **Fallback:** Relay traffic when NAT/firewall prevents direct connection

**Implementation approach:** Build relay infrastructure first (Tasks 002-004), then add hole punching to achieve direct P2P (Task 005).

---

## Task Overview

| Task | Component | Status | Branch |
|------|-----------|--------|--------|
| [001](../001-quic-tunnel-integration/) | Agent QUIC Client | ✅ Complete | `master` |
| [002](../002-intermediate-server/) | Intermediate Server | ✅ Complete | `master` |
| [003](../003-app-connector/) | App Connector | ✅ Complete | `master` |
| [004](../004-e2e-relay-testing/) | E2E Relay Testing | ✅ Complete | `master` |
| [005](../005-p2p-hole-punching/) | P2P Hole Punching | ✅ Complete | `master` |
| [005a](../005a-swift-agent-integration/) | Swift Agent Integration | ✅ Complete | `master` |
| [006](../006-cloud-deployment/) | Cloud Deployment | ✅ Complete (MVP) | `master` (PR #7 merged) |
| [007](../007-security-hardening/) | Security Hardening | ⏳ Not Started (26 findings documented) | — |
| [008](../008-production-operations/) | Production Operations | ⏳ Not Started | — |
| [009](../009-multi-service-architecture/) | Multi-Service Architecture | ⏳ Not Started | — |
| [010](../010-admin-dashboard/) | Admin Dashboard | ⏳ Not Started | — |
| [011](../011-protocol-improvements/) | Protocol Improvements | ⏳ Not Started | — |
| [012](../012-multi-environment-testing/) | Multi-Environment Testing | ⏳ Not Started | — |
| [013](../013-swift-modernization/) | Swift Modernization | ✅ Complete | `master` (PR #7) |

### Task Dependencies

```
001 (Agent Client) ✅
         │
         ▼
002 (Intermediate Server) ✅ ───┐
         │                      │
         ▼                      ▼
003 (App Connector) ✅ ◄── 004 (E2E Testing) ✅
         │                      │
         └──────────┬───────────┘
                    ▼
         005 (P2P Hole Punching) ✅
                    │
                    ▼
         005a (Swift Agent Integration) ✅
                    │
                    ▼
         006 (Cloud Deployment) ✅ COMPLETE (MVP)
                    │
                    ├──► 013 (Swift Modernization) ✅ COMPLETE
                    │
         ┌──────────┼──────────────────────┐
         ▼          ▼                      ▼
   007 (Security)  009 (Multi-Service)   011 (Protocol)
   P1 (26 findings) P2                   P3
         │          │                      │
         ▼          ▼                      │
   008 (Prod Ops)  010 (Dashboard)       012 (Multi-Env)
   P2              P3                     P3
```

---

## Branching Strategy

Each task uses a feature branch workflow:

```bash
# Before starting a task:
git checkout master
git pull origin master
git checkout -b feature/XXX-task-name

# While working:
git add . && git commit -m "descriptive message"

# When complete:
git push -u origin feature/XXX-task-name
# Create PR for review → Merge to master
```

---

## Component Architecture

```
┌─────────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
│   macOS Endpoint    │     │  Intermediate System │     │  App Connector      │
│                     │     │                      │     │                     │
│  ┌───────────────┐  │     │  - QUIC Server       │     │  - QUIC Client      │
│  │ SwiftUI App   │  │     │  - QAD (addr discov) │     │  - Decapsulates     │
│  │ (configurable │  │     │  - 0x2F svc routing  │     │  - UDP/TCP/ICMP     │
│  │  host/port/   │  │     │  - Relay (fallback)  │     │  - JSON config      │
│  │  service)     │  │     │  - Signaling (P2P)   │     │  - Keepalive        │
│  └───────┬───────┘  │     │  - JSON config       │     │  - P2P server mode  │
│          │          │     └──────────▲───────────┘     └──────────▲──────────┘
│  ┌───────▼───────┐  │                │                            │
│  │ NEPacketTun.  │  │                │    QUIC Tunnel             │
│  │ Provider      │──┼────────────────┴────────────────────────────┘
│  │ (route table, │  │      (relay or direct P2P, 0x2F service-routed)
│  │  0x2F wrap,   │  │
│  │  P2P+hole     │  │
│  │  punch)       │  │
│  └───────┬───────┘  │
│          │ FFI      │
│  ┌───────▼───────┐  │
│  │ Rust Core     │  │
│  │ (quiche)      │  │
│  └───────────────┘  │
└─────────────────────┘
```

---

## Key Technologies

| Component | Technology | Notes |
|-----------|------------|-------|
| QUIC Library | `quiche` (Cloudflare) | Sans-IO model, Rust |
| Agent | Swift 6 (strict concurrency) + Rust FFI | NetworkExtension framework, macOS 26.2+ |
| Intermediate Server | Rust + mio | Event loop matches quiche examples |
| Connector | Rust + mio | Matches Intermediate (mio chosen over tokio) |
| Packet Encapsulation | QUIC DATAGRAM | RFC 9221 |
| Address Discovery | QAD | Replaces STUN |
| Linting | clippy + rustfmt, SwiftLint, ShellCheck | GitHub Actions CI + pre-commit hooks |

---

## Key Files

| Component | Path |
|-----------|------|
| Architecture Doc | `docs/architecture.md` |
| Agent Extension | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| Agent FFI Bridge | `ios-macos/ZtnaAgent/Extension/AgentFFI.swift` |
| Agent Utilities | `ios-macos/ZtnaAgent/ZtnaAgent/TunnelUtilities.swift` |
| Agent UI + VPN Manager | `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` |
| Rust QUIC Client | `core/packet_processor/src/lib.rs` |
| P2P Modules | `core/packet_processor/src/p2p/` |
| Bridging Header | `ios-macos/Shared/PacketProcessor-Bridging-Header.h` |
| CI Lint Workflow | `.github/workflows/lint.yml` |
| Pre-commit Config | `.pre-commit-config.yaml` |
| SwiftLint Config | `.swiftlint.yml` |
| Intermediate Server | `intermediate-server/src/main.rs` |
| Signaling Module | `intermediate-server/src/signaling.rs` |
| App Connector | `app-connector/src/main.rs` |
| E2E Test Framework | `tests/e2e/README.md` |
| E2E Test Runner | `tests/e2e/run-mvp.sh` |
| Agent Config (reference) | `deploy/config/agent.json` |
| Connector Config (reference) | `deploy/config/connector.json` |
| Intermediate Config (reference) | `deploy/config/intermediate.json` |

---

## Session Resume Instructions

When resuming work on any task:

1. **Read this file first** (`tasks/_context/README.md`)
2. **Check component status** (`tasks/_context/components.md`)
3. **Read the specific task's state.md** (e.g., `tasks/002-intermediate-server/state.md`)
4. **Review the task's todo.md** for current progress
5. **Ensure you're on the correct branch** (`git branch`)

---

## Build & Test Commands

### Build All Components

```bash
cd /Users/hank/dev/src/agent-driver/ztna-agent

# Rust library (for macOS Agent FFI)
(cd core/packet_processor && cargo build --release --target aarch64-apple-darwin)

# Intermediate Server + App Connector
(cd intermediate-server && cargo build --release)
(cd app-connector && cargo build --release)

# Test fixtures
(cd tests/e2e/fixtures/echo-server && cargo build --release)
(cd tests/e2e/fixtures/quic-client && cargo build --release)

# macOS Agent app
xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
    -scheme ZtnaAgent -configuration Debug \
    -derivedDataPath /tmp/ZtnaAgent-build build

# Run all unit tests (114+ tests)
cargo test --workspace
```

### Run macOS Agent Demo

```bash
# Full automated demo (builds everything, runs for 30 seconds)
tests/e2e/scenarios/macos-agent-demo.sh --build --auto --duration 30

# Interactive demo (manual Start/Stop buttons)
tests/e2e/scenarios/macos-agent-demo.sh --manual

# Just run app with automation flags
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app \
    --args --auto-start --auto-stop 30 --exit-after-stop
```

### Run E2E Test Suites

```bash
# Full E2E suite (61+ tests, shell-based)
tests/e2e/run-mvp.sh

# Individual test suites
tests/e2e/scenarios/protocol-validation.sh   # 14 tests
tests/e2e/scenarios/udp-advanced.sh          # 11 tests
tests/e2e/scenarios/reliability-tests.sh     # 11 tests
tests/e2e/scenarios/performance-metrics.sh   # 6 tests
```

### Run Docker NAT Simulation Demo

```bash
# Full demo (builds Docker images + runs NAT simulation test)
tests/e2e/scenarios/docker-nat-demo.sh

# Skip builds if images already exist
tests/e2e/scenarios/docker-nat-demo.sh --no-build

# Cleanup Docker resources
tests/e2e/scenarios/docker-nat-demo.sh --clean

# Multi-terminal log watching (open 4 terminals)
deploy/docker-nat-sim/watch-logs.sh intermediate  # Terminal 1
deploy/docker-nat-sim/watch-logs.sh connector     # Terminal 2
deploy/docker-nat-sim/watch-logs.sh traffic       # Terminal 3
# Terminal 4: Run the demo script
```

### Run Pi k8s Cluster Demo

```bash
# 1. Verify cluster access
kubectl --context k8s1 get nodes

# 2. Check deployed components
kubectl --context k8s1 get pods -n ztna
kubectl --context k8s1 get svc -n ztna

# 3. Test connection from macOS
./app-connector/target/release/app-connector \
  --server 10.0.150.205:4433 \
  --service test-from-mac \
  --insecure

# Multi-terminal log watching
# Terminal 1: Intermediate server
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# Terminal 2: App connector
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=app-connector -f

# Terminal 3: Watch pods
watch -n2 'kubectl --context k8s1 get pods -n ztna -o wide'

# Terminal 4: Run test connection (above command)
```

**Note:** App Connector CrashLoopBackOff is expected - 30 second QUIC idle timeout causes restart when no traffic.

### Run macOS ZtnaAgent E2E with k8s

```bash
# 1. Verify Extension has k8s IP
grep "serverHost" ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift
# Should show: private let serverHost = "10.0.150.205"

# 2. Clean and rebuild if needed
rm -rf ~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*
xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
    -scheme ZtnaAgent -configuration Debug \
    -derivedDataPath /tmp/ZtnaAgent-build build

# 3. Launch app (Click Start or use auto-start)
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app --args --auto-start

# 4. Monitor k8s for connection
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f
# Look for: "New connection from 10.0.0.22:XXXXX"

# 5. Send UDP test traffic to echo-service (10.100.0.1 = virtual service IP)
echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999
# K8s logs: "Received XX bytes to relay" (UDP packet tunneled)
```

### View Logs

```bash
# macOS Agent logs (real-time)
log stream --predicate 'subsystem CONTAINS "ztna"' --info

# Recent macOS Agent logs
log show --last 5m --predicate 'subsystem CONTAINS "ztna"' --info

# Server/Connector logs
tail -f tests/e2e/artifacts/logs/*.log
```

---

## Glossary

| Term | Definition |
|------|------------|
| **QAD** | QUIC Address Discovery - learns public IP via QUIC (replaces STUN) |
| **DATAGRAM** | QUIC frame type for unreliable data (RFC 9221) |
| **Hole Punching** | NAT traversal technique for direct P2P connection |
| **Intermediate** | Relay server for bootstrap and fallback |
| **Connector** | Component that decapsulates packets and forwards to apps |
| **Service Registration** | Protocol for Agents/Connectors to register with Intermediate for routing |
| **0x2F Datagram** | Service-routed datagram: `[0x2F, id_len, service_id, ip_packet]` |
| **Split Tunnel** | Only configured virtual IPs (10.100.0.0/24) go through QUIC tunnel |

### Registration & Routing Protocol

Both Agents and Connectors register with a service ID to enable relay routing:
- **Agent (0x10)**: "I want to reach service X" (can register multiple)
- **Connector (0x11)**: "I provide service X"
- **Service-Routed Datagram (0x2F)**: Per-packet routing with embedded service ID

Registration format: `[type_byte, service_id_length, service_id_bytes...]`
Routed datagram: `[0x2F, service_id_length, service_id_bytes..., ip_packet_bytes...]`

The Intermediate strips the 0x2F wrapper before forwarding to the Connector.
See `tasks/_context/components.md` for full protocol details.

---

## Deferred Items / Technical Debt

Items deferred from MVP implementation that must be addressed for production.

### Priority 1: Security (Required for Production)

| Item | Component | Description | Risk if Missing |
|------|-----------|-------------|-----------------|
| **Stateless Retry** | 002-Server | Anti-amplification protection via HMAC tokens (→ Task 007) | DoS amplification attacks |
| **TLS Certificate Verification** | 001-Agent, 002-Server | Currently `verify_peer(false)` (→ Task 007) | MITM attacks |
| **Client Authentication** | 002-Server | No auth - any client can connect (→ Task 007) | Unauthorized access |
| **Rate Limiting** | 002-Server | No per-client DATAGRAM rate limits (→ Task 007) | Resource exhaustion |

### Priority 2: Reliability (Recommended)

| Item | Component | Description | Impact if Missing |
|------|-----------|-------------|-------------------|
| **Graceful Shutdown** | 002-Server | Connection draining on shutdown (→ Task 008) | Abrupt disconnects |
| **Connection State Tracking** | 002-Server | Full state machine for connections (→ Task 008) | Edge case bugs |
| ~~Error Recovery (Agent)~~ | ~~001-Agent~~ | ✅ Done (Task 006 Phase 4.9) — Auto-reconnect with exponential backoff, NWPathMonitor, 3 detection paths | Agent auto-recovers |
| **Error Recovery (Server/Connector)** | 002-Server, 003-Connector | Automatic reconnection logic (→ Task 008) | Manual intervention needed |
| ~~TCP Support~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.4) | Userspace TCP proxy |
| **Registration Acknowledgment** | 002-Server, 003-Connector | Server doesn't ACK registration (→ Task 007) | Silent registration failures |
| ~~Return-Path DATAGRAM→TUN~~ | ~~001-Agent~~ | ✅ Done (Task 006 Phase 4.6) | `agent_recv_datagram()` FFI + `drainIncomingDatagrams()` |

### Priority 3: Operations (Nice to Have)

| Item | Component | Description |
|------|-----------|-------------|
| **Metrics/Stats Endpoint** | 002-Server, 003-Connector | Connection counts, packet rates, latency (→ Task 008) |
| ~~Configuration File~~ | ~~002-Server, 003-Connector~~ | ✅ Done (Task 006 Phase 4.2) - JSON configs |
| **Multiple Bind Addresses** | 002-Server | Only `0.0.0.0:4433` supported (→ Task 008) |
| **IPv6 QAD Support** | 001-Agent, 002-Server, 003-Connector | Currently IPv4 only, 7-byte format (→ Task 011) |
| **Production Certificates** | All | Currently using self-signed dev certs (→ Task 007) |
| ~~ICMP Support~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.5) - Echo Reply |
| ~~Multiple Service Registration~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.3) - 0x2F routing |
| **Per-Service Backend Routing** | 003-Connector | Route different services to different backends (→ Task 009) |
| **TCP Window Flow Control** | 003-Connector | Currently simple ACK-per-segment (→ Task 011) |
| **QUIC Connection Migration** | 001-Agent | quiche doesn't support — full reconnect used instead (→ Task 011) |
| **QUIC 0-RTT Reconnection** | 001-Agent | Requires session ticket storage in quiche (→ Task 011) |
| **Multiplexed QUIC Streams** | 002-Server | DATAGRAMs sufficient for current relay needs (→ Task 011) |
| ~~P2P NAT Testing~~ | ~~006-Cloud~~ | ✅ Done (Task 006 Phase 6.8) — Direct P2P path achieved, keepalive demux fix in Rust `recv()` |

### Tracking

Post-MVP tasks (007-012) have been created to address these deferred items.
Each item above references its target task number (→ Task NNN).
When implementing, reference this section in the task's `plan.md` and update
this table when complete (change to ✅ and add commit reference).

---

## Cloud Deployment Strategy

After E2E testing validates local relay functionality, components will be deployed to cloud infrastructure for NAT testing and production readiness.

### Deployment Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CLOUD DEPLOYMENT ARCHITECTURE                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐                ┌─────────────────────────────────────────┐ │
│  │   Agent     │                │           Cloud Infrastructure           │ │
│  │  (macOS)    │                │           (AWS EC2: 3.128.36.92)        │ │
│  │             │                │                                          │ │
│  │  Config:    │                │  ┌─────────────────────────────────────┐ │ │
│  │  services:  │                │  │    Intermediate Server              │ │ │
│  │  - echo-svc │◄──── QUIC ────►│  │    :4433 (intermediate.json)       │ │ │
│  │    10.100.  │  0x2F routed   │  │    - QAD + DATAGRAM relay          │ │ │
│  │    0.1      │                │  │    - 0x2F service routing           │ │ │
│  │  - web-app  │                │  │    - Multi-service registry         │ │ │
│  │    10.100.  │                │  └─────────────────────────────────────┘ │ │
│  │    0.2      │                │                    │                      │ │
│  │             │                │                    │ QUIC                 │ │
│  │  Routes:    │                │                    ▼                      │ │
│  │  10.100.0.  │                │  ┌─────────────────────────────────────┐ │ │
│  │  0/24→utun  │                │  │    App Connector                    │ │ │
│  │             │                │  │    (connector.json)                 │ │ │
│  │  All other  │                │  │    - UDP/TCP/ICMP forwarding        │ │ │
│  │  traffic:   │                │  │    - TCP session proxy              │ │ │
│  │  normal     │                │  │    - ICMP Echo Reply                │ │ │
│  └─────────────┘                │  └───────────────┬─────────────────────┘ │ │
│                                  │                   │                       │ │
│                                  │                   │ Local UDP/TCP         │ │
│                                  │                   ▼                       │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    Internal Services                │ │ │
│                                  │  │    echo-server :9999 (UDP)          │ │ │
│                                  │  │    web-app :8080 (TCP)              │ │ │
│                                  │  └─────────────────────────────────────┘ │ │
│                                  └──────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Deployment Targets

| Component | Deployment | Purpose |
|-----------|------------|---------|
| Intermediate Server | Cloud (public IP) | Bootstrap, QAD, relay fallback |
| App Connector | Cloud/Edge | Service exposure, traffic termination |
| Agent | Local (macOS) | Endpoint packet interception |

### Cloud Provider Options

| Provider | Service | Notes |
|----------|---------|-------|
| AWS | EC2, Lightsail | Flexible, well-documented |
| GCP | Compute Engine | Good networking options |
| DigitalOcean | Droplets | Simple, cost-effective |
| Vultr | Cloud Compute | Low-cost, global |

### Deployment Requirements

**Intermediate Server:**
- Public IPv4 address
- UDP port 4433 open (QUIC)
- TLS certificate (production: Let's Encrypt)
- Minimal resources: 1 vCPU, 512MB RAM

**App Connector:**
- Network access to internal services
- Can be same or different machine as Intermediate
- UDP port range for local services

### Task Reference

See [Task 006: Cloud Deployment](../006-cloud-deployment/) for implementation details.

---

## MVP Boundary (Task 006 Complete)

**Everything below constitutes the MVP — fully implemented and validated:**

- Full E2E relay (UDP/TCP/ICMP) through QUIC DATAGRAM tunnel
- P2P hole punching with automatic per-packet relay fallback
- Multi-service routing (0x2F protocol, 2 services: echo-service + web-app)
- Connection resilience (auto-recovery, NWPathMonitor, exponential backoff)
- Split-tunnel architecture (only 10.100.0.0/24 tunneled)
- macOS Agent with SwiftUI config UI + 23 FFI functions
- AWS EC2 deployment (Intermediate + 2 Connectors + echo + HTTP services)
- Pi k8s deployment (Kustomize + Cilium L2 LoadBalancer)
- 177+ tests (116 unit + 61+ E2E)
- Performance: P2P 32.6ms vs Relay 76ms (2.3x faster), 10-min 0% loss, seamless failover

**Post-MVP completions (merged in PR #7):**

- Swift 6 language mode with strict concurrency (`SWIFT_STRICT_CONCURRENCY = complete`)
- macOS deployment target aligned to 26.2
- Extracted `AgentFFI.swift` (FFI boundary) and `TunnelUtilities.swift` (IP parsing)
- Unit tests: `TunnelUtilitiesTests` + `VPNManagerTests` (Swift Testing framework)
- Multi-language linting CI: GitHub Actions (Rust clippy/fmt, SwiftLint, ShellCheck)
- Pre-commit hooks: 12 hooks (5 rustfmt, 5 clippy, 1 shellcheck, 1 swiftlint)
- Security review: 26 findings documented in task 007 (1 Critical, 4 High, 8 Medium, 9 Low, 4 Info)

**Everything below this line is post-MVP.**

---

## Post-MVP Roadmap

| Task | Name | Priority | Description | Dependencies |
|------|------|----------|-------------|--------------|
| [007](../007-security-hardening/) | Security Hardening | P1 | 26 findings: TLS certs, client auth, rate limiting, FFI safety, protocol hardening | None |
| [008](../008-production-operations/) | Production Operations | P2 | Prometheus metrics, graceful shutdown, deployment automation, CI/CD | 007 |
| [009](../009-multi-service-architecture/) | Multi-Service Architecture | P2 | Per-service backend routing, dynamic discovery, health checks | None |
| [010](../010-admin-dashboard/) | Admin Dashboard | P3 | REST API on Intermediate, web frontend, topology visualization | 008, 009 |
| [011](../011-protocol-improvements/) | Protocol Improvements | P3 | IPv6 QAD, TCP flow control, separate P2P/relay sockets, QUIC migration, 0-RTT | None |
| [012](../012-multi-environment-testing/) | Multi-Environment Testing | P3 | DigitalOcean, multi-region, symmetric NAT/CGNAT, load testing | None |
| ~~[013](../013-swift-modernization/)~~ | ~~Swift Modernization~~ | ~~Done~~ | ~~Swift 6, strict concurrency, deployment target 26.2, linting infra~~ | ~~None~~ |
