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
| `core/packet_processor/src/lib.rs` | 37 | `MAX_DATAGRAM_SIZE = 1350` may exceed effective writable limit (~1307) (Oracle Finding 9) | Active | Investigate and align or query dynamically |
| `core/packet_processor/src/p2p/candidate.rs` | 280 | `to_ne_bytes()` on `sin_addr.s_addr` — Oracle disputes original endian bug finding (Oracle Finding 10) | Active | Investigate byte order on macOS before changing |
| `core/packet_processor/src/lib.rs` | 362,395,411 | Per-poll-iteration buffer allocations in hot path (Oracle Finding 13) | Active | Pre-allocate in Agent struct |
| `app-connector/src/main.rs` | 792 | Per-poll `vec![0u8; 65535]` in `process_quic_socket` (Oracle Finding 13) | Active | Pre-allocate in Connector struct |
