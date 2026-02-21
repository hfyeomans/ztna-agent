# Placeholder: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Document intentional placeholder/scaffolding code related to security hardening that exists in the codebase from the MVP implementation.

---

## Known Placeholders

| File | Line | Description | Status | Action |
|------|------|-------------|--------|--------|
| `certs/` | — | Self-signed development certificates | Active | Replace with Let's Encrypt |
| `intermediate-server/src/main.rs` | — | No client authentication on connection | Active | Add mTLS or token validation |
| `intermediate-server/src/main.rs` | — | No rate limiting | Active | Add per-IP rate limits |
| `intermediate-server/src/registry.rs` | — | Registration is fire-and-forget (no ACK) | Active | Add Registration ACK protocol |
| `app-connector/src/main.rs` | — | `--insecure` flag bypasses cert verification | Active | Remove for production |
