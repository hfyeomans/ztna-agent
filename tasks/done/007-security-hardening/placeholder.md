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

**Last reviewed:** 2026-02-28 (Oracle audit of all completed tasks)

| File | Line | Description | Finding | Status | Action |
|------|------|-------------|---------|--------|--------|
| ~~`core/packet_processor/src/lib.rs`~~ | ~~195~~ | ~~`verify_peer(false)` — TLS disabled~~ | ~~C1~~ | ~~Resolved~~ | ~~Task 007 made `verify_peer` default to `true`; `--no-verify-peer` for dev only~~ |
| ~~`app-connector/src/main.rs`~~ | ~~415,440~~ | ~~`verify_peer(false)` — TLS disabled~~ | ~~C1~~ | ~~Resolved~~ | ~~Task 007 changed default to `true`; configurable via `--no-verify-peer`~~ |
| ~~`intermediate-server/src/main.rs`~~ | ~~244~~ | ~~`verify_peer(false)` — TLS disabled~~ | ~~C1~~ | ~~Resolved~~ | ~~Task 007 changed default to `true`; configurable via `--no-verify-peer`~~ |
| `certs/` | -- | Self-signed development certificates | H4 | Deferred | Replace with Let's Encrypt for production; tracked in `_context/README.md` Priority 4 |
| `deploy/k8s/base/secrets.yaml` | -- | Placeholder TLS certs/keys in repo | H4 | Deferred | Remove from base kustomization; k8s manifests need production cert workflow |
| ~~`intermediate-server/src/main.rs`~~ | ~~530-570~~ | ~~No auth on service registration~~ | ~~H2~~ | ~~Resolved~~ | ~~Task 007 added mTLS (`--require-client-cert`) + SAN-based service authorization~~ |
| ~~`intermediate-server/src/registry.rs`~~ | ~~57~~ | ~~Silent Connector replacement on re-register~~ | ~~H2~~ | ~~Resolved~~ | ~~Task 007 added auth; mTLS validates before registration~~ |
| `intermediate-server/src/main.rs` | -- | No rate limiting | H1 | Deferred | Add per-IP rate limits; tracked in `_context/README.md` deferred items |
| ~~`app-connector/src/main.rs`~~ | ~~1150~~ | ~~Blocking TCP connect (500ms)~~ | ~~H3/L1~~ | ~~Resolved~~ | ~~Task 007 migrated to non-blocking `mio::net::TcpStream::connect()`~~ |
| ~~`app-connector/src/main.rs`~~ | ~~--~~ | ~~`--insecure` flag~~ | ~~C1~~ | ~~Resolved~~ | ~~Renamed to `--no-verify-peer`; `--insecure` no longer exists~~ |
| `core/packet_processor/src/lib.rs` | -- | No queue depth limit on `received_datagrams` | H1 | Deferred | Add max queue constant; prevents unbounded memory growth under load |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | -- | Hardcoded `3.128.36.92` default | M1 | Deferred | Use `0.0.0.0` or empty placeholder; low priority (UI overrides on launch) |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | -- | `parseIPv4` returns `[0,0,0,0]` on invalid input | M7 | Deferred | Return optional, fail explicitly; tracked for future Swift work |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | -- | `hasRegistered` boolean for partial multi-service registration | L8 | Deferred | Track per-service registration state; → Task 009 |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | -- | `verifyPeer` defaults to `false` | C1 | Deferred | Flip to `true` when production TLS certs available; tracked in `_context/README.md` |
| `deploy/k8s/build-push.sh` | -- | `--no-push` silently pushes on multi-platform | M8 | Deferred | Fail fast instead of silent push |
| ~~`app-connector/src/main.rs`~~ | ~~1207-1222~~ | ~~TCP FIN removes session without half-close drain~~ | ~~L6~~ | ~~Resolved~~ | ~~Task 007 implemented TCP half-close draining with `TCP_DRAIN_TIMEOUT_SECS = 5`~~ |
| `deploy/aws/aws-deploy-skill.md` | -- | `StrictHostKeyChecking=no` in SSH guide | L9 | Deferred | Replace with ssh-keyscan approach |

**Note:** Line numbers removed from resolved/deferred items — they've drifted since Task 007 and are no longer accurate. The items themselves are correctly tracked by description.
