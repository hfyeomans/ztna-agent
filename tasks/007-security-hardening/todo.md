# TODO: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Phases 1-5 Complete, Phases 6-8 In Progress
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** `feature/007-security-hardening`
**Last Updated:** 2026-02-25

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

## Phase 2: Client Authentication & Authorization (H2, M3) — DONE

- [x] **H2:** Log warning on Connector registration replacement (`registry.rs`)
- [x] **M3:** Add sender authorization check in `relay_service_datagram` (verify Agent registered for service)
- [x] **M3:** Add `is_agent_for_service()` method to Registry with unit tests

## Phase 3: Rate Limiting & DoS Protection (H1, H3, L6, L7) — DONE

- [x] **H1:** Cap `received_datagrams` queue depth — already implemented as `MAX_QUEUED_DATAGRAMS = 4096`
- [x] **H3:** Validate destination IP in TCP proxy matches expected virtual service IP
- [x] **H3:** Add rate limiting on new TCP session creation per source IP (`MAX_SYN_PER_SOURCE_PER_SECOND = 10`)
- [x] **L6:** Implement TCP half-close: drain backend stream on FIN before removing session (`TCP_DRAIN_TIMEOUT_SECS = 5`)

## Phase 4: Protocol Hardening (M2, M5, M6) — DONE

- [x] **M2/M6:** Add ZTNA_MAGIC `0x5A` prefix to keepalive messages (avoids QUIC header collision)
- [x] **M2/M6:** Validate magic bytes on keepalive receive
- [x] **M5:** Add `ip_len` parameter to `agent_set_local_addr` FFI, validate `>= 4` before dereference

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

## Phase 6A: mTLS Client Authentication — DONE

- [x] **6A.1:** Add `x509-parser = "0.16"` and `signal-hook = "0.3"` to `intermediate-server/Cargo.toml`
- [x] **6A.2:** Create `intermediate-server/src/auth.rs` — `ClientIdentity` struct, `extract_identity(der_cert)`, `is_authorized_for_service()`
- [x] **6A.3:** Add `authenticated_identity` + `authenticated_services` fields to Client struct (`intermediate-server/src/client.rs`)
- [x] **6A.4:** Extract peer cert after handshake — `conn.peer_cert()` when `is_established()`, parse with auth module (`intermediate-server/src/main.rs`)
- [x] **6A.5:** Authorize service registration — check `authenticated_services` in `handle_registration()` (`intermediate-server/src/main.rs`)
- [x] **6A.6:** Add `--require-client-cert` CLI flag + `ServerConfig` field, default false (`intermediate-server/src/main.rs`)
- [x] **6A.7:** Create cert generation script — CA + Agent/Connector client certs with SAN authorization (`scripts/generate-client-certs.sh`)
- [x] **6A.8:** Unit tests for auth module — cert parsing, SAN extraction, authorization, invalid cert handling (`intermediate-server/src/auth.rs`)
- [x] **6A.9:** Add `--client-cert` and `--client-key` flags to quic-test-client (`tests/e2e/fixtures/quic-client/src/main.rs`)

## Phase 6B: Certificate Auto-Renewal — DONE

- [x] **6B.1:** SIGHUP handler for cert hot-reload — `signal-hook` AtomicBool, re-create `quiche::Config` on signal (`intermediate-server/src/main.rs`)
- [x] **6B.2:** AWS certbot setup script — Route53 DNS-01 plugin (`deploy/aws/setup-certbot.sh`)
- [x] **6B.3:** Systemd timer for cert renewal with SIGHUP deploy-hook (`deploy/aws/ztna-cert-renew.{service,timer}`)
- [x] **6B.4:** K8s cert-manager CRDs — Issuer + Certificate in kustomize overlay (`deploy/k8s/overlays/cert-manager/`)

## Phase 7A: Non-Blocking TCP Proxy / mio Integration — DONE

- [x] **7A.1:** Add `TcpConnState` enum + update `TcpSession` for `mio::net::TcpStream`, `mio_token`, `conn_state`, `connect_started` (`app-connector/src/main.rs`)
- [x] **7A.2:** Add token allocator — `next_tcp_token: usize` (starts at 2), `token_to_flow: HashMap<Token, FlowKey>` (`app-connector/src/main.rs`)
- [x] **7A.3:** Replace `StdTcpStream::connect_timeout(500ms)` with `mio::net::TcpStream::connect()`, register with mio Poll (`app-connector/src/main.rs`)
- [x] **7A.4:** Handle mio events for TCP tokens — WRITABLE=connect complete, READABLE=data. Send SYN-ACK after connect (`app-connector/src/main.rs`)
- [x] **7A.5:** Migrate `process_tcp_sessions()` to event-driven — I/O in `process_tcp_event()`, keep timeout/drain in sweep (`app-connector/src/main.rs`)
- [x] **7A.6:** Session cleanup — deregister from mio Poll, remove token_to_flow entry (`app-connector/src/main.rs`)
- [x] **7A.7:** Non-blocking connect timeout — 5s checked in periodic sweep, send RST on timeout (`app-connector/src/main.rs`)

## Phase 7B: Stateless Retry Tokens — DONE

- [x] **7B.1:** Generate AEAD token encryption key at startup — `ring::aead::AES_256_GCM` (`intermediate-server/src/main.rs`)
- [x] **7B.2:** Token generation/validation — encrypt `[addr, dcid, timestamp]`, validate addr + freshness (<60s) (`intermediate-server/src/main.rs`)
- [x] **7B.3:** Modify `handle_new_connection()` — no token → Retry, valid token → accept with odcid (`intermediate-server/src/main.rs`)
- [x] **7B.4:** Add `--disable-retry` CLI flag for dev/testing (`intermediate-server/src/main.rs`)

## Phase 8A: Registration ACK Protocol — DONE

- [x] **8A.1:** Define `REG_TYPE_ACK = 0x12` and `REG_TYPE_NACK = 0x13` constants in all 3 crates
- [x] **8A.2:** Server sends ACK `[0x12, status, id_len, service_id...]` after `registry.register()`, NACK on denial (`intermediate-server/src/main.rs`)
- [x] **8A.3:** Agent retry logic — `pending_registration` tuple, 2s timeout, 3 retries max (`core/packet_processor/src/lib.rs`)
- [x] **8A.4:** Connector retry logic — `RegistrationState` enum replacing `registered: bool` (`app-connector/src/main.rs`)
- [x] **8A.5:** Handle 0x12/0x13 in client datagram processing (`core/packet_processor/src/lib.rs`, `app-connector/src/main.rs`)

## Phase 8B: Connection ID Rotation — DONE

- [x] **8B.1:** Add CID rotation timer (5-min default) and `cid_aliases` HashMap to Server (`intermediate-server/src/main.rs`)
- [x] **8B.2:** `rotate_connection_ids()` — `conn.new_scid()`, update aliases, cleanup on close (`intermediate-server/src/main.rs`)
- [x] **8B.3:** Client-side CID rotation in Agent (FFI tick) and Connector (`core/packet_processor/src/lib.rs`, `app-connector/src/main.rs`)
