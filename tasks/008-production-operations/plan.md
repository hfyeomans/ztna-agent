# Plan: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan the implementation of monitoring, graceful shutdown, Connector auto-reconnection, deployment automation, and CI/CD pipelines.

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
