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

## Current Phase: Phase 1 - Project Setup

### Prerequisites
- ✅ Task 001 complete (Agent QUIC client)
- ✅ Create feature branch
- ✅ Plan reviewed by Oracle (Codex)
- ✅ Todo reviewed by Oracle (Codex)
- 🔲 Create `intermediate-server/` crate

### What's Done
- Feature branch created: `feature/002-intermediate-server`
- Plan and todo files reviewed and updated with Oracle feedback
- Research.md updated to fix ALPN and QAD format inconsistencies
- Critical compatibility requirements documented

### What's Next
1. Create Rust crate: `intermediate-server/`
2. Add dependencies (quiche, mio, ring, log)
3. Generate development certificates
4. Implement basic QUIC server with mio event loop

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
| quiche library | ✅ Available | Version 0.22 (same as Agent) |
| mio runtime | 🔲 Add | Matches quiche examples |
| ring crypto | 🔲 Add | For retry token HMAC |

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

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/002-intermediate-server`
5. Continue with next unchecked item in `todo.md`

---

## Oracle Review Summary

**Reviewed:** 2026-01-18 via `codex exec`

**Critical Issues Fixed:**
- ALPN mismatch (`ztna` → `ztna-v1`)
- QAD format (removed IP version byte, DATAGRAM only)
- Runtime choice (tokio → mio)

**Recommendations Applied:**
- Added stateless retry/anti-amplification steps
- Added QUIC header parsing details
- Defined client registration message protocol
- Clarified routing model (connection-based, not packet header)
