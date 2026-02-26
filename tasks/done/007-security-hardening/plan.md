# Plan: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Phases 1-5 Complete, Phases 6-8 In Progress
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** `feature/007-security-hardening`
**Last Updated:** 2026-02-25

---

## Purpose

Implement production-grade security across the ZTNA system. This is the highest-priority post-MVP task. A security review on 2026-02-21 identified 18 findings, and PR #7 code review added 8 more, for a total of 26 findings (1 Critical, 4 High, 8 Medium, 9 Low, 4 Info). All findings are documented in `research.md` with specific file locations and fix approaches.

---

## Summary Table

| # | Severity | Phase | Component | Issue |
|---|----------|-------|-----------|-------|
| C1 | **Critical** | 1 | All Rust crates | TLS peer verification disabled (`verify_peer(false)`) |
| H1 | **High** | 3 | packet_processor | Unbounded `received_datagrams` queue (OOM DoS) |
| H2 | **High** | 2 | intermediate-server | No auth/authz on service registration |
| H3 | **High** | 3 | app-connector | TCP proxy: no destination validation + blocking connect |
| H4 | **High** | 1 | k8s deployment | Placeholder TLS secrets committed to repo |
| M1 | **Medium** | 5 | Configs, Swift | Hardcoded AWS IP `3.128.36.92` in source |
| M2 | **Medium** | 4 | app-connector | Fragile P2P protocol demux based on first byte |
| M3 | **Medium** | 2 | intermediate-server | Service-routed datagram: no sender authorization |
| M4 | **Medium** | 5 | docker-compose | Excessive `NET_ADMIN`/`NET_RAW` on non-gateway containers |
| M5 | **Medium** | 4 | packet_processor (FFI) | `agent_set_local_addr` assumes 4-byte buffer |
| M6 | **Medium** | 4 | packet_processor | Keepalive interception could swallow QUIC packets |
| L1 | **Low** | 3 | app-connector | Blocking TCP connect on event loop (acknowledged MVP) |
| L2 | **Low** | 5 | Config files | Cert paths not validated at startup |
| L3 | **Low** | 5 | intermediate-server | `from_utf8_lossy` routing collisions |
| L4 | **Low** | 5 | setup-nat.sh | Unsanitized env vars in `/proc` paths |
| L5 | **Low** | 5 | Swift agent | Verify no force-unwrap on `NWEndpoint.Port` |
| L6 | **Low** | 3 | app-connector | TCP FIN removes session without half-close draining |
| L7 | **Low** | 3 | app-connector | TCP backends polled manually, not mio-integrated |
| L8 | **Low** | 5 | Swift agent | Partial multi-service registration marks fully registered |
| L9 | **Low** | 5 | deploy docs | SSH guide disables host key verification |
| I1 | **Info** | 5 | All servers | Verbose network topology logging at `info` level |
| I2 | **Info** | 5 | docker-compose | Verify `certs/` is in `.gitignore` |
| I3 | **Info** | 5 | TEST_REPORT.md | Local filesystem paths exposed in test report |
| I4 | **Info** | 5 | build-push.sh | Default registry doesn't match help text |

---

## Phases

### Phase 1: TLS Certificate Management (C1, H4)
**Priority: CRITICAL — do first**

Enable TLS peer verification on all QUIC connections. This is the single most important fix — it currently undermines the entire "Zero Trust" model.

- Replace `verify_peer(false)` with `verify_peer(true)` in all 3 Rust crates
- Implement CA cert loading for client configs
- Let's Encrypt integration (DNS-01 challenge for UDP services)
- Remove placeholder secrets from k8s base resources

### Phase 2: Client Authentication & Authorization (H2, M3)
**Priority: HIGH**

Prevent unauthorized clients from registering for arbitrary services.

- Implement client authentication on Intermediate Server
- Service registration authorization (signed tokens or mTLS cert-based)
- Sender authorization on 0x2F service-routed datagrams
- Log warning on Connector registration replacement

### Phase 3: Rate Limiting & DoS Protection (H1, H3)
**Priority: HIGH**

Prevent resource exhaustion and abuse.

- Cap `received_datagrams` queue (OOM prevention)
- TCP proxy destination IP validation (SSRF prevention)
- Non-blocking TCP connect with mio-integrated backend streams (event loop DoS prevention)
- TCP half-close handling (drain backend before teardown)
- Per-IP rate limiting on Intermediate Server

### Phase 4: Protocol Hardening (M2, M5, M6)
**Priority: MEDIUM**

Harden protocol-level ambiguities and FFI safety.

- P2P control message magic prefix (avoid QUIC collision)
- FFI buffer length validation on `agent_set_local_addr`
- Keepalive message magic prefix (avoid 5-byte QUIC collision)
- QUIC stateless retry tokens
- Registration ACK protocol

### Phase 5: Configuration & Operational Security (M1, M4, M7-M8, L1-L5, L8-L9, I1-I4)
**Priority: LOW-MEDIUM**

Clean up hardcoded values, excessive permissions, and operational hygiene.

- Remove hardcoded AWS IP from Swift defaults
- Remove excessive Docker capabilities
- Make `parseIPv4` return optional (fail explicitly on non-IPv4 input)
- Fix `--no-push` to error on multi-platform builds
- Track per-service registration state in Swift agent
- Strict UTF-8 validation on service IDs
- Startup cert path validation
- Replace insecure SSH guidance in deploy docs
- Redact local paths in test reports
- Align build script defaults with documentation
- Production logging levels

---

## Phase 6: mTLS & Certificate Infrastructure — HIGH priority

### Phase 6A: mTLS Client Authentication (2-3 days)

Intermediate Server validates client certificates during QUIC handshake, extracts identity from cert SAN, and authorizes service registration.

**SAN convention:** `DNS:agent.<service>.ztna` / `DNS:connector.<service>.ztna` / `DNS:agent.*.ztna` (wildcard). No ZTNA SAN = allow all (backward compat).

- Add `x509-parser` + `signal-hook` dependencies to intermediate-server
- Create `auth.rs` module — `ClientIdentity`, `extract_identity()`, `is_authorized_for_service()`
- Add `authenticated_identity` + `authenticated_services` fields to Client struct
- Extract peer cert after handshake via `conn.peer_cert()`, parse with auth module
- Authorize service registration in `handle_registration()`
- Add `--require-client-cert` CLI flag (default: false)
- Create cert generation script (`scripts/generate-client-certs.sh`)
- Unit tests for auth module
- Add `--client-cert`/`--client-key` to quic-test-client

### Phase 6B: Certificate Auto-Renewal (1-2 days)

- SIGHUP handler for cert hot-reload (re-create `quiche::Config`)
- AWS certbot setup script (Route53 DNS-01)
- Systemd timer for renewal with SIGHUP deploy-hook
- K8s cert-manager CRDs (Issuer + Certificate)

## Phase 7: Network Hardening — MEDIUM priority (parallelizes with Phase 6)

### Phase 7A: Non-Blocking TCP Proxy / mio Integration (2-3 days)

Replace `StdTcpStream::connect_timeout(500ms)` with non-blocking `mio::net::TcpStream::connect()`. Eliminates event loop blocking under SYN floods to unreachable backends.

- Add `TcpConnState` enum + update `TcpSession` struct for mio::net::TcpStream
- Implement token allocator for TCP sockets (Token(2+))
- Replace blocking connect with non-blocking mio connect
- Handle WRITABLE (connect completion) and READABLE (data) events
- Migrate `process_tcp_sessions()` to event-driven model
- Session cleanup with mio deregistration
- Non-blocking connect timeout (5s, checked in periodic sweep)

### Phase 7B: Stateless Retry Tokens (1 day)

QUIC anti-amplification via `quiche::retry()`.

- Generate AEAD token encryption key at startup (ring AES-256-GCM)
- Token generation/validation (encrypted addr + dcid + timestamp)
- Modify `handle_new_connection()` for retry flow
- Add `--disable-retry` flag

## Phase 8: Protocol Completeness — LOW priority

### Phase 8A: Registration ACK Protocol (1 day)

Replace fire-and-forget 0x10/0x11 with acknowledged registration.

- Define `REG_TYPE_ACK = 0x12`, `REG_TYPE_NACK = 0x13` in all 3 crates
- Server sends ACK/NACK after registration
- Agent retry logic (2s timeout, 3 retries max)
- Connector retry logic (RegistrationState enum)

### Phase 8B: Connection ID Rotation (0.5 days)

Periodic CID rotation for privacy.

- CID rotation timer (5-min default) with `cid_aliases` HashMap
- Server-side `rotate_connection_ids()` via `conn.new_source_cid()`
- Client-side rotation in Agent (FFI tick) and Connector

---

## Success Criteria

**Phases 1-5 (COMPLETE):**
- [x] All QUIC connections verify peer TLS certificates
- [x] Agents can only route to services they're registered for
- [x] `received_datagrams` queue is bounded
- [x] TCP proxy validates destination IP
- [x] No hardcoded infrastructure IPs in source code
- [x] P2P control and keepalive messages have distinctive prefixes
- [x] FFI functions validate buffer lengths
- [x] Pre-deploy script validates k8s secrets exist

**Phases 6-8 (IN PROGRESS):**
- [ ] mTLS: Client certificates validated, service authorization from cert SAN
- [ ] Cert auto-renewal: SIGHUP hot-reload, certbot/cert-manager integration
- [ ] Non-blocking TCP: No event loop blocking on backend connect
- [ ] Stateless retry: Amplification attack prevention
- [ ] Registration ACK: Clients get confirmation of registration success/failure
- [ ] CID rotation: Connection IDs rotated periodically for privacy
