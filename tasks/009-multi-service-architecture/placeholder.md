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
