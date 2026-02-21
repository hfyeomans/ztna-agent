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

## References

- Current deployment: manual systemd services on AWS EC2
- Connector idle timeout: 30s (QUIC IDLE_TIMEOUT_MS)
- Deferred from `_context/components.md`: Connector auto-reconnection
- Deferred from `_context/README.md`: monitoring, deployment automation
