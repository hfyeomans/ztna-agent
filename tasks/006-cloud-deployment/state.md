# Task State: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Status:** In Progress
**Branch:** `feature/006-cloud-deployment`
**Last Updated:** 2026-01-25

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

## Current Phase: Phase 1 - Pi k8s Deployment ✅ COMPLETE

### Prerequisites
- [x] Task 004 complete (E2E Relay Testing - local validation) ✅
- [x] Task 005 complete (P2P Hole Punching - protocol implementation) ✅
- [x] Task 005a complete (Swift Agent Integration) ✅
- [x] Phase 0: Docker NAT simulation validated ✅
- [x] Pi k8s cluster access confirmed ✅

### What's Done
- Task planning documentation created
- P2P validation phase added (Phase 7) with NAT testing matrix
- Task 004 merged to master
- Task 005 merged to master (P2P protocol complete - 79 tests)
- Task 005a merged to master (Swift Agent integration)
- Feature branch created: `feature/006-cloud-deployment`
- Research updated with NAT testing requirements
- Cloud provider analysis completed (Vultr/DigitalOcean recommended)
- **P2P fixed port (4434) implemented in app-connector** ✅
- **Phase 0: Docker NAT Simulation completed** ✅
  - Created `deploy/docker-nat-sim/` environment
  - NAT gateway containers with iptables MASQUERADE
  - End-to-end relay test successful
  - Agent observed at 172.20.0.2 (NATted), Connector at 172.20.0.3 (NATted)
  - UDP tunnel echo working through relay
- **Phase 1: Pi k8s Deployment completed** ✅
  - Created `deploy/k8s/` Kustomize structure (base + overlays)
  - Built multi-arch images (arm64) and pushed to Docker Hub
  - Configured Cilium L2 announcements for LoadBalancer
  - Deployed intermediate-server, app-connector, echo-server to Pi cluster
  - **All components verified working:**
    - intermediate-server: Running, accepting QUIC connections
    - app-connector: Running, registers for 'echo-service', receives QAD (30s idle timeout expected)
    - echo-server: Running
    - LoadBalancer: 10.0.150.205:4433/UDP accessible from macOS
  - **macOS → k8s QUIC connection verified:**
    - Local app-connector connects to k8s intermediate-server
    - Successfully registers as connector
    - QAD returns observed address
  - Created comprehensive skill documentation (`deploy/k8s/k8s-deploy-skill.md`)

### Phase 1.1: macOS E2E Test ✅ COMPLETE

**Test Date:** 2026-01-25

**Test Setup:**
- macOS ZtnaAgent app (built from /tmp/ZtnaAgent-build)
- Extension pointing to k8s LoadBalancer: 10.0.150.205:4433
- VPN profile "ZTNA Agent" configured and connected

**Test Results:**
```
✅ macOS VPN tunnel established (utun6, 100.64.0.1)
✅ Routes configured (1.1.1.1/32 → utun6)
✅ QUIC connection to intermediate-server (via Cilium L2 LoadBalancer)
✅ QAD received (observed: 10.0.0.22:55625 - SNAT'd k8s node IP)
✅ Packets intercepted and tunneled through QUIC DATAGRAM
✅ Intermediate server received 84-byte relay data (ICMP ping)
⚠️  "No destination for relay" - expected (no service ID routing for 1.1.1.1)
```

**Key Findings:**
1. **VPN on macOS 26+ works** - No system dialog popup needed; shows "Connected" in Settings
2. **SNAT with externalTrafficPolicy: Cluster** - macOS appears as 10.0.0.22 (k8s node IP) to intermediate
3. **30-second idle timeout** - QUIC connection closes after 30s without traffic
4. **Service-based routing** - MVP routes by service ID, not destination IP
5. **84-byte payload** - ICMP ping (56 data + 8 ICMP header + 20 IP header overhead)

**Next Steps (Full E2E with echo-service):**
- Configure macOS agent to send traffic to echo-service destination
- Or modify routing to forward packets to registered app-connector

### Phase 1.2: Agent Registration ✅ COMPLETE

**Date:** 2026-01-25

**Implementation:**
- Added `agent_register(agent, service_id)` FFI function to Rust core
- Registration sends DATAGRAM: `[0x10, len, service_id_bytes]`
- Updated bridging header with `agent_register` declaration
- Swift calls registration after QUIC connection established
- Service ID hardcoded to "echo-service" (matches k8s app-connector)
- 81 tests passing in packet_processor crate

**Files Changed:**
- `core/packet_processor/src/lib.rs` - Added REG_TYPE_AGENT, Agent::register(), agent_register FFI
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - Added agent_register declaration
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Calls registerForService() after connect

### Phase 1.3: Agent Registration Verification ✅ COMPLETE

**Date:** 2026-01-25

**Test Results:**
```
k8s intermediate-server logs:
[2026-01-25T20:27:45Z INFO] Registration: Agent for service 'echo-service' (conn=8faf58ba...)
[2026-01-25T20:27:45Z INFO] Registering Agent targeting service 'echo-service'
```

**Verified:**
- ✅ Agent connects to k8s intermediate-server (10.0.150.205:4433)
- ✅ Agent receives QAD (observed: 10.0.0.22 - SNAT'd k8s node IP)
- ✅ Agent sends registration DATAGRAM with service ID 'echo-service'
- ✅ Intermediate-server recognizes Agent registration

**Discovered Gap:**
- ⚠️ Full E2E relay blocked: Tunneled IP packets (DATAGRAM) lack service context
- Current `send_datagram(ip_packet)` sends raw IP, but intermediate needs service ID for routing
- Requires: Prepend service ID to DATAGRAM or use single-service-per-connection model

### What's Next
1. ~~Test macOS ZtnaAgent app connecting to Pi k8s intermediate-server~~ ✅
2. ~~Implement Agent registration for relay routing~~ ✅
3. ~~Verify registration appears in k8s logs~~ ✅
4. **BLOCKED:** Implement service-aware relay routing (see Phase 5a.3)
5. Deploy to DigitalOcean or AWS for public IP testing
6. Test P2P hole punching with home NAT → cloud setup

---

## Decisions Made

| Question | Decision | Notes |
|----------|----------|-------|
| AWS VPC | Create NEW VPC | Dedicated ZTNA testing environment |
| P2P Listen Port | Fixed port **4434** | Predictable firewall rules |
| TLS Certificates | Self-signed (MVP) | Domain + Let's Encrypt for later |
| Home k8s Cluster | Pi cluster available | 10.0.150.101-108 |

---

## Home MVP Infrastructure

**Raspberry Pi Kubernetes Cluster:**

| Role | IP Address | Notes |
|------|------------|-------|
| Control Plane | 10.0.150.101 | kubectl access confirmed |
| Worker 1 | 10.0.150.102 | |
| Worker 2 | 10.0.150.103 | |
| Worker 3 | 10.0.150.104 | |
| Worker 4 | 10.0.150.105 | |
| Worker 5 | 10.0.150.106 | |
| Worker 6 | 10.0.150.107 | |
| Worker 7 | 10.0.150.108 | |

**Purpose:** True NAT-to-NAT hole punching validation (both Agent and Connector behind home NAT)

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 004 (E2E Testing) | ✅ Complete | Local testing passed (61+ tests) |
| Task 005 (P2P Protocol) | ✅ Complete | Protocol implementation (79 tests) |
| Task 005a (Swift Integration) | ✅ Complete | macOS Agent wired up |
| AWS CLI | ✅ Configured | Access confirmed |
| DigitalOcean API | ✅ Available | API key ready |
| Pi k8s Cluster | ✅ Available | kubectl access confirmed |

---

## Deployment Components

### Pi k8s Cluster (Phase 1 - Complete)

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Pi k8s (LoadBalancer 10.0.150.205:4433) | ✅ Running |
| App Connector | Pi k8s (ClusterIP, registers for 'echo-service') | ✅ Running |
| Echo Server | Pi k8s (ClusterIP, test service) | ✅ Running |
| TLS Certificates | Self-signed (manual secret creation) | ✅ Done |
| Cilium L2 | LoadBalancer IP announcements | ✅ Working |

**Key Configuration:**
- LoadBalancer: `externalTrafficPolicy: Cluster` (required for Cilium L2)
- Docker Hub images: `hyeomans/ztna-{intermediate-server,app-connector,echo-server}:latest`
- Skill guide: `deploy/k8s/k8s-deploy-skill.md`

### Cloud VM (Phase 2 - Pending)

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
