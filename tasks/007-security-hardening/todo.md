# TODO: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Complete (core fixes shipped, deferred items tracked below)
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** `feature/007-security-hardening`
**Last Updated:** 2026-02-22

---

## Phase 1: TLS Certificate Management (C1, H4) — DONE

- [x] **C1:** Enable `verify_peer(true)` on Agent QUIC config (`packet_processor/src/lib.rs`)
- [x] **C1:** Enable `verify_peer(true)` on Connector QUIC config (`app-connector/src/main.rs`)
- [x] **C1:** Enable `verify_peer(true)` on Intermediate Server config (`intermediate-server/src/main.rs`)
- [x] **C1:** Implement CA cert loading with `load_verify_locations_from_file()` on client configs
- [x] **C1:** Update Swift FFI (AgentFFI.swift, bridging header) for ca_cert_path + verify_peer params
- [x] **C1:** Update PacketTunnelProvider to read TLS config from providerConfiguration
- [x] **H4:** Remove placeholder certs from `secrets.yaml` (replaced with instructions template)
- [x] **H4:** Add pre-deploy validation script (`validate-secrets.sh`) for k8s secrets

### Deferred — Certificate Operations (Future Task)

- [ ] **Let's Encrypt ACME with QUIC/UDP**: QUIC uses UDP so HTTP-01 challenge won't work. Need DNS-01 challenge (e.g., via `certbot` with Route53 plugin or `lego` CLI). The Intermediate Server and App Connector both need valid TLS certs for `verify_peer(true)` to work in production without `--no-verify-peer`.
  - **Files:** `intermediate-server/src/main.rs` (load certs at startup), systemd service unit
  - **Approach:** Run certbot/lego as a sidecar or cron job, reload certs via SIGHUP or periodic file watch
  - **Complexity:** Medium — requires DNS provider integration, cert reload without dropping QUIC connections

- [ ] **Cert auto-renewal on Intermediate Server**: Currently certs are static files loaded at startup. Need graceful cert rotation — either restart the process (simplest, brief downtime) or implement hot-reload (watch cert file mtime, re-create `quiche::Config` when changed).
  - **Files:** `intermediate-server/src/main.rs` (Server struct, config creation)
  - **Approach:** Start with process restart via systemd `ExecReload`; hot-reload is a stretch goal
  - **Complexity:** Low (restart) / High (hot-reload without connection drops)

- [ ] **Update Connector TLS configuration for production certs**: Connector needs its own cert for P2P server mode. Currently uses self-signed certs from `certs/` directory.
  - **Files:** `app-connector/src/main.rs` (P2P server config), deploy configs
  - **Approach:** Same cert provisioning as Intermediate, or separate certs per Connector

- [ ] **Update macOS Agent trust chain**: Agent currently uses `--no-verify-peer` for dev. In production, it needs the CA cert bundled or pointed to via `providerConfiguration["caCertPath"]`.
  - **Files:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`, app bundle resources
  - **Approach:** Bundle CA cert in app, or download from config server on first launch

- [ ] **Test certificate rotation without connection drops**: Verify that cert renewal doesn't break established QUIC connections (QUIC connections survive cert changes since TLS handshake is per-connection, but new connections need the new cert).

- [ ] **cert-manager or sealed-secrets for k8s**: Replace manual `kubectl create secret tls` with automated cert management. cert-manager can auto-provision from Let's Encrypt; sealed-secrets encrypts secrets for safe git storage.
  - **Files:** `deploy/k8s/base/`, new cert-manager CRDs
  - **Approach:** Add cert-manager Issuer + Certificate CRDs to kustomize overlay
  - **Complexity:** Medium — requires cert-manager installation on cluster

## Phase 2: Client Authentication & Authorization (H2, M3) — DONE (Partial)

- [x] **H2:** Log warning on Connector registration replacement (`registry.rs`)
- [x] **M3:** Add sender authorization check in `relay_service_datagram` (verify Agent registered for service)
- [x] **M3:** Add `is_agent_for_service()` method to Registry with unit tests

### Deferred — Full Client Authentication (Future Task)

- [ ] **Design auth approach — mTLS vs token-based**: Two viable options:
  1. **mTLS (mutual TLS)**: Each Agent/Connector gets a client certificate signed by the ZTNA CA. The Intermediate Server validates client certs during QUIC handshake. Service authorization embedded in cert Subject/SAN (e.g., `CN=agent-web-app` restricts to `web-app` service). quiche supports this via `config.verify_peer(true)` + `config.load_verify_locations_from_file()` on the server side.
     - **Pros:** Zero-trust, no token management, cert rotation via CA
     - **Cons:** Certificate provisioning complexity, revocation (CRL/OCSP needed)
  2. **Token-based (JWT/PASETO)**: Agent/Connector authenticates with a signed token in the first QUIC stream message after handshake. Token contains `{client_type, service_ids[], exp}` signed by a shared secret or asymmetric key.
     - **Pros:** Simpler provisioning (API key → token), easy revocation (short TTL)
     - **Cons:** Token replay window, need token refresh mechanism
  - **Recommendation:** Start with mTLS (aligns with existing TLS infrastructure from Phase 1)
  - **Files:** `intermediate-server/src/main.rs` (connection accept), `intermediate-server/src/client.rs`, new `auth` module

- [ ] **Implement client authentication on Intermediate Server connection accept**: After QUIC handshake completes, extract client certificate from the connection and validate it. Reject connections without valid client certs.
  - **Files:** `intermediate-server/src/main.rs` (`handle_new_connection`), quiche peer cert API
  - **quiche API:** `conn.peer_cert()` returns DER-encoded peer certificate after handshake
  - **Approach:** Parse cert, extract CN/SAN, store as `client.authenticated_identity`

- [ ] **Implement service registration authorization**: When a client sends a 0x10/0x11 registration message, verify the authenticated identity is authorized for the requested service ID.
  - **Files:** `intermediate-server/src/main.rs` (`handle_registration`), `registry.rs`
  - **Approach:** Compare `service_id` against allowed services from client cert SAN or an ACL config file

- [ ] **Credential provisioning for Agents**: How Agents get their client certificates.
  - **Options:** Manual cert distribution, SCEP/EST protocol, admin API that generates certs
  - **Files:** New provisioning service or script, Agent configuration UI
  - **Complexity:** High — requires PKI infrastructure or a credential management service

- [ ] **Credential provisioning for Connectors**: Same as Agents but for Connector deployments.
  - **Files:** Deploy configs, systemd services, Docker entrypoints
  - **Approach:** Mount client cert as k8s secret or Docker volume

- [ ] **Test unauthorized client rejection**: Integration test that connects without valid cert and verifies rejection.
  - **Files:** `intermediate-server/tests/integration_test.rs`

## Phase 3: Rate Limiting & DoS Protection (H1, H3, L6, L7) — DONE

- [x] **H1:** Cap `received_datagrams` queue depth — already implemented as `MAX_QUEUED_DATAGRAMS = 4096`
- [x] **H3:** Validate destination IP in TCP proxy matches expected virtual service IP
- [x] **H3:** Add rate limiting on new TCP session creation per source IP (`MAX_SYN_PER_SOURCE_PER_SECOND = 10`)
- [x] **L6:** Implement TCP half-close: drain backend stream on FIN before removing session (`TCP_DRAIN_TIMEOUT_SECS = 5`)

### Deferred — Advanced Rate Limiting (Future Task)

- [ ] **Migrate TCP proxy to non-blocking mio TcpStream**: Currently uses `StdTcpStream::connect_timeout(500ms)` which blocks the single-threaded mio event loop. Under SYN floods to unreachable backends, this stalls all QUIC processing for up to 500ms per connection attempt.
  - **Files:** `app-connector/src/main.rs` (TCP session creation, `handle_tcp_packet`)
  - **Approach:** Replace `StdTcpStream::connect_timeout` with `mio::net::TcpStream::connect()` (non-blocking), register with mio Poll, handle `WRITABLE` event as connect completion
  - **Complexity:** High — requires restructuring TCP session state machine, integrating TCP fds into the mio event loop alongside QUIC UDP socket
  - **Also resolves:** L7 (TCP backends polled manually instead of mio-integrated)

- [ ] **Per-IP connection rate limiting on Intermediate Server**: Limit how many QUIC connections a single source IP can establish per time window. Prevents connection-exhaustion DoS.
  - **Files:** `intermediate-server/src/main.rs` (`handle_new_connection`)
  - **Approach:** `HashMap<IpAddr, (Instant, u32)>` with sliding window, similar to TCP SYN rate limiter in Connector

- [ ] **Registration flood protection**: Rate-limit 0x10/0x11 registration messages per connection to prevent registry churn.
  - **Files:** `intermediate-server/src/main.rs` (`handle_registration`), `client.rs`

- [ ] **DATAGRAM throughput limits per connection**: Cap bytes-per-second or datagrams-per-second per QUIC connection to prevent a single client from monopolizing bandwidth.
  - **Files:** `intermediate-server/src/main.rs` (`process_datagrams`)

- [ ] **Load testing**: Test all rate limiting under simulated load (e.g., using `quic-test-client` with concurrent connections).

## Phase 4: Protocol Hardening (M2, M5, M6) — DONE

- [x] **M2/M6:** Add ZTNA_MAGIC `0x5A` prefix to keepalive messages (avoids QUIC header collision)
- [x] **M2/M6:** Validate magic bytes on keepalive receive
- [x] **M5:** Add `ip_len` parameter to `agent_set_local_addr` FFI, validate `>= 4` before dereference

### Deferred — Advanced Protocol Features (Future Task)

- [ ] **Stateless retry tokens (QUIC anti-amplification)**: Without retry, the Intermediate Server is vulnerable to amplification attacks — an attacker spoofs source IP and the server sends back more data than it received. quiche supports `retry()` but we don't use it.
  - **Files:** `intermediate-server/src/main.rs` (`handle_new_connection`)
  - **quiche API:** `quiche::retry()` generates a Retry packet; client resends Initial with token
  - **Approach:** On first Initial packet, send Retry with token. Only accept connections with valid retry token.
  - **Complexity:** Medium — need token generation/validation, affects connection setup latency

- [ ] **Registration ACK protocol**: Currently registration (0x10/0x11 datagrams) is fire-and-forget. The Agent/Connector has no confirmation that registration succeeded. Silently fails if Intermediate drops the message.
  - **Files:** `intermediate-server/src/main.rs` (`handle_registration`), `core/packet_processor/src/lib.rs`, `app-connector/src/main.rs`
  - **Approach:** Define new 0x12 ACK datagram type. Server sends ACK with service_id after successful registration. Client retries if no ACK within timeout.
  - **Complexity:** Medium — new message type, retry logic, timeout handling

- [ ] **Connection ID rotation**: QUIC supports connection migration via new connection IDs. Without rotation, a connection is trackable across network changes by its static connection ID.
  - **Files:** All QUIC components
  - **quiche API:** `conn.new_source_cid()`, `conn.retire_destination_cid()`
  - **Complexity:** Low-Medium — quiche handles most of the mechanics

- [ ] **Update quic-test-client for auth testing**: Add `--ca-cert` and `--no-verify-peer` flags to the e2e test client.
  - **Files:** `tests/e2e/fixtures/quic-client/src/main.rs`

## Phase 5: Configuration & Operational Security — DONE

- [x] **M1:** Remove hardcoded AWS IP `3.128.36.92` from Swift defaults and deploy configs (→ `0.0.0.0`)
- [x] **M4:** Remove `NET_ADMIN`/`NET_RAW` from non-gateway Docker containers (`docker-compose.yml`)
- [x] **M7:** Change `parseIPv4` to return optional `[UInt8]?` for safer IP parsing
- [x] **M8:** Fix `--no-push` to fail fast (exit 1) on multi-platform builds instead of silently pushing
- [x] **L2:** Add startup validation for cert/key file paths in intermediate-server and app-connector
- [x] **L3:** Replace `from_utf8_lossy` with strict `from_utf8` for service IDs (rejects invalid UTF-8)
- [x] **L4:** Validate interface names in `setup-nat.sh` (regex `^[a-zA-Z0-9]+$`)
- [x] **L5:** Audited Swift code for force-unwraps on `NWEndpoint.Port` — all use `guard let`, no fixes needed
- [x] **L8:** Track per-service registration state (`registeredServices: Set<String>`)
- [x] **L9:** Replace `StrictHostKeyChecking=no` in SSH guide with `ssh-keyscan` approach + security warning
- [x] **I1:** Reduce data-plane logging to `debug` level (6 entries in intermediate, 3 in connector)
- [x] **I2:** Verified `certs/` is in `.gitignore`, removed `!**/certs/*.pem` exception
- [x] **I3:** Redact local filesystem paths in `TEST_REPORT.md` (→ relative paths)
- [x] **I4:** Verified build-push.sh defaults already aligned (`ghcr.io`, `hfyeomans`)
