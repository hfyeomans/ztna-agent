# Task State: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Status:** Not Started
**Branch:** `feature/006-cloud-deployment`
**Last Updated:** 2026-01-20

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

## Current Phase: Not Started (Ready to Begin)

### Prerequisites
- [x] Task 004 complete (E2E Relay Testing - local validation) ✅
- [ ] Task 005 complete (P2P Hole Punching - protocol implementation)
- [ ] Cloud provider account (AWS/GCP/DigitalOcean/Vultr)
- [ ] Domain name (optional, for TLS certificates)

### What's Done
- Task planning documentation created
- P2P validation phase added (Phase 7) with NAT testing matrix
- Task 004 merged to master

### What's Next
1. Complete Task 005 (P2P protocol implementation - local PoC)
2. Choose cloud provider
3. Create feature branch: `git checkout -b feature/006-cloud-deployment`
4. Set up cloud infrastructure
5. Deploy and configure components
6. Test NAT traversal
7. **Validate P2P hole punching with real NATs**

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 004 (E2E Testing) | ✅ Complete | Local testing passed |
| Task 005 (P2P Protocol) | 🔄 In Progress | Protocol implementation (local PoC) |
| Cloud Account | 🔲 Not Configured | Need provider credentials |

---

## Deployment Components

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Cloud VM (public IP) | 🔲 |
| App Connector | Cloud VM (same or separate) | 🔲 |
| TLS Certificates | Let's Encrypt or self-signed | 🔲 |
| Firewall Rules | UDP 4433 inbound | 🔲 |

---

## P2P Validation Scope (From Task 005)

> Task 005 implements P2P protocol locally. This task validates it with real NATs.

### Testable Only with Cloud Deployment

| Feature | Local Testing | Cloud Testing |
|---------|---------------|---------------|
| NAT hole punching | ❌ Not possible | ✅ Real NAT behavior |
| Reflexive candidates | ❌ Returns 127.0.0.1 | ✅ Actual public IP |
| NAT type detection | ❌ No NAT to detect | ✅ Various NAT types |
| Cross-network latency | ❌ Always localhost | ✅ Real network paths |

### NAT Types to Test

| NAT Type | Hole Punching | Priority |
|----------|---------------|----------|
| Full Cone | Easy | P1 |
| Restricted Cone | Medium | P1 |
| Port Restricted | Medium | P1 |
| Symmetric | Hard (relay fallback) | P2 |

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
