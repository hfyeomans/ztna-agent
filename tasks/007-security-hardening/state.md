# State: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started (security review complete)
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Current State

Security review completed 2026-02-21. PR #7 code review (Gemini + CodeRabbit) added 8 additional findings on 2026-02-21. All 26 findings documented in `research.md` with specific file locations, severity ratings, and fix approaches. Implementation has not started.

### Security Review Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| Critical | 1 | TLS verification disabled everywhere |
| High | 4 | Unbounded queues, no auth on registration, open proxy, committed secrets |
| Medium | 8 | Hardcoded IPs, fragile protocol demux, missing authz, FFI buffer safety, parseIPv4 fragile, build script push safety |
| Low | 9 | Blocking I/O, config validation, UTF-8 routing, shell sanitization, TCP half-close, partial registration, SSH guide |
| Info | 4 | Verbose logging, cert mount hygiene, local path exposure, build script defaults |

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

### New Findings from PR #7 Code Review (Gemini + CodeRabbit)

These were added from automated code review on 2026-02-21:
- M7: `parseIPv4` returns `[0,0,0,0]` for non-IPv4 input (fragile error handling)
- M8: `--no-push` silently pushes on multi-platform builds
- L6: TCP FIN removes session without half-close draining
- L7: TCP backends polled manually instead of mio-integrated (resolved by H3)
- L8: Partial multi-service registration marks agent as fully registered
- L9: SSH guide recommends disabling host key verification
- I3: Local filesystem paths exposed in TEST_REPORT.md
- I4: Build script default registry doesn't match help text

---

## Decisions Log

(No implementation decisions yet — task not started)
