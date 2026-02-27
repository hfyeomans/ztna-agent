# Plan: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening ✅ (PR #8 merged)
**Branch:** (not yet created)
**Last Updated:** 2026-02-26

---

## Purpose

Plan the implementation of monitoring, graceful shutdown, Connector auto-reconnection, deployment automation, and CI/CD pipelines.

---

## Oracle Review Findings (Assigned to This Task)

From `oracle-review-01.md` — must be addressed as part of this task:

| Severity | Finding | Evidence | Description |
|----------|---------|----------|-------------|
| **High** | Local UDP injection | `app-connector/src/main.rs:1964-1985` | Connector accepts UDP from any local process and forwards into tunnel without checking source matches `forward_addr` |
| **Low** | Recv buffer allocation per poll | `app-connector/src/main.rs:1965` | `vec![0u8; 65535]` allocated every `process_local_socket()` call instead of reusing `self.recv_buf` |

**Note:** Finding 7 line references updated to current codebase (post-Task 007). Original oracle references were pre-Task 007.

---

## Phases (To Be Defined)

### Phase 1: Connector Auto-Reconnection
- Add reconnection logic to App Connector (match Agent pattern)
- Exponential backoff on connection loss
- Re-registration after reconnect
- Remove dependency on systemd restarts for liveness

### Phase 2: Monitoring
- Add Prometheus metrics endpoint to Intermediate Server
- Expose key metrics: connections, throughput, latency, errors
- Create Grafana dashboard templates
- Add health check endpoints

### Phase 3: Graceful Shutdown
- QUIC connection draining on SIGTERM
- Connector deregistration protocol
- Agent notification of service changes

### Phase 4: Deployment Automation
- Terraform modules for AWS infrastructure
- Ansible playbooks for service deployment
- Container image CI builds

### Phase 5: CI/CD Pipeline
- GitHub Actions workflow for build + test
- Automated E2E test suite
- Release artifact publishing

---

## Success Criteria

- [ ] Connector auto-reconnects without systemd restarts
- [ ] Prometheus metrics available for all components
- [ ] Zero-downtime restarts possible
- [ ] Single-command deployment to AWS
- [ ] All tests run in CI on every push
