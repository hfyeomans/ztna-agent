# Plan: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Implement production-grade security across the ZTNA system. This is the highest-priority post-MVP task. A security review on 2026-02-21 identified 18 findings (1 Critical, 4 High, 6 Medium, 5 Low, 2 Info). All findings are documented in `research.md` with specific file locations and fix approaches.

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
| I1 | **Info** | 5 | All servers | Verbose network topology logging at `info` level |
| I2 | **Info** | 5 | docker-compose | Verify `certs/` is in `.gitignore` |

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
- Non-blocking TCP connect (event loop DoS prevention)
- Per-IP rate limiting on Intermediate Server

### Phase 4: Protocol Hardening (M2, M5, M6)
**Priority: MEDIUM**

Harden protocol-level ambiguities and FFI safety.

- P2P control message magic prefix (avoid QUIC collision)
- FFI buffer length validation on `agent_set_local_addr`
- Keepalive message magic prefix (avoid 5-byte QUIC collision)
- QUIC stateless retry tokens
- Registration ACK protocol

### Phase 5: Configuration & Operational Security (M1, M4, L1-L5, I1-I2)
**Priority: LOW-MEDIUM**

Clean up hardcoded values, excessive permissions, and operational hygiene.

- Remove hardcoded AWS IP from Swift defaults
- Remove excessive Docker capabilities
- Strict UTF-8 validation on service IDs
- Startup cert path validation
- Production logging levels

---

## Success Criteria

- [ ] All QUIC connections verify peer TLS certificates
- [ ] No self-signed certificates in production deployment
- [ ] Unauthorized clients cannot register for services
- [ ] Agents can only route to services they're registered for
- [ ] `received_datagrams` queue is bounded
- [ ] TCP proxy validates destination IP
- [ ] No hardcoded infrastructure IPs in source code
- [ ] P2P control and keepalive messages have distinctive prefixes
- [ ] FFI functions validate buffer lengths
- [ ] Pre-deploy script validates k8s secrets exist
