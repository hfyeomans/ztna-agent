# State: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** feature/008-production-operations
**Last Updated:** 2026-02-27

---

## Purpose

Track the current state of production operations implementation including monitoring, automation, and reliability improvements.

---

## Current State

In progress. Branch `feature/008-production-operations` created from master (c33a739).

### Active Work (2026-02-27)
- Phase 1 (Connector Auto-Reconnection): **COMPLETE** — reconnection loop + exponential backoff + SIGTERM
- Phase 3 (Graceful Shutdown): **COMPLETE** — drain_and_shutdown() + SIGTERM/SIGINT for Intermediate + Connector
- Oracle Finding 7 (UDP Injection): **COMPLETE** — source IP validation in process_local_socket()
- Oracle Finding 14 (Recv Buffer): **COMPLETE** — recv_buf reuse, eliminated per-poll 65KB allocation
- Phase 2 (Monitoring): **In Progress** — Prometheus metrics + health checks
- Phase 4 (Deployment Automation): Pending
- Phase 5 (CI/CD): Pending

### Test Results (2026-02-27)
- app-connector: 19/19 unit tests pass, clippy clean
- intermediate-server: 40/40 unit tests pass, clippy clean
- packet_processor: 84/84 unit tests pass
- Integration tests: 1 PermissionDenied (sandbox, pre-existing)

### What Exists (from MVP)
- Manual systemd service files on AWS EC2
- Manual binary builds and SCP deployment
- Connector relies on systemd restarts for reconnection (30s idle timeout)
- No metrics or monitoring
- GitHub Actions CI: lint only (clippy, rustfmt, SwiftLint, ShellCheck)
- Manual k8s deployment via kustomize
- Docker NAT simulation setup exists
- GHCR image registry configured in `deploy/k8s/build-push.sh`

### What This Task Delivers
- Connector auto-reconnection (no systemd restart needed)
- Prometheus metrics for all server components
- Graceful shutdown with connection draining
- Terraform/Ansible deployment automation
- CI/CD pipeline with automated testing

---

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-27 | Use `signal-hook` for SIGTERM in both Connector and Intermediate | Already used for SIGHUP in Intermediate; consistent pattern |
| 2026-02-27 | Exponential backoff 1s→30s for reconnection | Matches Agent pattern; prevents thundering herd |
| 2026-02-27 | 3-second drain period for graceful shutdown | Short enough for dev; long enough for close frames |
| 2026-02-27 | Source IP validation (not port) for UDP injection fix | Backends use ephemeral ports; IP is stable identifier |
