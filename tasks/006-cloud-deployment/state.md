# Task State: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Status:** In Progress
**Branch:** `feature/006-cloud-deployment`
**Last Updated:** 2026-01-24

---

## Overview

Deploy Intermediate Server and App Connector to cloud infrastructure for NAT testing and production readiness. Enables testing Agent behavior behind real NAT environments.

**Primary purposes:**
1. Test Agent behavior behind real NAT environments
2. Validate QAD (QUIC Address Discovery) with actual public IPs
3. **Validate P2P hole punching with real NATs** (from Task 005)
4. Prepare infrastructure for production deployment

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 1 - Infrastructure Selection

### Prerequisites
- [x] Task 004 complete (E2E Relay Testing - local validation) ✅
- [x] Task 005 complete (P2P Hole Punching - protocol implementation) ✅
- [x] Task 005a complete (Swift Agent Integration) ✅
- [ ] Cloud provider account (Vultr or DigitalOcean recommended)
- [ ] Domain name (optional, for TLS certificates)

### What's Done
- Task planning documentation created
- P2P validation phase added (Phase 7) with NAT testing matrix
- Task 004 merged to master
- Task 005 merged to master (P2P protocol complete - 79 tests)
- Task 005a merged to master (Swift Agent integration)
- Feature branch created: `feature/006-cloud-deployment`
- Research updated with NAT testing requirements
- Cloud provider analysis completed (Vultr/DigitalOcean recommended)

### What's Next
1. Choose cloud provider (Vultr or DigitalOcean)
2. Provision cloud VM
3. Deploy Intermediate Server + App Connector
4. Test from Agent behind home NAT
5. Validate P2P hole punching with real NATs

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 004 (E2E Testing) | ✅ Complete | Local testing passed (61+ tests) |
| Task 005 (P2P Protocol) | ✅ Complete | Protocol implementation (79 tests) |
| Task 005a (Swift Integration) | ✅ Complete | macOS Agent wired up |
| Cloud Account | 🔲 Not Configured | Vultr or DigitalOcean recommended |

---

## Deployment Components

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Cloud VM (public IP) | 🔲 |
| App Connector | Cloud VM (same or separate) | 🔲 |
| TLS Certificates | Let's Encrypt or self-signed | 🔲 |
| Firewall Rules | UDP 4433 inbound | 🔲 |

---

## Critical Testing Insight

> **IMPORTANT:** Cloud VMs have **direct public IPs** - they are NOT behind NAT.
> To test hole punching, the **Agent must be behind real NAT** (home network, mobile hotspot, etc.)

### What Cloud Deployment Tests

| Test Type | Cloud-Only | Cloud + Home NAT |
|-----------|------------|------------------|
| DATAGRAM relay | ✅ Yes | ✅ Yes |
| QAD public IP discovery | ✅ Yes | ✅ Yes |
| Cross-internet latency | ✅ Yes | ✅ Yes |
| **P2P hole punching** | ❌ No* | ✅ Yes |
| **NAT type behavior** | ❌ No* | ✅ Yes |

*Both cloud peers have direct public IPs - no NAT to punch through.

### Minimum Test Topology for P2P

```
┌─────────────────┐                    ┌─────────────────────────┐
│  Home Network   │                    │     Cloud VM            │
│  (Behind NAT)   │                    │  (Direct Public IP)     │
│                 │                    │                         │
│  ┌───────────┐  │                    │  Intermediate Server    │
│  │   Agent   │──┼──► Home Router ───►│       + Connector       │
│  │  (macOS)  │  │       NAT          │                         │
│  └───────────┘  │                    └─────────────────────────┘
└─────────────────┘
```

---

## P2P Validation Scope (From Task 005)

> Task 005 implements P2P protocol locally. This task validates it with real NATs.

### Testable Only with Cloud Deployment + Real NAT

| Feature | Local Testing | Cloud Testing |
|---------|---------------|---------------|
| NAT hole punching | ❌ Not possible | ✅ Real NAT behavior |
| Reflexive candidates | ❌ Returns 127.0.0.1 | ✅ Actual public IP |
| NAT type detection | ❌ No NAT to detect | ✅ Various NAT types |
| Cross-network latency | ❌ Always localhost | ✅ Real network paths |

### NAT Types to Test

| NAT Type | Hole Punching | Priority | Common Location |
|----------|---------------|----------|-----------------|
| Full Cone | Easy | P1 | Most home routers |
| Restricted Cone | Medium | P1 | Some home routers |
| Port Restricted | Medium | P1 | Some enterprise |
| Symmetric | Hard (relay fallback) | P2 | Carrier-grade, enterprise |

### Key P2P Validation Items

- Address exchange via Intermediate (real NAT)
- Simultaneous UDP open (hole punch timing)
- Direct QUIC connection after hole punch
- Path selection (direct vs relay RTT comparison)
- Fallback to relay when hole punching fails
- Connection priority order (Direct LAN → Direct WAN → Relay)

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/006-cloud-deployment`
5. Continue with next unchecked item in `todo.md`
