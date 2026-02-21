# TODO: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Phase 1: TLS Certificate Management (C1, H4)

- [ ] **C1:** Enable `verify_peer(true)` on Agent QUIC config (`packet_processor/src/lib.rs:195`)
- [ ] **C1:** Enable `verify_peer(true)` on Connector QUIC config (`app-connector/src/main.rs:415,440`)
- [ ] **C1:** Enable `verify_peer(true)` on Intermediate Server config (`intermediate-server/src/main.rs:244`)
- [ ] **C1:** Implement CA cert loading with `load_verify_locations_from_file()` on client configs
- [ ] Research Let's Encrypt ACME with QUIC/UDP (DNS-01 challenge likely needed)
- [ ] Implement cert auto-renewal on Intermediate Server
- [ ] Update Connector TLS configuration for production certs
- [ ] Update macOS Agent trust chain (remove self-signed cert trust)
- [ ] Test certificate rotation without connection drops
- [ ] **H4:** Remove `secrets.yaml` from base `kustomization.yaml` resources
- [ ] **H4:** Add pre-deploy validation script for k8s secrets
- [ ] **H4:** Consider cert-manager or sealed-secrets for k8s

## Phase 2: Client Authentication & Authorization (H2, M3)

- [ ] Design auth approach (mTLS vs token-based)
- [ ] **H2:** Implement client authentication on Intermediate Server connection accept
- [ ] **H2:** Implement service registration authorization (signed tokens or cert-based)
- [ ] **H2:** Log warning on Connector registration replacement (`registry.rs:57`)
- [ ] **M3:** Add sender authorization check in `relay_service_datagram` (verify Agent registered for service)
- [ ] Implement credential provisioning for Agents
- [ ] Implement credential provisioning for Connectors
- [ ] Test unauthorized client rejection

## Phase 3: Rate Limiting & DoS Protection (H1, H3, L6, L7)

- [ ] **H1:** Cap `received_datagrams` queue depth (e.g., 1024) in `packet_processor/src/lib.rs:582`
- [ ] **H3:** Validate destination IP in TCP proxy matches expected virtual service IP (`app-connector/src/main.rs`)
- [ ] **H3:** Migrate TCP proxy to non-blocking `mio::net::TcpStream::connect()` (remove 500ms blocking)
- [ ] **H3:** Register TCP backend streams with mio `Poll` for event-driven I/O (resolves L7)
- [ ] **H3:** Add rate limiting on new TCP session creation per source
- [ ] **L6:** Implement TCP half-close: on FIN, drain backend stream before removing session (`main.rs:1207-1222`)
- [ ] Add per-IP connection rate limiting on Intermediate
- [ ] Add registration flood protection
- [ ] Add DATAGRAM throughput limits per connection
- [ ] Test under simulated load

## Phase 4: Protocol Hardening (M2, M5, M6)

- [ ] Implement stateless retry tokens (QUIC anti-amplification)
- [ ] Implement Registration ACK (replace fire-and-forget)
- [ ] Implement connection ID rotation
- [ ] **M2:** Add magic byte prefix to P2P control messages to avoid QUIC header collision
- [ ] **M5:** Add `ip_len` parameter to `agent_set_local_addr` FFI and validate `>= 4`
- [ ] **M6:** Add distinctive magic prefix to keepalive messages (avoid 5-byte QUIC collision)
- [ ] Update quic-test-client for auth testing

## Phase 5: Configuration & Operational Security (M1, M7, M8, L2-L5, L8-L9, I1-I4)

- [ ] **M1:** Remove hardcoded AWS IP `3.128.36.92` from Swift defaults (use `0.0.0.0` placeholder)
- [ ] **M1:** Move real IPs to `.env` files in `.gitignore`
- [ ] **M7:** Change `parseIPv4` to return optional, fail explicitly on non-IPv4 input (`PacketTunnelProvider.swift:264`)
- [ ] **M8:** Fix `--no-push` to fail fast on multi-platform builds instead of silently pushing (`build-push.sh:147-156`)
- [ ] **M4:** Remove `NET_ADMIN`/`NET_RAW` from non-gateway Docker containers
- [ ] **L2:** Add startup validation for cert/key file paths in config loading
- [ ] **L3:** Replace `from_utf8_lossy` with strict `from_utf8` for service IDs
- [ ] **L4:** Validate interface names in `setup-nat.sh` (regex `^[a-zA-Z0-9]+$`)
- [ ] **L5:** Audit Swift code for force-unwraps on `NWEndpoint.Port` construction
- [ ] **L8:** Track per-service registration state instead of boolean `hasRegistered` (`PacketTunnelProvider.swift:735-752`)
- [ ] **L9:** Replace `StrictHostKeyChecking=no` in SSH guide with `ssh-keyscan` approach (`aws-deploy-skill.md:141-143`)
- [ ] **I1:** Reduce network topology logging to `debug` level for production
- [ ] **I2:** Verify `certs/` directory is in `.gitignore`
- [ ] **I3:** Redact local filesystem paths in `TEST_REPORT.md` (replace `/Users/hank/...` with relative paths)
- [ ] **I4:** Align build-push.sh default registry/owner with help text (`docker.io` → `ghcr.io`)
