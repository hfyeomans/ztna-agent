# Task State: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Status:** Not Started
**Branch:** `feature/005-p2p-hole-punching`
**Last Updated:** 2026-01-18

---

## Overview

Implement direct peer-to-peer connectivity using NAT hole punching. This is the **primary connectivity goal** of the architecture - relay through the Intermediate is only a fallback.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Not Started

### Prerequisites
- [ ] Task 002 complete (Intermediate Server with QAD)
- [ ] Task 003 complete (App Connector with QAD)
- [ ] Task 004 complete (E2E relay testing validated)

### What's Done
- Nothing yet

### What's Next
1. Wait for Tasks 002, 003, 004 to complete
2. Create feature branch: `git checkout -b feature/005-p2p-hole-punching`
3. Implement candidate gathering (local, reflexive, relay)
4. Implement hole punching coordination protocol
5. Implement connection migration from relay to direct
6. Test with various NAT types

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 002 (Intermediate) | Not Started | QAD provides reflexive addresses |
| Task 003 (App Connector) | Not Started | Needs hole punching capability |
| Task 004 (E2E Testing) | Not Started | Validates relay works first |

---

## NAT Types to Support

| NAT Type | Hole Punching | Priority |
|----------|---------------|----------|
| Full Cone | Easy | P1 |
| Restricted Cone | Medium | P1 |
| Port Restricted | Medium | P1 |
| Symmetric | Hard (may need TURN) | P2 |

---

## Connection Strategy

```
Priority Order:
1. Direct LAN (same network)
2. Direct WAN (hole punching)
3. Relay (via Intermediate)
```

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/005-p2p-hole-punching`
5. Continue with next unchecked item in `todo.md`
