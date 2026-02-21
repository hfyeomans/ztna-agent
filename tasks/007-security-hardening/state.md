# State: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started (security review complete)
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Current State

Security review completed 2026-02-21. All findings documented in `research.md` with specific file locations, severity ratings, and fix approaches. Implementation has not started.

### Security Review Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| Critical | 1 | TLS verification disabled everywhere |
| High | 4 | Unbounded queues, no auth on registration, open proxy, committed secrets |
| Medium | 6 | Hardcoded IPs, fragile protocol demux, missing authz, FFI buffer safety |
| Low | 5 | Blocking I/O, config validation, UTF-8 routing, shell sanitization |
| Info | 2 | Verbose logging, cert mount hygiene |

### What Exists (from MVP)

- Self-signed TLS certs in `certs/` directory
- QUIC TLS handshake with ALPN `ztna-v1` (but `verify_peer(false)` everywhere)
- No client authentication (any client can connect and register for any service)
- No rate limiting on Intermediate Server
- No queue depth limits on datagram buffers
- Registration is fire-and-forget (no ACK)
- TCP proxy forwards without destination validation
- Hardcoded AWS IP in Swift defaults and config files
- P2P/keepalive protocol demux based on fragile byte patterns

### What This Task Delivers

- TLS peer verification enabled on all QUIC connections
- Let's Encrypt certificates with auto-renewal
- Client authentication (mTLS or token-based)
- Service registration authorization
- Rate limiting and queue depth caps
- TCP proxy destination validation
- Protocol-level magic prefixes for P2P/keepalive
- FFI buffer length validation
- Hardcoded IP removal
- Production logging levels

### Previously Tracked Items (from `_context/README.md`)

These items were already called out as deferred post-MVP for Task 007:
- TLS Certificate Verification (→ C1)
- Client Authentication (→ H2)
- Rate Limiting (→ H1, H3)
- Stateless Retry (→ Phase 4)
- Registration ACK (→ Phase 4)
- Production Certificates (→ H4)

### New Findings from Security Review

These were NOT previously tracked:
- H1: Unbounded datagram queue (OOM DoS)
- H3: TCP proxy destination validation + blocking connect
- M1: Hardcoded AWS IP
- M2: Fragile P2P demux
- M3: Missing sender authorization on 0x2F routing
- M4: Excessive Docker capabilities
- M5: FFI buffer length not validated
- M6: Keepalive/QUIC packet collision risk
- L3: UTF-8 lossy service ID parsing
- L4: Unsanitized shell env vars

---

## Decisions Log

(No implementation decisions yet — task not started)
