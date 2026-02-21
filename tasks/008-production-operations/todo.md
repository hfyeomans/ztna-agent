# TODO: Production Operations

**Task ID:** 008-production-operations
**Status:** Not Started
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation tasks for monitoring, graceful shutdown, Connector auto-reconnection, deployment automation, and CI/CD.

---

## Phase 1: Connector Auto-Reconnection

- [ ] Add reconnection loop to App Connector main()
- [ ] Implement exponential backoff (1s → 30s cap)
- [ ] Re-register service after reconnect
- [ ] Re-establish P2P listener after reconnect
- [ ] Test: restart Intermediate, Connector auto-recovers
- [ ] Remove dependency on systemd restart for liveness

## Phase 2: Monitoring

- [ ] Add Prometheus metrics HTTP endpoint to Intermediate Server
- [ ] Expose metrics: active_connections, relay_bytes_total, registrations_total
- [ ] Add metrics to App Connector (forwarded_packets, tcp_sessions, errors)
- [ ] Create Grafana dashboard JSON
- [ ] Add health check endpoint (HTTP 200 if QUIC listener active)

## Phase 3: Graceful Shutdown

- [ ] Handle SIGTERM in Intermediate Server (drain connections)
- [ ] Implement Connector deregistration on shutdown
- [ ] Notify Agents of service unavailability
- [ ] Test zero-downtime restart

## Phase 4: Deployment Automation

- [ ] Create Terraform module for AWS (VPC, EC2, SG, EIP)
- [ ] Create Ansible playbook for service deployment
- [ ] Create container images for Intermediate + Connector
- [ ] Update k8s manifests for production

## Phase 5: CI/CD

- [ ] GitHub Actions: Build Rust binaries (linux amd64/arm64, macOS arm64)
- [ ] GitHub Actions: Run unit tests
- [ ] GitHub Actions: Run E2E test suite
- [ ] Automated container image builds and push
