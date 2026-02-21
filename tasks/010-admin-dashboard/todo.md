# TODO: Admin Dashboard

**Task ID:** 010-admin-dashboard
**Status:** Not Started
**Priority:** P3
**Depends On:** 008-production-operations, 009-multi-service-architecture
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation tasks for the web-based admin dashboard.

---

## Phase 1: REST API

- [ ] Add HTTP listener to Intermediate Server (e.g., port 8443)
- [ ] Implement GET /api/connections (list active connections)
- [ ] Implement GET /api/services (list registered services)
- [ ] Implement GET /api/stats (connection metrics)
- [ ] Implement DELETE /api/connections/:id (force disconnect)
- [ ] Add admin authentication (API key)

## Phase 2: Dashboard Frontend

- [ ] Choose frontend framework
- [ ] Create dashboard layout (sidebar + main content)
- [ ] Active connections table with auto-refresh
- [ ] Service management page (list, add, remove)
- [ ] Connection detail view (latency, throughput, path type)

## Phase 3: Topology Visualization

- [ ] Agent → Intermediate → Connector graph
- [ ] P2P vs relay path indicators per connection
- [ ] Health status color coding
- [ ] Auto-layout for multiple services

## Phase 4: Configuration Management

- [ ] Service configuration editor (JSON form)
- [ ] Agent policy editor
- [ ] Configuration change audit log
