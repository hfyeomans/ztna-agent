# Research: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research TLS certificate management, client authentication, rate limiting, and protocol hardening to move from self-signed development certs to production-grade security.

---

## Research Areas

### TLS Certificate Management
- Let's Encrypt integration for Intermediate Server
- Certificate rotation without downtime
- ACME protocol with UDP-based services (DNS-01 challenge)
- Certificate pinning in macOS Agent

### Client Authentication
- Mutual TLS (mTLS) between Agent ↔ Intermediate
- Client certificate provisioning and revocation
- Token-based authentication as alternative
- MDM certificate distribution for enterprise

### Rate Limiting
- Per-connection rate limits on Intermediate Server
- Registration flood protection
- DATAGRAM throughput limits
- DDoS mitigation for public-facing QUIC endpoint

### Protocol Hardening
- Stateless retry tokens (QUIC anti-amplification)
- Registration ACK (currently fire-and-forget)
- Connection ID rotation
- Address validation during handshake

---

## References

- Current TLS: self-signed certs in `certs/` directory
- Deferred from Task 006 Phase 3: TLS cert management
- Deferred from `_context/README.md`: Registration ACK, rate limiting
- QUIC RFC 9000 Section 8: Address Validation
- Let's Encrypt ACME: https://letsencrypt.org/docs/
