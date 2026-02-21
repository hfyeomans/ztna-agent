# Plan: Admin Dashboard

**Task ID:** 010-admin-dashboard
**Status:** Not Started
**Priority:** P3
**Depends On:** 008-production-operations, 009-multi-service-architecture
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan the implementation of a web-based admin dashboard for ZTNA management.

---

## Phases (To Be Defined)

### Phase 1: REST API
- Add HTTP listener to Intermediate Server (separate port from QUIC)
- Implement CRUD endpoints for services and connections
- Admin authentication (API key or basic auth)

### Phase 2: Dashboard Frontend
- Single-page application for admin interface
- Active connections view with real-time updates
- Service management (add/remove/configure)
- Connection metrics and latency display

### Phase 3: Topology Visualization
- Network topology diagram
- P2P vs relay path indicators
- Health status per connection

### Phase 4: Configuration Management
- Service configuration editor
- Agent policy management
- Audit log of configuration changes

---

## Success Criteria

- [ ] Admin can view all active connections via web UI
- [ ] Admin can add/remove services without CLI
- [ ] Real-time connection status updates
- [ ] Visual topology of Agent ↔ Connector relationships
