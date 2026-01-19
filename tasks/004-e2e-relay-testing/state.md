# Task State: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Status:** Ready to Start
**Branch:** `feature/004-e2e-relay-testing`
**Last Updated:** 2026-01-19

---

## Overview

Comprehensive end-to-end testing of the relay infrastructure. Validates that traffic flows correctly: Agent → Intermediate → Connector → Local Service and back.

**Important:** App Connector is **UDP-only** (TCP support deferred). All tests must account for this constraint.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Ready to Start

### Prerequisites
- ✅ Task 001 complete (Agent QUIC Client)
- ✅ Task 002 complete (Intermediate Server)
- ✅ Task 003 complete (App Connector - UDP-only)

### What's Done
- Oracle review completed (2026-01-19)
- Plan updated based on Oracle feedback

### What's Next
1. Create feature branch: `git checkout -b feature/004-e2e-relay-testing`
2. Set up local process test environment (not Docker for MVP)
3. Create UDP-focused test scripts
4. Validate protocol invariants (ALPN, registration, datagram size)
5. Run UDP relay tests

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | QUIC client, QAD support |
| Task 002 (Intermediate) | ✅ Complete | QUIC server, DATAGRAM relay |
| Task 003 (Connector) | ✅ Complete | UDP-only forwarding, mio event loop |

---

## Test Categories

| Category | Description | Status |
|----------|-------------|--------|
| Local Relay | All components on localhost | 🔲 |
| NAT Traversal | Intermediate on cloud | 🔲 |
| Latency | Round-trip timing | 🔲 |
| Reliability | Connection recovery | 🔲 |
| Load | Concurrent connections | 🔲 |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/004-e2e-relay-testing`
5. Continue with next unchecked item in `todo.md`
