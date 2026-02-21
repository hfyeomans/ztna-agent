# Research: Admin Dashboard

**Task ID:** 010-admin-dashboard
**Status:** Not Started
**Priority:** P3
**Depends On:** 008-production-operations, 009-multi-service-architecture
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research web-based administration dashboard for managing ZTNA services, connections, and configuration.

---

## Research Areas

### REST API on Intermediate Server
- API design for connection management
- Endpoints: list connections, list services, connection stats, force disconnect
- Authentication for admin API (separate from QUIC data plane)
- WebSocket for real-time connection updates

### Web Frontend
- Framework selection (React, Vue, plain HTML/JS)
- Dashboard views: active connections, service topology, metrics
- Real-time log streaming
- Mobile-responsive design

### Configuration Management
- Service CRUD operations via API
- Connector configuration management
- Agent policy management
- Configuration versioning and rollback

### Topology Visualization
- Network topology diagram (Agent → Intermediate → Connector → Backend)
- P2P vs relay path visualization
- Latency overlay on connections
- Connection health indicators

---

## References

- Intermediate Server currently has no HTTP API
- Config mechanism from Task 006: JSON config files
- Metrics from Task 008: Prometheus endpoint (planned)
- Service registry from Task 009: dynamic discovery (planned)
