# State: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track the current state of security hardening implementation. This task addresses production TLS certificates, client authentication, rate limiting, and protocol hardening.

---

## Current State

Not started. Task 006 MVP uses self-signed certificates and has no client authentication or rate limiting.

### What Exists (from MVP)
- Self-signed TLS certs in `certs/` directory
- QUIC TLS handshake with ALPN `ztna-v1`
- No client authentication (any client can connect and register)
- No rate limiting on Intermediate Server
- Registration is fire-and-forget (no ACK)

### What This Task Delivers
- Let's Encrypt TLS certificates with auto-renewal
- Client authentication (mTLS or token-based)
- Rate limiting on Intermediate Server
- Stateless retry tokens (QUIC anti-amplification)
- Registration ACK protocol

---

## Decisions Log

(No decisions yet — task not started)
