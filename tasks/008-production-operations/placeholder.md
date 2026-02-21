# Placeholder: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Document intentional placeholder/scaffolding code related to production operations that exists in the codebase from the MVP implementation.

---

## Known Placeholders

| File | Line | Description | Status | Action |
|------|------|-------------|--------|--------|
| `app-connector/src/main.rs` | — | No auto-reconnection logic; exits on connection loss | Active | Add reconnection loop with backoff |
| `intermediate-server/src/main.rs` | — | No metrics endpoint | Active | Add Prometheus HTTP exporter |
| `intermediate-server/src/main.rs` | — | No graceful shutdown (hard exit on SIGTERM) | Active | Add connection draining |
| `deploy/` | — | No Terraform or Ansible automation | Active | Create IaC modules |
| `.github/` | — | No CI/CD workflows | Active | Add GitHub Actions |
