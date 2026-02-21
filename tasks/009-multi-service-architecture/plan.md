# Plan: Multi-Service Architecture

**Task ID:** 009-multi-service-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan the implementation of per-service backend routing in the Connector, dynamic service discovery, health checks, and virtual IP allocation.

---

## Phases (To Be Defined)

### Phase 1: Per-Service Backend Routing
- Connector config: service_id → backend mapping
- Route incoming packets based on service ID from 0x2F header
- Support different protocols per service (UDP, TCP, ICMP)
- Single Connector replaces multiple Connector instances

### Phase 2: Service Health Checks
- Connector monitors backend health (TCP connect, HTTP, UDP)
- Configurable intervals and failure thresholds
- Unhealthy backends deregistered from Intermediate

### Phase 3: Dynamic Service Discovery
- Config file hot-reload (watch for changes)
- API endpoint for dynamic registration
- DNS-based discovery (optional)

### Phase 4: Virtual IP Allocation
- Automatic IP assignment from 10.100.0.0/24 pool
- Central allocation via Intermediate Server
- DNS integration (service-name → virtual IP)

---

## Success Criteria

- [ ] Single Connector handles multiple backend services
- [ ] Unhealthy backends automatically removed from routing
- [ ] New services can be added without Agent restart
- [ ] Virtual IPs allocated automatically (no manual assignment)
