# Research: Infrastructure Architecture

**Description:** Research the deployment architecture for separating ZTNA components into independent, production-ready systems on AWS.

**Purpose:** The current MVP deployment collocates all components (intermediate-server, app-connector, echo-server) on a single t3.micro EC2 instance. This was appropriate for initial development but is architecturally wrong for production — each component has a different security posture, scaling profile, and network position. This research informs the plan for proper separation.

---

## Current State (MVP — Task 006)

All components on one EC2 (3.128.36.92, t3.micro, us-east-2):

```
┌─────────────────────────────────────────┐
│  Single EC2 (t3.micro)                  │
│                                         │
│  intermediate-server :4433 (systemd)    │
│  app-connector → localhost:4433         │
│  echo-server :8080 (systemd)            │
│  http-server :80 (systemd)              │
└─────────────────────────────────────────┘
```

**Problems:**
- Single point of failure — one instance crash takes down everything
- No isolation between public-facing relay and internal connector
- Can't scale components independently
- App Connector shares a security boundary with the Intermediate Server
- No admin/policy plane

---

## Intermediate Server — What It Is (and Isn't)

The Intermediate Server combines two roles:

1. **QAD (QUIC Address Discovery)** — Tells agents their observed public IP:port. This is analogous to STUN but is NOT STUN — it uses native QUIC DATAGRAMs (`QAD_OBSERVED_ADDRESS = 0x20`, 7-byte IPv4 format) over the same QUIC connection used for data.

2. **QUIC Relay** — Forwards QUIC DATAGRAMs between agents and connectors using 0x2F service-routed framing. This is analogous to TURN but is NOT TURN — there is no TURN allocation/permission model. Routing is based on service registration (0x10/0x11) and mTLS identity.

**Key difference from STUN/TURN:** The Intermediate Server uses the same QUIC connection for signaling, address discovery, AND data relay. STUN/TURN uses separate UDP flows with different protocols. The Intermediate Server is a single, purpose-built QUIC server.

**Future note:** The Intermediate Server will eventually need multi-tenancy support (per-tenant isolation, resource quotas, tenant-aware routing). This is NOT in scope for initial architecture work — build a solid single-tenant architecture first, then layer multi-tenancy on top.

---

## Target Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        AWS (us-east-2)                                    │
│                                                                           │
│  ┌─────────────────────────────────┐   ┌──────────────────────────────┐  │
│  │  Dedicated EC2 (Docker,         │   │  ECS/Fargate                 │  │
│  │  --net=host)                    │   │  Admin Panel                 │  │
│  │  Intermediate Server            │   │  - Policy management         │  │
│  │  - QAD (address discovery)      │   │  - Push policy to components │  │
│  │  - QUIC relay (fallback)        │   │  - Monitoring dashboard      │  │
│  │  - Signaling (P2P)              │   │                              │  │
│  │  - mTLS + service routing       │   │                              │  │
│  │  - Elastic IP (direct)          │   │  :443 HTTPS (ALB)            │  │
│  │  :4433 UDP                      │   └──────────────────────────────┘  │
│  │                                 │                                      │
│  │  NOTE: Multi-tenancy planned    │                                      │
│  │  for future, not this task.     │                                      │
│  └──────────────▲──────────────────┘                                      │
│                 │                                                         │
│                 │ QUIC                                                    │
│                 │                                                         │
│  ┌──────────────┴──────────────────┐   ┌──────────────────────────────┐  │
│  │  ECS/Fargate                    │   │  ECS/Fargate                 │  │
│  │  App Connector                  │   │  Test Backend                │  │
│  │  - QUIC client to Intermediate  │   │  - Echo server (validation)  │  │
│  │  - Decapsulates + forwards      │   │  - HTTP smoke target         │  │
│  │  - Topology-aware placement     │   │  - QuakeJS (demo layer)      │  │
│  └─────────────────────────────────┘   └──────────────────────────────┘  │
│                                                                           │
│  ┌─────────────────────────────────┐                                      │
│  │  Pi k8s / On-Prem (optional)    │                                      │
│  │  App Connector (edge/LAN)       │                                      │
│  │  - Near local backend services  │                                      │
│  └─────────────────────────────────┘                                      │
└──────────────────────────────────────────────────────────────────────────┘

         ▲
         │ QUIC (:4433 UDP)
         │
┌────────┴────────┐
│  macOS Agent    │
│  (local Mac)    │
│  ZTNA Client    │
└─────────────────┘
```

---

## Research Areas

### Intermediate Server: Docker vs Bare EC2

**Docker (ECS/Fargate) advantages:**
- Consistent builds, reproducible deployments
- Easy horizontal scaling (if needed later for multi-tenancy)
- AWS-managed infrastructure (Fargate = no EC2 management)
- Better for CI/CD pipeline integration

**Docker concerns:**
- QUIC uses UDP — Fargate supports UDP via NLB but adds latency
- Need stable public IP — NLB with Elastic IP works but adds complexity
- Container networking may impact quiche's sans-IO UDP performance
- mio event loop is tuned for direct socket access

**Bare EC2 advantages:**
- Direct socket access, no container networking overhead
- Simple Elastic IP attachment
- Known-working systemd model
- Lower latency for QUIC UDP

**Bare EC2 concerns:**
- Manual instance management
- Harder to automate deployments
- Single instance = single point of failure

### Oracle Evaluation (gpt-5.3-codex, xhigh — 2026-02-27)

**Recommendation: Option C — Docker on dedicated EC2 with host networking (`--net=host`)**

Ranking for current phase (single-tenant, performance-sensitive):
1. **Option C** (Docker on EC2, host networking) — best balance of CI/CD benefits + bare-socket performance
2. **Option B** (bare binary on EC2) — best raw simplicity/perf, weakest automation
3. **Option A** (ECS/Fargate) — most operationally managed, most network complexity for this server

**Key reasoning:**
- Server is stateful-in-process (`clients`, `registry`, `session_manager` — all in-memory, single event loop)
- Retry token crypto key generated at process start, kept in-memory — cross-target shifts break sessions
- Docker `--net=host` removes Docker NAT/userland-proxy overhead, closest to bare-socket behavior
- Keeps Elastic IP simplicity without NLB in the QUIC path
- Preserves image-based CI/CD pipeline benefits

**Why NOT Fargate:**
- Fargate only supports `awsvpc` networking (no host mode) — adds container networking overhead
- NLB UDP listeners use flow hash routing with idle timeout; target shifts can break relay/signaling continuity
- NLB QUIC listener exists but has constraints (QUIC v1 behavior/feature limits)
- With in-memory connection model, any mid-session target change breaks everything

**Critical gaps Oracle identified:**
1. Missing scale-out strategy for Intermediate Server state affinity/sharding (future multi-tenancy)
2. Missing retry-token key management for rolling deploys/multi-instance
3. Missing health-check/drain model for UDP service during deploys
4. Admin panel "push policy" underdefined (authz, config versioning, rollback, ack/retry semantics)
5. App Connector placement should be topology-aware (near protected services, not just "ECS everywhere")
6. QAD is IPv4-only (`qad.rs:33`); dual-stack needs protocol evolution (tracked in Task 011)
7. Minimal observability/graceful shutdown should be in-scope for infra cutover (coordinate with Task 008)

**Test backend recommendation:** Deterministic validation stack first (echo + HTTP/WebSocket smoke target); keep QuakeJS as demo-only layer.

**IaC recommendation:** Terraform — module maturity for ECS/NLB/ALB/ECR/IAM/CloudWatch is strong.

---

### App Connector Deployment

- Could run on ECS/Fargate (outbound QUIC to Intermediate)
- Could run on Pi k8s (already has Kustomize manifests)
- Needs network proximity to backend services it's protecting
- Multiple connectors for different service groups

### Admin Panel

- New component — web application for policy management
- REST API + web frontend
- Push configuration to Intermediate Server and Connectors
- Monitor system health, connection status
- Could be ECS/Fargate (standard web app pattern)

### Test Backend Services

- Need something to access through the tunnel for demos/testing
- Options: echo-server (existing), web server (nginx), QuakeJS (impressive demo)
- Should be on separate infrastructure from ZTNA components

### Pi k8s Role

- Currently has Kustomize manifests for intermediate-server + app-connector
- In production architecture: could host an App Connector near local services
- Not suitable for internet-facing Intermediate Server (no public IP, LAN-only)
- Useful for testing multi-site connector deployment

---

## References

- Current AWS deployment: `deploy/aws/aws-deploy-skill.md`
- Current k8s deployment: `deploy/k8s/`
- Docker NAT simulation: `deploy/docker-nat-sim/`
- Intermediate Server source: `intermediate-server/src/main.rs`
- QAD implementation: `intermediate-server/src/qad.rs`
- Architecture docs: `docs/architecture.md`, `docs/architecture_design.md`
