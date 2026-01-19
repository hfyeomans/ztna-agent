# Task State: App Connector

**Task ID:** 003-app-connector
**Status:** Not Started
**Branch:** `feature/003-app-connector`
**Last Updated:** 2026-01-18

---

## Overview

Build the App Connector - a QUIC client that connects to the Intermediate System, receives encapsulated IP packets, and forwards them to local applications.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Not Started

### Prerequisites
- ✅ Task 001 complete (Agent QUIC client)
- 🔲 Task 002 complete (Intermediate Server)
- 🔲 Create feature branch

### What's Done
- Nothing yet

### What's Next
1. Wait for Task 002 (Intermediate Server) to complete
2. Create feature branch: `git checkout -b feature/003-app-connector`
3. Create Rust crate: `app-connector/`
4. Implement QUIC client connecting to Intermediate
5. Implement DATAGRAM decapsulation and forwarding

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | Reference for QUIC client code |
| Task 002 (Intermediate) | 🔲 In Progress | Must connect to Intermediate |
| quiche library | ✅ Available | Already used in Agent |
| tokio runtime | 🔲 Add | For async networking |

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Forwarding Method | Raw socket / TUN | TBD based on requirements |
| Service Discovery | CLI config | Simple for MVP |
| Protocol Support | TCP + UDP | Cover common use cases |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/003-app-connector`
5. Continue with next unchecked item in `todo.md`
