# Plan: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan the implementation of production-grade TLS, client authentication, rate limiting, and protocol hardening. This is the highest-priority post-MVP task as it addresses the security foundation for all other production work.

---

## Phases (To Be Defined)

### Phase 1: TLS Certificate Management
- Replace self-signed certs with Let's Encrypt
- Implement automatic renewal
- Certificate trust chain for macOS Agent

### Phase 2: Client Authentication
- mTLS or token-based Agent authentication
- Connector authentication
- Registration authorization

### Phase 3: Rate Limiting & DDoS Protection
- Per-IP rate limiting on Intermediate
- Registration flood protection
- DATAGRAM throughput caps

### Phase 4: Protocol Hardening
- Stateless retry tokens
- Registration ACK protocol
- Connection ID management

---

## Success Criteria

- [ ] No self-signed certificates in production
- [ ] Unauthorized clients cannot register
- [ ] Intermediate Server withstands basic DDoS/flood
- [ ] Registration messages are acknowledged (not fire-and-forget)
