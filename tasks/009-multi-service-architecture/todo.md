# TODO: Multi-Service Architecture

**Task ID:** 009-multi-service-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation tasks for per-service backend routing, service discovery, health checks, and virtual IP allocation.

---

## Phase 1: Per-Service Backend Routing

- [ ] Design Connector config format for multi-service routing table
- [ ] Implement service_id → backend dispatch in Connector
- [ ] Support mixed protocols per service (UDP, TCP, ICMP)
- [ ] Replace two Connector instances with single multi-service Connector on AWS
- [ ] Test: echo-service and web-app via single Connector
- [ ] Update E2E tests

## Phase 2: Service Health Checks

- [ ] Design health check protocol (TCP connect, HTTP GET, UDP probe)
- [ ] Implement health check loop in Connector
- [ ] Deregister unhealthy services from Intermediate
- [ ] Re-register when backend recovers
- [ ] Configurable check interval and failure threshold

## Phase 3: Dynamic Service Discovery

- [ ] Config file hot-reload (inotify/kqueue watch)
- [ ] API endpoint for dynamic service registration (optional)
- [ ] Agent config refresh when services change

## Phase 4: Virtual IP Allocation

- [ ] Design IP allocation protocol (Intermediate assigns from pool)
- [ ] Implement allocation API on Intermediate
- [ ] Agent receives virtual IP assignments after registration
- [ ] DNS resolution: service-name.ztna.local → virtual IP (optional)

## Oracle Findings (Cross-Cutting — Must Address)

### Finding 2 (Critical): Registration Auth Hardening
- [ ] Decide policy: require `--require-client-cert` for production configs
- [ ] Add loud warning or reject SAN-less certificates in production mode
- [ ] Document that SAN-less backward compat is dev-only
- [ ] Test: connection without valid SAN is rejected when client cert required

### Finding 3 (High): Signaling Session Hijack
- [ ] Extend `SignalingSession` struct to store expected connector `conn_id`
- [ ] Record agent's `conn_id` when `CandidateOffer` creates a session
- [ ] Record the specific connector chosen at offer time (`main.rs:1366`) in session
- [ ] Validate sender's `conn_id` on `CandidateAnswer` against the exact expected connector
- [ ] Reject answers from unauthorized connections with `log::warn!()`
- [ ] Test: `CandidateAnswer` from non-owning connection is rejected

### Finding 5 (High): Cross-Tenant Connector Routing
- [ ] Design per-agent flow isolation strategy (3 options in research.md: per-flow socket, extra metadata, embedded flow ID)
- [ ] Replace "first flow wins" (`flow_map.keys().next()`) with chosen strategy
- [ ] Handle single local socket limitation — connector strips headers so return path needs agent-identifying info
- [ ] Match return traffic to originating agent's connection
- [ ] Test: two agents accessing same service get correct return traffic
- [ ] Test: flow table cleanup on agent disconnection
