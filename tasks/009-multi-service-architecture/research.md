# Research: Multi-Service Architecture

**Task ID:** 009-multi-service-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research per-service backend routing, dynamic service discovery, health checks, and virtual IP allocation to support many services behind a single ZTNA deployment.

---

## Research Areas

### Per-Service Backend Routing
- Currently each Connector instance handles one service with one `--forward` address
- Need: single Connector handling multiple services with different backends
- Routing table: service_id → backend_addr:port
- Protocol-aware forwarding (UDP vs TCP vs ICMP per service)

### Dynamic Service Discovery
- Static config (JSON/YAML) for initial implementation
- DNS-based discovery (SRV records)
- API-based registration (Connector announces services)
- Health check integration (remove unhealthy backends)

### Service Health Checks
- Backend health monitoring from Connector
- Health status propagation to Intermediate/Agent
- Automatic deregistration of unhealthy services
- Configurable check intervals and thresholds

### Virtual IP Allocation
- Current: manual 10.100.0.x assignment per service
- Need: automatic IP allocation from pool
- Conflict detection across Agents
- DNS resolution: `service-name.ztna.local` → virtual IP

---

## Oracle Review Findings (Assigned to This Task)

From `oracle-review-01.md`, verified by Codex Oracle (gpt-5.3-codex, xhigh) on 2026-02-26.

### Finding 2 (Critical): Registration Auth — Conditionally Fixed

- **Severity:** Critical (conditionally mitigated)
- **Component:** intermediate-server
- **Location:** `intermediate-server/src/main.rs:185`, `intermediate-server/src/auth.rs:173`
- **Status:** Task 007 added mTLS with SAN-based service authorization, but:
  - Requires `--require-client-cert` flag to enforce (not default)
  - Certificates without ZTNA SANs are allowed for backward compatibility (treated as "authorize all")
- **Oracle assessment:** Disputed "fully fixed" — conditionally fixed only. Production deployments should mandate `--require-client-cert` and reject SAN-less certificates.
- **Action for this task:** When implementing multi-service routing, enforce that production configs require client certs. Consider removing the backward-compat path for SAN-less certificates, or at minimum logging a loud warning. This directly affects service isolation — without mandatory mTLS, any client can register for any service.

### Finding 3 (High): Signaling Session Hijack

- **Severity:** High
- **Component:** intermediate-server
- **Location:** `intermediate-server/src/signaling.rs:291`, `intermediate-server/src/main.rs:1411`
- **Current code:** `CandidateAnswer` is accepted from whichever connection sends a matching `session_id`. No ownership or role check on the sender.
- **Risk:** Any connected client that knows (or guesses) a session_id can inject a `CandidateAnswer`, redirecting P2P hole punching to an attacker-controlled endpoint.
- **Oracle assessment:** Confirmed NOT fixed by Task 007. Initial triage incorrectly marked this as fixed. The session_id is used as a lookup key but there is no validation that the answering connection is the intended connector for that session.
- **Proposed fix:** Bind sessions to connection IDs at creation time. When `CandidateOffer` creates a session, record the agent's `conn_id` AND the specific connector chosen at offer time (`main.rs:1366`). When `CandidateAnswer` arrives, verify the sender's `conn_id` matches the **exact connector** that was selected during offer processing — not just "any connector for the service." The current `SignalingSession` struct (`signaling.rs:291`) does not store the expected connector, so the struct must be extended. Reject answers from any other connection.
- **Relationship to this task:** Multi-service architecture makes this worse — more services means more sessions, increasing the attack surface. Fix must land with or before multi-service routing.

### Finding 5 (High): Cross-Tenant Connector Routing

- **Severity:** High
- **Component:** app-connector
- **Location:** `app-connector/src/main.rs:1992-1995`
- **Current code:** `let flow_key = self.flow_map.keys().next().cloned();` — "first flow wins" return-path routing. Comment: "Find matching flow (any flow for now - simplified MVP)".
- **Risk:** When multiple agents are connected, responses can be routed to the wrong agent, causing cross-tenant data leakage.
- **Oracle assessment:** Confirmed still open. Architectural fix required — needs per-agent flow isolation with proper source IP/port matching.
- **Proposed fix:** Replace "first flow wins" with per-agent flow isolation. Oracle notes that a simple 4-tuple map alone is insufficient: the connector strips UDP headers/payload and uses a single local socket (`main.rs:1276, 1292, 1995`), so the return path cannot be recovered from packet content alone. Options:
  1. **Per-flow NAT/socket strategy:** Allocate a unique local socket per agent flow so return traffic is demuxed by which socket receives it.
  2. **Extra metadata:** Tag outbound traffic to backends with agent-identifying metadata (e.g., unique source port per agent) that the return path can match on.
  3. **Embedded flow ID:** Extend the 0x2F protocol to carry a flow identifier that the connector preserves in its flow table.
  This requires architectural design — not a simple flow table swap. Core requirement for safe multi-service architecture.
- **Relationship to this task:** This is a prerequisite for safe multi-service architecture. Must be fixed in Phase 1 alongside per-service backend routing.

---

## References

- Current architecture: one Connector per service, single `--forward` address
- 0x2F protocol: `[0x2F, id_len, service_id, ip_packet]` already supports multi-service
- MVP services: echo-service (10.100.0.1), web-app (10.100.0.2)
- Deferred from `_context/components.md`: per-service backend routing
- Deferred from `_context/README.md`: dynamic discovery, health checks
- Oracle findings triage: `tasks/015-oracle-quick-fixes/research.md`
