# TODO: Production Operations

**Task ID:** 008-production-operations
**Status:** Complete
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** feature/008-production-operations
**Last Updated:** 2026-02-27

---

## Purpose

Track implementation tasks for monitoring, graceful shutdown, Connector auto-reconnection, deployment automation, and CI/CD.

---

## Phase 1: Connector Auto-Reconnection

- [x] Add reconnection loop to App Connector run() — replaces `break` with backoff loop
- [x] Implement exponential backoff (1s → 30s cap) — RECONNECT_INITIAL_DELAY_MS, RECONNECT_MAX_DELAY_MS
- [x] Re-register service after reconnect — reg_state reset to NotRegistered, maybe_register() handles
- [x] Re-establish P2P listener after reconnect — P2P clients unaffected (separate connections)
- [x] SIGTERM handler for clean exit during reconnection — signal-hook AtomicBool pattern
- [ ] Test: restart Intermediate, Connector auto-recovers (requires live AWS test — deferred)
- [x] Remove dependency on systemd restart for liveness

## Phase 2: Monitoring

- [x] Add Prometheus metrics HTTP endpoint to Intermediate Server — mio TcpListener, /metrics + /healthz
- [x] Expose metrics: active_connections, relay_bytes_total, registrations_total + 6 more counters
- [x] Add metrics to App Connector (forwarded_packets, tcp_sessions, errors) — 6 atomic counters
- [ ] Create Grafana dashboard JSON (deferred — requires Grafana instance)
- [x] Add health check endpoint (HTTP 200 if QUIC listener active) — GET /healthz on both

## Phase 3: Graceful Shutdown

- [x] Handle SIGTERM in Intermediate Server (drain connections) — drain_and_shutdown(), 3s drain period
- [x] Handle SIGINT in Intermediate Server — same shutdown_flag
- [x] Handle SIGTERM in App Connector — clean loop exit
- [x] Implement connection draining — APPLICATION_CLOSE (0x00) to all clients, poll for close acks
- [ ] Notify Agents of service unavailability (deferred — agents detect connection close via QUIC)
- [ ] Test zero-downtime restart (requires live AWS test — deferred)

## Phase 4: Deployment Automation

- [x] Create Terraform module for AWS (VPC, EC2, SG, EIP) — deploy/terraform/
- [x] Create Ansible playbook for service deployment — deploy/ansible/
- [x] Create production Dockerfiles for Intermediate + Connector — deploy/docker/
- [x] Create deploy/README.md documenting all deployment options
- [ ] Update k8s manifests for production (deferred — existing kustomize works for Pi cluster)

## Phase 5: CI/CD

- [x] GitHub Actions: Build Rust binaries (linux amd64/arm64, macOS arm64) — .github/workflows/release.yml
- [x] GitHub Actions: Run unit tests — .github/workflows/test.yml (5-crate matrix)
- [ ] GitHub Actions: Run E2E test suite (deferred — needs Docker + server infrastructure in CI)
- [x] Automated container image builds and push — GHCR multi-arch images in release.yml

## Oracle Findings (Cross-Cutting)

### Finding 7 (High): Local UDP Injection
- [x] Validate source address in `process_local_socket()` against expected `forward_addr` IP
- [x] Drop packets from unexpected sources with `log::warn!()`
- [x] Coordinate with Task 009 if multi-service routing changes validation model (noted — IP-based validation)
- [ ] Add test: UDP from unexpected source address is dropped (network mock needed — deferred)

### Finding 14 (Low): Recv Buffer Allocation
- [x] Refactor `process_local_socket()` to reuse `self.recv_buf` instead of allocating `vec![0u8; 65535]`
- [x] Adjust borrow scopes — copy received data via `to_vec()` before calling `&mut self` methods
- [ ] Add benchmark: measure allocation reduction in high-PPS scenarios (deferred — perf optimization)

---

## Summary

All primary implementation items complete. Deferred items require either live AWS infrastructure
or additional tooling (Grafana, Docker-in-CI, network mocking) and are documented above.

**Commits:**
1. `bb917f1` — feat: Add auto-reconnection, graceful shutdown, and Oracle security fixes
2. `b89f026` — feat: Add Prometheus metrics and health check endpoints
3. `8b1f6e9` — feat: Add deployment automation and CI/CD pipelines
4. `5de9805` — fix: Address Oracle review findings (3 issues)

**Test Results:** 153 unit tests pass (24 app-connector + 45 intermediate-server + 84 packet_processor), clippy clean.
