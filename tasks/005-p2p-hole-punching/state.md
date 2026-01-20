# Task State: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Status:** In Progress - Phase 0 Ready
**Branch:** `feature/005-p2p-hole-punching`
**Last Updated:** 2026-01-20

---

## Overview

Implement direct peer-to-peer connectivity using NAT hole punching. This is the **primary connectivity goal** of the architecture - relay through the Intermediate is only a fallback.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 0 (Socket Architecture)

### Prerequisites ✅ COMPLETE
- [x] Task 002 complete (Intermediate Server with QAD)
- [x] Task 003 complete (App Connector with QAD)
- [x] Task 004 complete (E2E relay testing validated - 61+ tests)
- [x] Feature branch created

### Oracle Review (2026-01-20)

Key findings and recommendations applied to plan.md and todo.md:

1. **P2P vs Path Migration Clarification**
   - P2P = NEW QUIC connection directly to Connector
   - Path Migration = same connection, different network path
   - These are different concepts (plan was conflating them)

2. **Socket Architecture (New Phase 0)**
   - Single socket reuse required for hole punching
   - QAD reflexive address must match P2P socket
   - Added as critical foundation phase

3. **Connector as QUIC Server**
   - Connector must accept incoming connections (currently client-only)
   - Requires TLS certificate for server mode
   - Major architectural change identified

4. **quiche API Corrections**
   - `probe_path()`, `migrate()`, `is_path_validated()` take `SocketAddr` pairs
   - `path_event_next()` for handling PathEvent variants
   - Connection migration only from client side

5. **Local Testing Strategy**
   - Host candidates testable locally
   - Signaling protocol testable locally
   - Direct QUIC connection testable (localhost)
   - Actual NAT hole punching requires Task 006 (Cloud)

---

## What's Done
- [x] Research documented (research.md)
- [x] Initial plan created (plan.md)
- [x] Initial todo created (todo.md)
- [x] Oracle review completed
- [x] Plan updated with Oracle recommendations
- [x] Todo reordered with new Phase 0
- [x] Feature branch created

---

## What's Next

1. **Phase 0: Socket Architecture**
   - Audit current socket usage in Agent and Connector
   - Design single-socket architecture
   - Implement QUIC server mode for Connector
   - Generate TLS certificates for Connector P2P

2. **Phase 1: Candidate Gathering**
   - Create `p2p/` module
   - Implement candidate types and priority calculation
   - Gather host candidates from interfaces

---

## Phase Summary

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 0: Socket Architecture | 🔲 Not Started | **Critical foundation** |
| Phase 1: Candidate Gathering | 🔲 Not Started | |
| Phase 2: Signaling Infrastructure | 🔲 Not Started | |
| Phase 3: Direct Path Establishment | 🔲 Not Started | |
| Phase 4: QUIC Connection & Path Selection | 🔲 Not Started | |
| Phase 5: Resilience | 🔲 Not Started | |
| Phase 6: Testing | 🔲 Not Started | |
| Phase 7: Documentation | 🔲 Not Started | |
| Phase 8: PR & Merge | 🔲 Not Started | |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 002 (Intermediate) | ✅ Complete | QAD provides reflexive addresses |
| Task 003 (App Connector) | ✅ Complete | Needs QUIC server mode added |
| Task 004 (E2E Testing) | ✅ Complete | 61+ tests, relay verified |

---

## Local Testing Constraints

This PoC runs entirely on localhost. Testing limitations:

| Feature | Testable Locally? | Notes |
|---------|-------------------|-------|
| Host candidates | ✅ Yes | Enumerate interfaces |
| Signaling protocol | ✅ Yes | Via Intermediate |
| Direct QUIC connection | ✅ Yes | Agent → Connector localhost |
| Fallback logic | ✅ Yes | Simulate failure |
| **NAT hole punching** | ❌ No | Requires real NAT (Task 006) |
| **Reflexive addresses** | ❌ No | QAD returns 127.0.0.1 locally |
| **NAT type detection** | ❌ No | Requires real NAT |

---

## Key Risks

| Risk | Impact | Status | Mitigation |
|------|--------|--------|------------|
| Connector as QUIC server | High | 🔲 Open | Phase 0: Add server mode |
| Single socket constraint | Medium | 🔲 Open | Phase 0: Socket architecture |
| quiche API differences | Medium | 🔲 Open | Validate during implementation |
| Symmetric NAT | Medium | 🔲 Open | Relay fallback always works |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read `tasks/_context/components.md` for component status
3. Read this file for task state
4. Read `plan.md` for implementation details
5. Check `todo.md` for current progress
6. Ensure on branch: `feature/005-p2p-hole-punching`
7. Start with Phase 0: Socket Architecture
