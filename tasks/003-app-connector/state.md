# Task State: App Connector

**Task ID:** 003-app-connector
**Status:** In Progress - Phase 1 Complete
**Branch:** `feature/003-app-connector`
**Last Updated:** 2026-01-18

---

## Overview

Build the App Connector - a QUIC client that connects to the Intermediate System, receives encapsulated IP packets, and forwards them to local applications.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 1 Complete - MVP Implementation

### Prerequisites
- ✅ Task 001 complete (Agent QUIC client)
- ✅ Task 002 complete (Intermediate Server)
- ✅ Create feature branch

### What's Done
- Created feature branch `feature/003-app-connector`
- Reviewed intermediate-server code for compatibility requirements
- Created `app-connector/` Rust crate with mio event loop (not tokio)
- Implemented QUIC client connecting to Intermediate Server
- Implemented registration protocol (0x11 for Connector)
- Implemented QAD message handling (0x01 OBSERVED_ADDRESS)
- Implemented DATAGRAM processing for encapsulated IP packets
- Implemented UDP-only local forwarding (MVP scope)
- Implemented return traffic handling with IP/UDP packet construction
- All 6 unit tests passing
- Integration test passing (handshake + QAD + registration verified)

### What's Next
1. Create PR for review
2. Address any review feedback
3. Merge to master

---

## Capabilities

- Creates QUIC connections via quiche (same library as Agent/Intermediate)
- Sends/receives QUIC DATAGRAMs
- Parses QAD OBSERVED_ADDRESS messages (7-byte IPv4 format)
- Registers as Connector with service ID (format: [0x11][len][service_id])
- Decapsulates IPv4/UDP packets from DATAGRAMs
- Forwards UDP payload to configurable local service
- Constructs return IP/UDP packets for response traffic
- Thread-safe mio event loop

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | Reference for QUIC client code |
| Task 002 (Intermediate) | ✅ Complete | Must connect to Intermediate |
| quiche library | ✅ 0.22 | Same version as Intermediate |
| mio runtime | ✅ 0.8 | Match Intermediate (not tokio) |

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Runtime | mio (not tokio) | Match Intermediate Server; quiche sans-IO model |
| Forwarding Method | UDP-only for MVP | Avoid TCP state complexity; TUN/TAP later if needed |
| Service Discovery | CLI args | Simple for MVP; config file later |
| Protocol Support | UDP only (MVP) | TCP requires TUN/TAP or TCP state tracking |

---

## Critical Compatibility

| Constant | Value | File |
|----------|-------|------|
| ALPN | `b"ztna-v1"` | main.rs:32 |
| MAX_DATAGRAM_SIZE | 1350 | main.rs:26 |
| IDLE_TIMEOUT_MS | 30_000 | main.rs:29 |
| REG_TYPE_CONNECTOR | 0x11 | main.rs:41 |
| QAD_OBSERVED_ADDRESS | 0x01 | main.rs:44 |

---

## Deferred Items (Post-MVP)

| Item | Description | Priority |
|------|-------------|----------|
| TCP support | Requires TUN/TAP or TCP state tracking | Medium |
| ICMP support | Ping replies for connectivity testing | Low |
| Multiple services | Register for multiple service IDs | Medium |
| Reconnection | Automatic reconnect on disconnect | High |
| Config file | TOML configuration instead of CLI args | Low |
| Metrics | Connection stats, packet counts | Low |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/003-app-connector`
5. Continue with next unchecked item in `todo.md`
