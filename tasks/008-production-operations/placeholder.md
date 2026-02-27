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
| `app-connector/src/main.rs` | 1964-1985 | `process_local_socket()` accepts UDP from any local source without validation (Oracle Finding 7) | Active | Add source address validation |
| `app-connector/src/main.rs` | 1965 | Per-poll `vec![0u8; 65535]` allocation instead of reusing `self.recv_buf` (Oracle Finding 14) | Active | Refactor to reuse buffer |
