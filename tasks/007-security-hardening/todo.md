# TODO: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation tasks for TLS certificates, client authentication, rate limiting, and protocol hardening.

---

## Phase 1: TLS Certificate Management

- [ ] Research Let's Encrypt ACME with QUIC/UDP (DNS-01 challenge likely needed)
- [ ] Implement cert auto-renewal on Intermediate Server
- [ ] Update Connector TLS configuration for production certs
- [ ] Update macOS Agent trust chain (remove self-signed cert trust)
- [ ] Test certificate rotation without connection drops

## Phase 2: Client Authentication

- [ ] Design auth approach (mTLS vs token-based)
- [ ] Implement client certificate or token validation on Intermediate
- [ ] Implement credential provisioning for Agents
- [ ] Implement credential provisioning for Connectors
- [ ] Test unauthorized client rejection

## Phase 3: Rate Limiting

- [ ] Add per-IP connection rate limiting on Intermediate
- [ ] Add registration flood protection
- [ ] Add DATAGRAM throughput limits per connection
- [ ] Test under simulated load

## Phase 4: Protocol Hardening

- [ ] Implement stateless retry tokens (QUIC anti-amplification)
- [ ] Implement Registration ACK (replace fire-and-forget)
- [ ] Implement connection ID rotation
- [ ] Update quic-test-client for auth testing
