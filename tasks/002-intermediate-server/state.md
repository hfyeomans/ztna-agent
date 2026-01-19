# Task State: Intermediate Server

**Task ID:** 002-intermediate-server
**Status:** In Progress
**Branch:** `feature/002-intermediate-server`
**Last Updated:** 2026-01-18

---

## Overview

Build the Intermediate System - a QUIC server that relays traffic between Agents and App Connectors, and provides QUIC Address Discovery (QAD).

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 7 - PR Ready

### Phases Completed
- ✅ Phase 1: Project Setup (crate, dependencies, TLS certs)
- ✅ Phase 2: QUIC Server Core (config, event loop, connection management)
- ✅ Phase 3: QAD Implementation (7-byte format, DATAGRAM delivery)
- ✅ Phase 4: Client Registry (Agent/Connector registration, routing)
- ✅ Phase 5: DATAGRAM Relay (bidirectional forwarding)
- ✅ Phase 6: Integration Testing (handshake + QAD verified)

### What's Done
- Feature branch created: `feature/002-intermediate-server`
- Plan and todo files reviewed and updated with Oracle feedback
- Full server implementation in `intermediate-server/`
  - `main.rs`: mio event loop, QUIC server, packet processing
  - `client.rs`: Client struct, ClientType enum
  - `qad.rs`: build_observed_address() with 7-byte format
  - `registry.rs`: Bidirectional Agent-Connector routing
- All 6 unit tests passing
- Integration test passing:
  - QUIC handshake completes
  - QAD OBSERVED_ADDRESS (7 bytes) received and parsed correctly

### What's Next
1. Create PR to merge into master
2. Full relay testing with Agent + Connector (Task 003)

### Deferred
- Stateless retry (anti-amplification) - Not needed for MVP localhost testing

---

## Critical Compatibility Notes

From Oracle review - these MUST match Agent implementation:

| Parameter | Value | Agent Reference |
|-----------|-------|-----------------|
| **ALPN** | `b"ztna-v1"` | `lib.rs:28` |
| **QAD Format** | `0x01 + IPv4(4 bytes) + port(2 bytes BE)` | `lib.rs:255-262` |
| **QAD Transport** | DATAGRAM only | `lib.rs:251` |
| **Max DATAGRAM** | 1350 bytes | `lib.rs:22` |
| **Idle Timeout** | 30000ms | `lib.rs:25` |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | Agent can connect once server exists |
| quiche library | ✅ Added | Version 0.22 (same as Agent) |
| mio runtime | ✅ Added | Version 0.8 with net/os-poll features |
| ring crypto | ✅ Added | Version 0.17 (for future retry tokens) |

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Async Runtime | **mio** | Matches quiche examples, sans-IO model |
| Server Architecture | Single-threaded event loop | Simple, sufficient for MVP |
| Client Registry | In-memory HashMap | No persistence needed for MVP |
| QAD Format | IPv4 only (7 bytes) | Matches Agent parser |
| Routing Model | Connection-based | Raw IP packets, no routing header |

---

## Commits

| Commit | Description |
|--------|-------------|
| `70d1e1d` | Oracle-reviewed plan and todo with critical fixes |
| `eaf1cfc` | Phase 1-5 implementation complete |
| `78431ae` | Update state.md to reflect Phase 1-5 completion |
| `46a5451` | Add integration test for QUIC handshake and QAD |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Ensure on branch: `feature/002-intermediate-server`
4. Run the server: `cd intermediate-server && cargo run`
5. Test with Agent from Task 001

---

## Oracle Review Summary

**Reviewed:** 2026-01-18 via `codex exec`

**Critical Issues Fixed:**
- ALPN mismatch (`ztna` → `ztna-v1`)
- QAD format (removed IP version byte, DATAGRAM only)
- Runtime choice (tokio → mio)

**Recommendations Applied:**
- Added QUIC header parsing details
- Defined client registration message protocol
- Clarified routing model (connection-based, not packet header)
- Stateless retry deferred (not critical for MVP)
