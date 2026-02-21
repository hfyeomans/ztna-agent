# Placeholder: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Document intentional placeholder/scaffolding code related to protocol improvements that exists in the codebase from the MVP implementation.

---

## Known Placeholders

| File | Line | Description | Status | Action |
|------|------|-------------|--------|--------|
| `intermediate-server/src/main.rs` | — | QAD only supports IPv4 (4+2 byte format) | Active | Extend for IPv6 (16+2) |
| `app-connector/src/main.rs` | — | TCP proxy forwards without window flow control | Active | Add back-pressure |
| `app-connector/src/main.rs` | — | Single `quic_socket` for P2P + relay | Active | Separate sockets |
| `core/packet_processor/src/lib.rs` | — | Full QUIC handshake on every connect (no 0-RTT) | Active | Add session ticket support |
| `core/packet_processor/src/p2p/hole_punch.rs` | — | ~1.8s warm-up before P2P active | Active | Optimize warm-up |
| `core/packet_processor/src/p2p/candidate.rs` | — | IPv4-only candidate gathering | Active | Add IPv6 candidates |
