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
| [001](../done/001-quic-tunnel-integration/) | Agent QUIC Client | ✅ Complete | `master` |
| [002](../done/002-intermediate-server/) | Intermediate Server | ✅ Complete | `master` |
| [003](../done/003-app-connector/) | App Connector | ✅ Complete | `master` |
| [004](../done/004-e2e-relay-testing/) | E2E Relay Testing | ✅ Complete | `master` |
| [005](../done/005-p2p-hole-punching/) | P2P Hole Punching | ✅ Complete | `master` |
| [005a](../done/005a-swift-agent-integration/) | Swift Agent Integration | ✅ Complete | `master` |
| [006](../done/006-cloud-deployment/) | Cloud Deployment | ✅ Complete (MVP) | `master` (PR #7 merged) |
| [007](../done/007-security-hardening/) | Security Hardening | ✅ Complete (Phases 1-8, PR #8 merged) | `master` |
| [008](../done/008-production-operations/) | Production Operations | ✅ Complete | `master` (PR #11 merged) |
| [009](../009-multi-service-architecture/) | Multi-Service Architecture | ⏳ Not Started | — |
| [010](../010-admin-dashboard/) | Admin Dashboard | ⏳ Not Started | — |
| [011](../011-protocol-improvements/) | Protocol Improvements | ⏳ Not Started | — |
| [012](../012-multi-environment-testing/) | Multi-Environment Testing | ⏳ Not Started | — |
| [013](../done/013-swift-modernization/) | Swift Modernization | ✅ Complete | `master` (PR #7) |
| [014](../done/014-pr-comment-graphql-hardening/) | PR Comment GraphQL Hardening | ✅ Complete | `master` |
| [015](../done/015-oracle-quick-fixes/) | Oracle Quick Fixes | ✅ Complete | `master` (PR #10 merged) |
| [016](../016-infrastructure-architecture/) | Infrastructure Architecture | ⏳ Not Started | — |

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
                    ├──► 014 (PR Comment GraphQL) ✅ COMPLETE
                    │
         ┌──────────┼──────────────────────┐
         ▼          ▼                      ▼
   007 (Security) ✅ 009 (Multi-Service)   011 (Protocol)
   P1 COMPLETE     P2                   P3
         │          │                      │
         ▼          ▼                      │
   008 (Prod Ops) ✅ 010 (Dashboard)       012 (Multi-Env)
   COMPLETE        P3                     P3

  016 (Infra Architecture) P2
  - Separates components onto independent AWS infrastructure
  - Docker on EC2 (host networking) for Intermediate Server
  - ECS/Fargate for App Connector, Admin Panel, test backends
  - Admin panel for policy management
  - Coordinate with 008 (graceful shutdown needed for cutover)

  ★ Oracle Review (cross-cutting, see Deferred Items) ★
  015 (Quick Fixes): IPv6 panic, predictable IDs, dead FFI, UDP sanity ✅
  Signaling hijack → 009    Reg auth hardening → 009
  Cross-tenant routing → 009  Local UDP injection → 008 ✅
  DATAGRAM mismatch → 011   Endian bug (disputed) → 011
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
│  └───────┬───────┘  │     │  - mTLS + cert reload│     │  - P2P server mode  │
│          │          │     │  - Stateless retry   │     │  - Non-blocking TCP │
│          │          │     │  - Reg ACK/NACK      │     │  - Rate limiting    │
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
| Metrics | Prometheus text format (lock-free atomics) | mio TcpListener HTTP, `--metrics-port` CLI flag |
| Deployment | Terraform, Ansible, Docker, Kustomize | `deploy/terraform/`, `deploy/ansible/`, `deploy/docker/` |
| CI/CD | GitHub Actions | `test.yml` (5-crate matrix), `release.yml` (cross-compile + Docker) |

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

# Run all unit tests (146+ tests)
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
  --server ${K8S_LB_HOST}:${K8S_LB_PORT} \
  --service test-from-mac \
  --no-verify-peer

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

# 5. Send UDP test traffic to echo-service ($ZTNA_ECHO_VIRTUAL_IP = virtual service IP)
echo "ZTNA-TEST" | nc -u -w1 $ZTNA_ECHO_VIRTUAL_IP 9999
# K8s logs: "Received XX bytes to relay" (UDP packet tunneled)
```

### View Logs

```bash
# macOS Agent logs (real-time)
log stream --predicate 'subsystem CONTAINS "ztna"' --info

# Recent macOS Agent logs
log show --last 5m --predicate 'subsystem CONTAINS "ztna"' --info

# Server/Connector logs (local E2E)
tail -f tests/e2e/artifacts/logs/*.log

# AWS service logs (real-time) — uses $ZTNA_SSH from demo-runbook.md Configuration
$ZTNA_SSH 'sudo journalctl -u ztna-intermediate -f'
$ZTNA_SSH 'sudo journalctl -u ztna-connector -f'
```

### Metrics & Health (Task 008)

Both Intermediate Server and App Connector expose Prometheus metrics and health endpoints.
Configurable via `--metrics-port` CLI flag (default 9090/9091, pass `0` to disable).

Commands use variables from demo-runbook.md Configuration section and testing-guide.md Configuration section (`$ZTNA_SSH`, `$ZTNA_ECHO_VIRTUAL_IP`, etc.).

```bash
# Health checks (Intermediate binds to --bind addr, connectors bind to 0.0.0.0)
$ZTNA_SSH "curl -s http://${ZTNA_INTERMEDIATE_BIND}:${ZTNA_INTERMEDIATE_METRICS_PORT}/healthz"  # → "ok"
$ZTNA_SSH "curl -s http://localhost:${ZTNA_CONNECTOR_METRICS_PORT}/healthz"                      # → "ok"
$ZTNA_SSH "curl -s http://localhost:${ZTNA_CONNECTOR_WEB_METRICS_PORT}/healthz"                  # → "ok"

# Intermediate Server metrics (9 counters)
$ZTNA_SSH "curl -s http://${ZTNA_INTERMEDIATE_BIND}:${ZTNA_INTERMEDIATE_METRICS_PORT}/metrics"
# Key counters: ztna_active_connections, ztna_relay_bytes_total,
#   ztna_registrations_total, ztna_registration_rejections_total,
#   ztna_datagrams_relayed_total, ztna_signaling_sessions_total,
#   ztna_retry_tokens_validated, ztna_retry_token_failures, ztna_uptime_seconds

# App Connector metrics (6 counters each)
$ZTNA_SSH "curl -s http://localhost:${ZTNA_CONNECTOR_METRICS_PORT}/metrics"      # echo-service
$ZTNA_SSH "curl -s http://localhost:${ZTNA_CONNECTOR_WEB_METRICS_PORT}/metrics"  # web-app
# Key counters: ztna_connector_forwarded_packets_total, ztna_connector_forwarded_bytes_total,
#   ztna_connector_tcp_sessions_total, ztna_connector_tcp_errors_total,
#   ztna_connector_reconnections_total, ztna_connector_uptime_seconds

# Watch metrics live (refreshes every 2s)
$ZTNA_SSH "watch -n2 'curl -s http://${ZTNA_INTERMEDIATE_BIND}:${ZTNA_INTERMEDIATE_METRICS_PORT}/metrics | grep -v ^#'"

# Note: Metrics ports are NOT in the AWS security group by default.
# To reach externally, set Terraform enable_metrics_port=true
# or use SSH tunnel: ssh -L 9090:${ZTNA_INTERMEDIATE_BIND}:9090 -L 9091:localhost:9091 $ZTNA_SSH_HOST

# Auto-reconnect test (NOT zero-downtime — ~30-40s gap during QUIC idle timeout)
$ZTNA_SSH 'sudo systemctl restart ztna-intermediate'
# Watch connector reconnect (~30-40s after restart):
$ZTNA_SSH 'sudo journalctl -u ztna-connector -f'
# Look for: "Reconnect attempt 1 — waiting 1000ms" then "Successfully reconnected"
# Verify: curl -s http://localhost:${ZTNA_CONNECTOR_METRICS_PORT}/metrics | grep reconnections
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
| **mTLS** | Mutual TLS — server validates client certificates for identity + service authorization (Task 007) |
| **Stateless Retry** | QUIC anti-amplification: server sends Retry with AEAD token before accepting (Task 007) |
| **Registration ACK** | Server confirms registration with 0x12 ACK or 0x13 NACK; clients retry with backoff (Task 007) |
| **CID Rotation** | Periodic QUIC Connection ID rotation for privacy (Task 007) |
| **ZTNA_MAGIC** | `0x5A` prefix byte distinguishing P2P control messages from QUIC packets (Task 007) |
| **Prometheus Metrics** | Lock-free atomic counters exposed at `/metrics` in Prometheus text format (Task 008) |
| **Health Check** | HTTP 200 `ok` response at `/healthz` endpoint — indicates component is running (Task 008) |
| **Graceful Shutdown** | SIGTERM/SIGINT triggers `drain_and_shutdown()` — APPLICATION_CLOSE to all clients, 3s drain period (Task 008) |
| **Auto-Reconnect** | Connector reconnects to Intermediate with exponential backoff (1s→30s cap) on connection loss (Task 008) |

### Registration & Routing Protocol

Both Agents and Connectors register with a service ID to enable relay routing:
- **Agent (0x10)**: "I want to reach service X" (can register multiple)
- **Connector (0x11)**: "I provide service X"
- **Registration ACK (0x12)**: Server confirms successful registration
- **Registration NACK (0x13)**: Server denies registration (auth failure or invalid)
- **Service-Routed Datagram (0x2F)**: Per-packet routing with embedded service ID

Registration format: `[type_byte, service_id_length, service_id_bytes...]`
ACK/NACK format: `[type_byte, status, service_id_length, service_id_bytes...]`
Routed datagram: `[0x2F, service_id_length, service_id_bytes..., ip_packet_bytes...]`

The Intermediate strips the 0x2F wrapper before forwarding to the Connector.
See `tasks/_context/components.md` for full protocol details.

---

## Deferred Items / Technical Debt

Items deferred from MVP implementation that must be addressed for production.

### Priority 1: Security (Required for Production)

| Item | Component | Description | Risk if Missing |
|------|-----------|-------------|-----------------|
| ~~Stateless Retry~~ | ~~002-Server~~ | ✅ Done (Task 007 Phase 7B) — AEAD retry tokens via `quiche::retry()` | ~~DoS amplification~~ |
| ~~TLS Certificate Verification~~ | ~~001-Agent, 002-Server~~ | ✅ Done (Task 007 Phase 1) — `verify_peer(true)` + CA cert loading | ~~MITM attacks~~ |
| ~~Client Authentication~~ | ~~002-Server~~ | ✅ Done (Task 007 Phase 6A) — mTLS with x509 SAN-based service authorization | ~~Unauthorized access~~ |
| ~~Rate Limiting~~ | ~~002-Server~~ | ✅ Done (Task 007 Phase 3) — Per-IP TCP SYN rate limiting | ~~Resource exhaustion~~ |

### Priority 1.5: Oracle Review Findings (Security — Open)

Findings from the original Oracle review not addressed by Task 007's 26-finding scope.

| Severity | Item | Component | Description | Target Task |
|----------|------|-----------|-------------|-------------|
| **Critical** | **Registration auth (conditional)** | 002-Server | mTLS requires `--require-client-cert` flag; SAN-less certs allowed. Oracle: conditionally fixed only | → Task 009 |
| **High** | **Signaling session hijack** | 002-Server | `CandidateAnswer` accepted from any conn with matching session_id — no ownership check. Oracle confirmed NOT fixed by Task 007 | → Task 009 |
| **High** | **Cross-tenant connector routing** | 003-Connector | "First flow wins" return-path; responses can route to wrong agent | → Task 009 |
| ~~**High**~~ | ~~**IPv6 QAD panic**~~ | ~~002-Server~~ | ✅ Done (Task 015) — `build_observed_address()` returns `Option<Vec<u8>>`, panic replaced with `log::warn` + `None`. Full IPv6 QAD in Task 011 | ✅ Task 015 |
| ~~**High**~~ | ~~**Local UDP injection**~~ | ~~003-Connector~~ | ✅ Done (Task 008) — Source IP validation in `process_local_socket()` against `forward_addr`, drops unexpected sources with `log::warn` | ✅ Task 008 |
| ~~**Medium**~~ | ~~**Predictable P2P identifiers**~~ | ~~packet_processor~~ | ✅ Done (Task 015) — `ring::rand::SystemRandom` CSPRNG replaces time+PID in `generate_session_id()` and `generate_transaction_id()` | ✅ Task 015 |
| **Medium** | **DATAGRAM size mismatch** | All Rust | Constants aligned at 1350, but effective writable limit ~1307. Needs investigation | → Task 011 |
| **Medium** | **Interface enumeration endian bug** | packet_processor | Oracle DISPUTES: `to_ne_bytes()` may be correct on macOS. Needs investigation, not blind fix | → Task 011 |
| ~~**Medium**~~ | ~~**Legacy FFI dead code**~~ | ~~packet_processor~~ | ✅ Done (Task 015) — `process_packet()`, `PacketAction` enum, bridging header decl, doc refs all removed | ✅ Task 015 |
| ~~**Medium**~~ | ~~**Service ID length truncation**~~ | ~~003-Connector~~ | ~~Fixed in Task 007 — bounds check before `u8` cast~~ | ✅ Task 007 |
| **Low** | **Hot-path per-packet allocations** | packet_processor, app-connector | Buffer reuse refactor across multiple hot paths | → Task 011 |
| ~~**Low**~~ | ~~**Local socket recv buffer**~~ | ~~003-Connector~~ | ✅ Done (Task 008) — `self.recv_buf` reuse with `to_vec()` copy, eliminates per-poll 65KB allocation | ✅ Task 008 |
| ~~**Low**~~ | ~~**UDP length sanity**~~ | ~~003-Connector~~ | ✅ Done (Task 015) — `udp_len < 8` guard drops malformed packets with `log::warn` | ✅ Task 015 |

### Priority 2: Reliability (Recommended)

| Item | Component | Description | Impact if Missing |
|------|-----------|-------------|-------------------|
| ~~Graceful Shutdown~~ | ~~002-Server~~ | ✅ Done (Task 008) — `drain_and_shutdown()`, SIGTERM/SIGINT handlers, 3s drain period, APPLICATION_CLOSE to all clients | ~~Abrupt disconnects~~ |
| **Connection State Tracking** | 002-Server | Full state machine for connections (→ Task 008) | Edge case bugs |
| ~~Error Recovery (Agent)~~ | ~~001-Agent~~ | ✅ Done (Task 006 Phase 4.9) — Auto-reconnect with exponential backoff, NWPathMonitor, 3 detection paths | Agent auto-recovers |
| ~~Error Recovery (Server/Connector)~~ | ~~002-Server, 003-Connector~~ | ✅ Done (Task 008) — Connector auto-reconnects with exponential backoff (1s→30s), interruptible 500ms sleep chunks, reg_state reset | ~~Manual intervention needed~~ |
| ~~TCP Support~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.4) | Userspace TCP proxy |
| ~~Registration Acknowledgment~~ | ~~002-Server, 003-Connector~~ | ✅ Done (Task 007 Phase 8A) — 0x12 ACK / 0x13 NACK with retry state machine | ~~Silent failures~~ |
| ~~Return-Path DATAGRAM→TUN~~ | ~~001-Agent~~ | ✅ Done (Task 006 Phase 4.6) | `agent_recv_datagram()` FFI + `drainIncomingDatagrams()` |

### Priority 3: Operations (Nice to Have)

| Item | Component | Description |
|------|-----------|-------------|
| ~~Metrics/Stats Endpoint~~ | ~~002-Server, 003-Connector~~ | ✅ Done (Task 008) — Prometheus text format on `/metrics`, health on `/healthz`, 9+6 atomic counters, `--metrics-port` CLI flag |
| ~~Configuration File~~ | ~~002-Server, 003-Connector~~ | ✅ Done (Task 006 Phase 4.2) - JSON configs |
| **Multiple Bind Addresses** | 002-Server | Only `0.0.0.0:4433` supported (→ Task 008) |
| **IPv6 QAD Support** | 001-Agent, 002-Server, 003-Connector | Currently IPv4 only, 7-byte format (→ Task 011) |
| ~~Production Certificates~~ | ~~All~~ | ✅ Done (Task 007 Phase 6B) — SIGHUP hot-reload, certbot DNS-01, cert-manager CRDs |
| ~~ICMP Support~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.5) - Echo Reply |
| ~~Multiple Service Registration~~ | ~~003-Connector~~ | ✅ Done (Task 006 Phase 4.3) - 0x2F routing |
| **Per-Service Backend Routing** | 003-Connector | Route different services to different backends (→ Task 009) |
| **TCP Window Flow Control** | 003-Connector | Currently simple ACK-per-segment (→ Task 011) |
| **QUIC Connection Migration** | 001-Agent | quiche doesn't support — full reconnect used instead (→ Task 011) |
| **QUIC 0-RTT Reconnection** | 001-Agent | Requires session ticket storage in quiche (→ Task 011) |
| **Multiplexed QUIC Streams** | 002-Server | DATAGRAMs sufficient for current relay needs (→ Task 011) |
| ~~P2P NAT Testing~~ | ~~006-Cloud~~ | ✅ Done (Task 006 Phase 6.8) — Direct P2P path achieved, keepalive demux fix in Rust `recv()` |

### Priority 4: Deferred from Task 008

Items that were deferred during Task 008 implementation because they require live infrastructure, external tooling, or are lower priority.

| Item | Component | Description | What's Needed |
|------|-----------|-------------|---------------|
| **Full Observability Stack** | Infrastructure | Deploy Prometheus server + Grafana to scrape `/metrics` endpoints and visualize dashboards. Alerting rules for reconnections, connection drops, error rates | Prometheus + Grafana instances (Docker or k8s); → Task 016 or dedicated observability task |
| **Grafana Dashboard JSON** | 002-Server, 003-Connector | Pre-built dashboard for Prometheus metrics (9+6 counters) | Grafana instance; import JSON; blocked by observability stack |
| **E2E Tests in CI** | CI/CD | GitHub Actions workflow for shell-based E2E test suite | Docker-in-CI infrastructure; server binary builds |
| **K8s Manifest Updates** | deploy/k8s | Production kustomize overlays for updated binaries | K8s cluster access; existing Pi kustomize works for dev |
| ~~Live Reconnect Test~~ | ~~003-Connector~~ | ~~Restart Intermediate, verify Connector auto-recovers~~ | ✅ Verified 2026-02-28 — Connector auto-reconnected, `reconnections_total` incremented |
| ~~Zero-Downtime Restart Test~~ | ~~002-Server~~ | ~~Verify graceful shutdown + restart~~ | ✅ Verified 2026-02-28 — Graceful drain works (EINTR fix in `d9c98b6`), instant connection close detection via APPLICATION_CLOSE. Not true zero-downtime (~30-40s gap without multiple intermediates) |
| **UDP Injection Mock Test** | 003-Connector | Unit test that UDP from unexpected source IP is dropped | Network mock / loopback test harness |
| **Buffer Reuse Benchmark** | 003-Connector | Measure allocation reduction in high-PPS scenarios | Perf benchmarking harness (criterion) |
| **Agent Service Unavailability Notification** | 002-Server, 001-Agent | Notify agents when connector goes down (currently agents detect via QUIC close) | Protocol extension; currently graceful enough via QUIC |
| **Multiple Bind Addresses** | 002-Server | Only `0.0.0.0:4433` supported currently | Multi-interface support; low priority |
| **MD040 Markdown Lint** | Docs | Pre-existing unlabeled fenced code blocks in `docs/architecture.md` (~20) and `docs/demo-runbook.md` (~3); Task 008 fixed only the new blocks it added | Bulk find-and-tag pass; low-priority cosmetic |

### Tracking

Post-MVP tasks (007-016) have been created to address these deferred items.
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
│  │    10.100.  │                │  │    - Metrics HTTP :9090             │ │ │
│  │    0.2      │                │  └─────────────────────────────────────┘ │ │
│  │    0.2      │                │                    │                      │ │
│  │             │                │                    │ QUIC                 │ │
│  │  Routes:    │                │                    ▼                      │ │
│  │  10.100.0.  │                │  ┌─────────────────────────────────────┐ │ │
│  │  0/24→utun  │                │  │    App Connector                    │ │ │
│  │             │                │  │    (connector.json)                 │ │ │
│  │  All other  │                │  │    - UDP/TCP/ICMP forwarding        │ │ │
│  │  traffic:   │                │  │    - TCP session proxy              │ │ │
│  │  normal     │                │  │    - ICMP Echo Reply                │ │ │
│  └─────────────┘                │  │    - Metrics HTTP :9091/:9092       │ │ │
│                                  │  └───────────────┬─────────────────────┘ │ │
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

### Deployment Automation (Task 008)

| Tool | Path | Purpose |
|------|------|---------|
| Terraform | `deploy/terraform/` | AWS VPC, EC2, SG, EIP provisioning |
| Ansible | `deploy/ansible/` | Service deployment, systemd templates, cert management |
| Docker | `deploy/docker/` | Production Dockerfiles (multi-stage, debian-slim, non-root) |
| CI/CD | `.github/workflows/test.yml` | Unit test matrix (5 crates) |
| CI/CD | `.github/workflows/release.yml` | Cross-compile, Docker images (GHCR), GitHub Releases |

### AWS Systemd Services

| Service | Binary | Default Metrics Port | Managed By | Description |
|---------|--------|---------------------|------------|-------------|
| `ztna-intermediate` | `intermediate-server` | 9090 | Ansible | QUIC relay + registry |
| `ztna-connector` | `app-connector` | 9091 | Ansible | echo-service connector |
| `echo-server` | Python | N/A | Ansible | TCP echo test service |
| `ztna-connector-web` | `app-connector` | 9092 (manual) | Manual | web-app connector (not in Ansible) |
| `http-server` | HTTP backend | N/A | Manual | Web test service (not in Ansible) |

**Note:** `ztna-connector-web` and `http-server` are manually configured on the current EC2 instance.
Port 9092 is not a code default — it requires explicit `--metrics-port 9092` when running a second connector.

### AWS Build & Deploy

Uses `$ZTNA_SSH`, `$ZTNA_SSH_KEY`, `$ZTNA_SSH_HOST` from demo-runbook.md Configuration section.

```bash
# Sync source
rsync -avz -e "ssh -i $ZTNA_SSH_KEY" \
  --exclude target/ --exclude .git/ --exclude ios-macos/ \
  . $ZTNA_SSH_HOST:/home/ubuntu/ztna-agent/

# Build (source ~/.cargo/env REQUIRED for non-login shells)
$ZTNA_SSH 'source ~/.cargo/env && cd /home/ubuntu/ztna-agent && \
   cargo build --release --manifest-path intermediate-server/Cargo.toml && \
   cargo build --release --manifest-path app-connector/Cargo.toml'

# Restart (order matters: intermediate first, then connectors)
$ZTNA_SSH 'sudo systemctl restart ztna-intermediate && sleep 2 && \
   sudo systemctl restart ztna-connector && sleep 1 && \
   sudo systemctl restart ztna-connector-web'
```

**Key deployment notes:**
- Self-signed certs require `--no-verify-peer` on connectors (Task 007 default changed to verify)
- Multiple connectors on same host need different `--metrics-port` values
- Intermediate metrics bind to `--bind` address (not 0.0.0.0), use that IP for curl

### CLI Reference

**Intermediate Server** (`intermediate-server`):

| Flag | Default | Added | Description |
|------|---------|-------|-------------|
| `--port` | `4433` | MVP | Listen port (also positional arg #1) |
| `--cert` | `certs/cert.pem` | MVP | TLS certificate path (positional #2) |
| `--key` | `certs/key.pem` | MVP | TLS private key path (positional #3) |
| `--bind` | `0.0.0.0` | MVP | Bind address (positional #4) |
| `--external-ip` | none | Task 006 | Public IP for QAD |
| `--config` | auto-detect | MVP | Config file path |
| `--ca-cert` | none | Task 007 | CA cert for peer verification |
| `--no-verify-peer` | verify on | Task 007 | Disable TLS verification (dev only) |
| `--require-client-cert` | off | Task 007 | Require mTLS client certs |
| `--disable-retry` | retry on | Task 007 | Disable stateless retry tokens |
| `--metrics-port` | `9090` | Task 008 | Metrics/health HTTP port (0=disabled) |

**App Connector** (`app-connector`):

| Flag | Default | Added | Description |
|------|---------|-------|-------------|
| `--server` | `127.0.0.1:4433` | MVP | Intermediate address |
| `--service` | `default` | MVP | Service ID to register |
| `--forward` | `127.0.0.1:8080` | MVP | Local backend address |
| `--config` | auto-detect | MVP | Config file path |
| `--p2p-cert` | none | Task 005 | TLS cert for P2P server |
| `--p2p-key` | none | Task 005 | TLS key for P2P server |
| `--p2p-listen-port` | `4434` | Task 005 | P2P listen port |
| `--external-ip` | none | Task 006 | Public IP for P2P candidates |
| `--service-ip` | none | Task 007 | Virtual IP for TCP validation |
| `--ca-cert` | none | Task 007 | CA cert for peer verification |
| `--no-verify-peer` | verify on | Task 007 | Disable TLS verification (dev only) |
| `--metrics-port` | `9091` | Task 008 | Metrics/health HTTP port (0=disabled) |

### Task References

- [Task 006: Cloud Deployment](../done/006-cloud-deployment/) — Initial AWS setup, QAD
- [Task 007: Security Hardening](../done/007-security-hardening/) — mTLS, retry, CID rotation
- [Task 008: Production Operations](../done/008-production-operations/) — Metrics, reconnect, CI/CD

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
- 214+ tests (153 unit + 61+ E2E)
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
| ~~[007](../done/007-security-hardening/)~~ | ~~Security Hardening~~ | ~~Done~~ | ~~26 findings + 6 deferred items: mTLS, cert renewal, non-blocking TCP, retry tokens, reg ACK, CID rotation~~ | ~~None~~ |
| ~~[008](../done/008-production-operations/)~~ | ~~Production Operations~~ | ~~Done~~ | ~~Prometheus metrics, graceful shutdown, auto-reconnection, deployment automation (Terraform/Ansible/Docker), CI/CD (test+release). +Oracle: UDP injection fix, buffer reuse~~ | ~~007 ✅~~ |
| [009](../009-multi-service-architecture/) | Multi-Service Architecture | P2 | Per-service backend routing, dynamic discovery, health checks. **+Oracle:** signaling hijack, cross-tenant routing fixes | None |
| [010](../010-admin-dashboard/) | Admin Dashboard | P3 | REST API on Intermediate, web frontend, topology visualization. **Note:** MVP admin panel now in Task 016; Task 010 extends it | 008, 009, 016 |
| [011](../011-protocol-improvements/) | Protocol Improvements | P3 | IPv6 QAD, TCP flow control, separate P2P/relay sockets, QUIC migration, 0-RTT. **+Oracle:** IPv6 panic, predictable IDs, endian bug, DATAGRAM size | None |
| [012](../012-multi-environment-testing/) | Multi-Environment Testing | P3 | DigitalOcean, multi-region, symmetric NAT/CGNAT, load testing | None |
| ~~[013](../done/013-swift-modernization/)~~ | ~~Swift Modernization~~ | ~~Done~~ | ~~Swift 6, strict concurrency, deployment target 26.2, linting infra~~ | ~~None~~ |
| ~~[014](../done/014-pr-comment-graphql-hardening/)~~ | ~~PR Comment GraphQL Hardening~~ | ~~Done~~ | ~~GraphQL retry/backoff, pagination, smoke-test for resolve-pr-comments.sh~~ | ~~None~~ |
| ~~[015](../done/015-oracle-quick-fixes/)~~ | ~~Oracle Quick Fixes~~ | ~~Done~~ | ~~IPv6 QAD panic, predictable P2P IDs, legacy FFI removal, UDP length sanity — 4 findings fixed, 146 tests pass~~ | ~~None~~ |
| [016](../016-infrastructure-architecture/) | Infrastructure Architecture | P2 | Separate components onto independent AWS infra, Docker on EC2 (host net) for Intermediate, ECS/Fargate for Connector/Admin/backends, admin panel, Terraform IaC | 007 ✅ |
