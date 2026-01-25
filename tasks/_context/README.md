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
| [006](../006-cloud-deployment/) | Cloud Deployment | 🔄 In Progress | `feature/006-cloud-deployment` |

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
         005a (Swift Agent Integration) ← Wire up macOS Agent with QUIC FFI
                    │
                    ▼
         006 (Cloud Deployment) ← NAT testing, production prep
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
│  └───────┬───────┘  │     │  - Relay (fallback)  │     │  - Forwards to App  │
│          │          │     │  - Signaling (P2P)   │     │                     │
│  ┌───────▼───────┐  │     └──────────▲───────────┘     └──────────▲──────────┘
│  │ NEPacketTun.  │  │                │                            │
│  │ Provider      │──┼────────────────┴────────────────────────────┘
│  └───────┬───────┘  │           QUIC Tunnel (relay or direct)
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
| Agent | Swift 6.2 + Rust FFI | NetworkExtension framework |
| Intermediate Server | Rust + mio | Event loop matches quiche examples |
| Connector | Rust + mio | Matches Intermediate (mio chosen over tokio) |
| Packet Encapsulation | QUIC DATAGRAM | RFC 9221 |
| Address Discovery | QAD | Replaces STUN |

---

## Key Files

| Component | Path |
|-----------|------|
| Architecture Doc | `docs/architecture.md` |
| Agent Extension | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| Rust QUIC Client | `core/packet_processor/src/lib.rs` |
| P2P Modules | `core/packet_processor/src/p2p/` |
| Bridging Header | `ios-macos/Shared/PacketProcessor-Bridging-Header.h` |
| Intermediate Server | `intermediate-server/src/main.rs` |
| Signaling Module | `intermediate-server/src/signaling.rs` |
| App Connector | `app-connector/src/main.rs` |
| E2E Test Framework | `tests/e2e/README.md` |
| E2E Test Runner | `tests/e2e/run-mvp.sh` |

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

# Run all unit tests (79+ tests)
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
# Full E2E suite (61+ tests)
tests/e2e/run-mvp.sh

# Individual test suites
tests/e2e/scenarios/protocol-validation.sh   # 14 tests
tests/e2e/scenarios/udp-advanced.sh          # 11 tests
tests/e2e/scenarios/reliability-tests.sh     # 11 tests
tests/e2e/scenarios/performance-metrics.sh   # 6 tests
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

---

## Deferred Items / Technical Debt

Items deferred from MVP implementation that must be addressed for production.

### Priority 1: Security (Required for Production)

| Item | Component | Description | Risk if Missing |
|------|-----------|-------------|-----------------|
| **Stateless Retry** | 002-Server | Anti-amplification protection via HMAC tokens | DoS amplification attacks |
| **TLS Certificate Verification** | 001-Agent, 002-Server | Currently `verify_peer(false)` | MITM attacks |
| **Client Authentication** | 002-Server | No auth - any client can connect | Unauthorized access |
| **Rate Limiting** | 002-Server | No per-client DATAGRAM rate limits | Resource exhaustion |

### Priority 2: Reliability (Recommended)

| Item | Component | Description | Impact if Missing |
|------|-----------|-------------|-------------------|
| **Graceful Shutdown** | 002-Server | Connection draining on shutdown | Abrupt disconnects |
| **Connection State Tracking** | 002-Server | Full state machine for connections | Edge case bugs |
| **Error Recovery** | 001-Agent, 002-Server, 003-Connector | Automatic reconnection logic | Manual intervention needed |
| **TCP Support** | 003-Connector | Requires TUN/TAP or TCP state tracking | UDP-only forwarding |
| **Registration Acknowledgment** | 002-Server, 003-Connector | Server doesn't ACK registration | Silent registration failures |

### Priority 3: Operations (Nice to Have)

| Item | Component | Description |
|------|-----------|-------------|
| **Metrics/Stats Endpoint** | 002-Server, 003-Connector | Connection counts, packet rates, latency |
| **Configuration File (TOML)** | 002-Server, 003-Connector | Currently CLI args only |
| **Multiple Bind Addresses** | 002-Server | Only `0.0.0.0:4433` supported |
| **IPv6 QAD Support** | 001-Agent, 002-Server, 003-Connector | Currently IPv4 only (7-byte format) |
| **Production Certificates** | All | Currently using self-signed dev certs |
| **ICMP Support** | 003-Connector | Ping replies for connectivity testing |
| **Multiple Service Registration** | 003-Connector | Currently single service ID only |

### Tracking

When implementing deferred items:
1. Create a task in `tasks/` (e.g., `tasks/006-security-hardening/`)
2. Reference this section in the task's `plan.md`
3. Update this table when complete (change to ✅ and add task reference)

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
│  │  (macOS)    │                │                                          │ │
│  │             │                │  ┌─────────────────────────────────────┐ │ │
│  │  Behind     │◄──── QUIC ────►│  │    Intermediate Server              │ │ │
│  │   NAT       │                │  │    (Public IP: x.x.x.x:4433)        │ │ │
│  │             │                │  │    - QAD (address discovery)        │ │ │
│  └─────────────┘                │  │    - DATAGRAM relay                 │ │ │
│                                  │  └─────────────────────────────────────┘ │ │
│                                  │                    │                      │ │
│                                  │                    │ QUIC                 │ │
│                                  │                    ▼                      │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    App Connector                    │ │ │
│                                  │  │    (Cloud VM or Edge)               │ │ │
│                                  │  │    - UDP forwarding                 │ │ │
│                                  │  │    - Local service access           │ │ │
│                                  │  └───────────────┬─────────────────────┘ │ │
│                                  │                   │                       │ │
│                                  │                   │ Local UDP             │ │
│                                  │                   ▼                       │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    Internal Services                │ │ │
│                                  │  │    (DNS, API, etc.)                 │ │ │
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
