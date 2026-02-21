# State: Multi-Service Architecture

**Task ID:** 009-multi-service-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track the current state of multi-service architecture implementation.

---

## Current State

Not started. MVP supports multiple services via separate Connector instances.

### What Exists (from MVP)
- 0x2F service-routed datagram protocol (Intermediate routes by service ID)
- Intermediate registry supports multiple services per Agent connection
- Each Connector registers for one service with one `--forward` address
- Two Connector instances on AWS: echo-service (port 4434) and web-app (port 4435)
- Agent config includes service→virtual IP mapping in providerConfiguration
- Manual virtual IP assignment (10.100.0.1, 10.100.0.2)

### What This Task Delivers
- Single Connector with per-service backend routing table
- Backend health checks with automatic deregistration
- Dynamic service discovery (config hot-reload or API)
- Automatic virtual IP allocation from pool

---

## Decisions Log

(No decisions yet — task not started)
