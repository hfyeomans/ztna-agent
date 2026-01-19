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
| [002](../002-intermediate-server/) | Intermediate Server | 🔲 Not Started | `feature/002-intermediate-server` |
| [003](../003-app-connector/) | App Connector | 🔲 Not Started | `feature/003-app-connector` |
| [004](../004-e2e-relay-testing/) | E2E Relay Testing | 🔲 Not Started | `feature/004-e2e-relay-testing` |
| [005](../005-p2p-hole-punching/) | P2P Hole Punching | 🔲 Not Started | `feature/005-p2p-hole-punching` |

### Task Dependencies

```
001 (Agent Client) ✅
         │
         ▼
002 (Intermediate Server) ──────┐
         │                      │
         ▼                      ▼
003 (App Connector) ◄───── 004 (E2E Testing)
         │                      │
         └──────────┬───────────┘
                    ▼
         005 (P2P Hole Punching)
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
| Server/Connector | Rust + tokio | Async I/O |
| Packet Encapsulation | QUIC DATAGRAM | RFC 9221 |
| Address Discovery | QAD | Replaces STUN |

---

## Key Files

| Component | Path |
|-----------|------|
| Architecture Doc | `docs/architecture.md` |
| Agent Extension | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| Rust QUIC Client | `core/packet_processor/src/lib.rs` |
| Bridging Header | `ios-macos/Shared/PacketProcessor-Bridging-Header.h` |

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

```bash
# Rust (core/packet_processor)
cargo build --release
cargo test

# Swift/Xcode (ios-macos/ZtnaAgent)
xcodebuild -scheme ZtnaAgent -configuration Release build

# Run app
open /tmp/ZtnaAgent-build/Release/ZtnaAgent.app

# View logs
log stream --predicate 'subsystem CONTAINS "ztna"'
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
