# TODO: Security Hardening

**Task ID:** 007-security-hardening
**Status:** In Progress
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** `feature/007-security-hardening`
**Last Updated:** 2026-02-21

---

## Phase 1: TLS Certificate Management (C1, H4) — DONE

- [x] **C1:** Enable `verify_peer(true)` on Agent QUIC config (`packet_processor/src/lib.rs`)
- [x] **C1:** Enable `verify_peer(true)` on Connector QUIC config (`app-connector/src/main.rs`)
- [x] **C1:** Enable `verify_peer(true)` on Intermediate Server config (`intermediate-server/src/main.rs`)
- [x] **C1:** Implement CA cert loading with `load_verify_locations_from_file()` on client configs
- [x] **C1:** Update Swift FFI (AgentFFI.swift, bridging header) for ca_cert_path + verify_peer params
- [x] **C1:** Update PacketTunnelProvider to read TLS config from providerConfiguration
- [ ] Research Let's Encrypt ACME with QUIC/UDP (DNS-01 challenge likely needed) — deferred to ops task
- [ ] Implement cert auto-renewal on Intermediate Server — deferred to ops task
- [ ] Update Connector TLS configuration for production certs — deferred to ops task
- [ ] Update macOS Agent trust chain (remove self-signed cert trust) — deferred to ops task
- [ ] Test certificate rotation without connection drops — deferred to ops task
- [x] **H4:** Remove placeholder certs from `secrets.yaml` (replaced with instructions template)
- [x] **H4:** Add pre-deploy validation script (`validate-secrets.sh`) for k8s secrets
- [ ] **H4:** Consider cert-manager or sealed-secrets for k8s — deferred to ops task

## Phase 2: Client Authentication & Authorization (H2, M3) — IN PROGRESS

- [ ] Design auth approach (mTLS vs token-based)
- [ ] **H2:** Implement client authentication on Intermediate Server connection accept
- [ ] **H2:** Implement service registration authorization (signed tokens or cert-based)
- [ ] **H2:** Log warning on Connector registration replacement (`registry.rs:57`)
- [ ] **M3:** Add sender authorization check in `relay_service_datagram` (verify Agent registered for service)
- [ ] Implement credential provisioning for Agents
- [ ] Implement credential provisioning for Connectors
- [ ] Test unauthorized client rejection

## Phase 3: Rate Limiting & DoS Protection (H1, H3, L6, L7) — DONE

- [x] **H1:** Cap `received_datagrams` queue depth — already implemented as `MAX_QUEUED_DATAGRAMS = 4096`
- [x] **H3:** Validate destination IP in TCP proxy matches expected virtual service IP
- [x] **H3:** Add rate limiting on new TCP session creation per source IP
- [ ] **H3:** Migrate TCP proxy to non-blocking `mio::net::TcpStream::connect()` — deferred (requires significant refactor)
- [ ] **H3:** Register TCP backend streams with mio `Poll` for event-driven I/O — deferred (L7)
- [x] **L6:** Implement TCP half-close: drain backend stream on FIN before removing session
- [ ] Add per-IP connection rate limiting on Intermediate — deferred
- [ ] Add registration flood protection — deferred
- [ ] Add DATAGRAM throughput limits per connection — deferred
- [ ] Test under simulated load — deferred

## Phase 4: Protocol Hardening (M2, M5, M6) — DONE

- [ ] Implement stateless retry tokens (QUIC anti-amplification) — deferred
- [ ] Implement Registration ACK (replace fire-and-forget) — deferred
- [ ] Implement connection ID rotation — deferred
- [x] **M2/M6:** Add ZTNA_MAGIC prefix to keepalive messages (avoids QUIC header collision)
- [x] **M2/M6:** Validate magic bytes on keepalive receive
- [ ] **M5:** Add `ip_len` parameter to `agent_set_local_addr` FFI — already present (validate >= 4)
- [ ] Update quic-test-client for auth testing — deferred to Phase 2

## Phase 5: Configuration & Operational Security (M1, M7, M8, L2-L5, L8-L9, I1-I4) — PARTIAL

- [ ] **M1:** Remove hardcoded AWS IP `3.128.36.92` from Swift defaults — needs review
- [ ] **M1:** Move real IPs to `.env` files in `.gitignore` — needs review
- [x] **M7:** Change `parseIPv4` to return optional for safer IP parsing
- [ ] **M8:** Fix `--no-push` to fail fast on multi-platform builds — needs review
- [ ] **M4:** Remove `NET_ADMIN`/`NET_RAW` from non-gateway Docker containers — needs review
- [x] **L2:** Add startup validation for cert/key file paths in intermediate-server
- [x] **L3:** Replace `from_utf8_lossy` with strict `from_utf8` for service IDs
- [ ] **L4:** Validate interface names in `setup-nat.sh` — needs review
- [ ] **L5:** Audit Swift code for force-unwraps on `NWEndpoint.Port` — needs review
- [x] **L8:** Track per-service registration state (`registeredServices: Set<String>`)
- [ ] **L9:** Replace `StrictHostKeyChecking=no` in SSH guide — needs review
- [ ] **I1:** Reduce network topology logging to `debug` level — needs review
- [ ] **I2:** Verify `certs/` directory is in `.gitignore` — needs review
- [ ] **I3:** Redact local filesystem paths in `TEST_REPORT.md` — needs review
- [ ] **I4:** Align build-push.sh default registry/owner with help text — needs review
