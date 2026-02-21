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

## References

- Current architecture: one Connector per service, single `--forward` address
- 0x2F protocol: `[0x2F, id_len, service_id, ip_packet]` already supports multi-service
- MVP services: echo-service (10.100.0.1), web-app (10.100.0.2)
- Deferred from `_context/components.md`: per-service backend routing
- Deferred from `_context/README.md`: dynamic discovery, health checks
