# Task State: Intermediate Server

**Task ID:** 002-intermediate-server
**Status:** Not Started
**Branch:** `feature/002-intermediate-server`
**Last Updated:** 2026-01-18

---

## Overview

Build the Intermediate System - a QUIC server that relays traffic between Agents and App Connectors, and provides QUIC Address Discovery (QAD).

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Not Started

### Prerequisites
- ✅ Task 001 complete (Agent QUIC client)
- 🔲 Create feature branch
- 🔲 Create `intermediate-server/` crate

### What's Done
- Nothing yet

### What's Next
1. Create feature branch: `git checkout -b feature/002-intermediate-server`
2. Create Rust crate: `intermediate-server/`
3. Implement basic QUIC server
4. Add QAD support
5. Add DATAGRAM relay

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | Agent can connect once server exists |
| quiche library | ✅ Available | Already used in Agent |
| tokio runtime | 🔲 Add | For async server |

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Async Runtime | tokio | Industry standard for Rust async |
| Server Architecture | Single-threaded event loop | Simple, sufficient for MVP |
| Client Registry | In-memory HashMap | No persistence needed for MVP |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/002-intermediate-server`
5. Continue with next unchecked item in `todo.md`
