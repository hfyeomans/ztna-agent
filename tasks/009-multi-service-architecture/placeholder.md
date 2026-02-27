# Placeholder: Multi-Service Architecture

**Task ID:** 009-multi-service-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Document intentional placeholder/scaffolding code related to multi-service architecture that exists in the codebase from the MVP implementation.

---

## Known Placeholders

| File | Line | Description | Status | Action |
|------|------|-------------|--------|--------|
| `app-connector/src/main.rs` | — | Single `--forward` address per Connector instance | Active | Add per-service routing table |
| `intermediate-server/src/registry.rs` | — | No health check or liveness monitoring | Active | Add health status tracking |
| `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` | — | Service list hardcoded in providerConfiguration | Active | Dynamic service list from server |
| `deploy/config/agent.json` | — | Manual virtual IP assignment per service | Active | Auto-allocation from pool |
| `intermediate-server/src/main.rs` | 185 | `--require-client-cert` not default; SAN-less certs allowed for backward compat (Oracle Finding 2) | Active | Enforce mTLS in production mode |
| `intermediate-server/src/signaling.rs` | 291 | `CandidateAnswer` accepted from any conn with matching session_id — no ownership check (Oracle Finding 3) | Active | Bind sessions to conn IDs, validate sender |
| `app-connector/src/main.rs` | 1992-1995 | "First flow wins" return-path routing — cross-tenant data leakage risk (Oracle Finding 5) | Active | Per-agent flow isolation with 4-tuple flow table |
