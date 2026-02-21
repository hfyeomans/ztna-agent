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

| File | Line | Description | Finding | Status | Action |
|------|------|-------------|---------|--------|--------|
| `core/packet_processor/src/lib.rs` | 195 | `verify_peer(false)` — TLS disabled | C1 | Active | Enable peer verification |
| `app-connector/src/main.rs` | 415,440 | `verify_peer(false)` — TLS disabled | C1 | Active | Enable peer verification |
| `intermediate-server/src/main.rs` | 244 | `verify_peer(false)` — TLS disabled | C1 | Active | Enable peer verification |
| `certs/` | -- | Self-signed development certificates | H4 | Active | Replace with Let's Encrypt |
| `deploy/k8s/base/secrets.yaml` | 1-36 | Placeholder TLS certs/keys in repo | H4 | Active | Remove from base kustomization |
| `intermediate-server/src/main.rs` | 530-570 | No auth on service registration | H2 | Active | Add mTLS or token validation |
| `intermediate-server/src/registry.rs` | 57 | Silent Connector replacement on re-register | H2 | Active | Add auth + warning log |
| `intermediate-server/src/main.rs` | -- | No rate limiting | H1 | Active | Add per-IP rate limits |
| `app-connector/src/main.rs` | 1150 | Blocking TCP connect (500ms) | H3/L1 | Active | Migrate to non-blocking mio |
| `app-connector/src/main.rs` | -- | `--insecure` flag bypasses cert verification | C1 | Active | Remove for production |
| `core/packet_processor/src/lib.rs` | 582 | No queue depth limit on `received_datagrams` | H1 | Active | Add max queue constant |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | 47 | Hardcoded `3.128.36.92` default | M1 | Active | Use `0.0.0.0` placeholder |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | 264 | `parseIPv4` returns `[0,0,0,0]` on invalid input | M7 | Active | Return optional, fail explicitly |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | 735-752 | `hasRegistered` boolean for partial multi-service registration | L8 | Active | Track per-service registration state |
| `deploy/k8s/build-push.sh` | 147-156 | `--no-push` silently pushes on multi-platform | M8 | Active | Fail fast instead of silent push |
| `app-connector/src/main.rs` | 1207-1222 | TCP FIN removes session without half-close drain | L6 | Active | Implement half-close draining |
| `deploy/aws/aws-deploy-skill.md` | 141-143 | `StrictHostKeyChecking=no` in SSH guide | L9 | Active | Replace with ssh-keyscan approach |
