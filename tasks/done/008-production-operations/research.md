# Research: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research monitoring, observability, graceful shutdown, deployment automation, and CI/CD pipelines needed to run ZTNA in production.

---

## Research Areas

### Monitoring & Observability
- Prometheus metrics exporter for Intermediate Server
- Key metrics: active connections, relay throughput, P2P vs relay ratio
- Grafana dashboards for ZTNA health
- Alerting on connection drops, high latency, service unavailability

### Graceful Shutdown
- QUIC connection draining on Intermediate restart
- Connector deregistration before shutdown
- Agent notification of service unavailability
- Zero-downtime deployment strategy

### Connector Auto-Reconnection
- Currently Connector exits on idle timeout (30s) or connection loss
- Need: automatic reconnection with backoff (similar to macOS Agent)
- Systemd restart works but is suboptimal (loses registration state)

### Deployment Automation
- Terraform for AWS infrastructure (EC2, security groups, Elastic IP)
- Ansible for service configuration and binary deployment
- Blue/green deployment for Intermediate Server upgrades

### CI/CD
- GitHub Actions for Rust builds (linux/amd64, linux/arm64)
- Automated E2E test suite in CI
- Container image builds and push
- Release binary distribution

---

## Oracle Review Findings (Assigned to This Task)

From `oracle-review-01.md`, verified by Codex Oracle (gpt-5.3-codex, xhigh) on 2026-02-26.

### Finding 7 (High): Local UDP Injection

- **Severity:** High
- **Component:** app-connector
- **Location:** `app-connector/src/main.rs:1964-1985` (`process_local_socket`)
- **Current code:** `self.local_socket.recv_from(&mut buf)` accepts UDP from ANY local process and calls `send_return_traffic()` without validating the source address.
- **Risk:** Any local process can inject traffic into the QUIC tunnel. Local privilege escalation enables tunnel injection.
- **Oracle assessment:** Confirmed still open. Source address validation is a medium-complexity fix — needs design decision on what constitutes valid sources (only `forward_addr`? configurable allowlist?).
- **Proposed fix:** Validate that `from` address matches the expected backend `forward_addr` for the active flow. Drop packets from unexpected sources with a log warning.
- **Consideration:** Multi-service routing (Task 009) may change the validation model — coordinate with Task 009 if both are in-flight.

### Finding 14 (Low): Local Socket Recv Buffer Allocation

- **Severity:** Low
- **Component:** app-connector
- **Location:** `app-connector/src/main.rs:1965`
- **Current code:** `let mut buf = vec![0u8; 65535];` allocated on every `process_local_socket()` call.
- **Risk:** Low — single 64KB allocation per poll cycle, not per-packet. Performance impact minimal at current traffic levels.
- **Oracle assessment:** Confirmed still open. Small refactor, not a one-liner — needs borrow-scope adjustment to reuse `self.recv_buf`.
- **Proposed fix:** Reuse existing `self.recv_buf` field (already allocated in constructor) instead of allocating a new buffer each call. Requires adjusting borrow scopes so `&mut self` isn't held across the buffer use.

---

## References

- Current deployment: manual systemd services on AWS EC2
- Connector idle timeout: 30s (QUIC IDLE_TIMEOUT_MS)
- Deferred from `_context/components.md`: Connector auto-reconnection
- Deferred from `_context/README.md`: monitoring, deployment automation
- Oracle findings triage: `tasks/015-oracle-quick-fixes/research.md`
