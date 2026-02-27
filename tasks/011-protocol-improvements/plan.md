# Plan: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-26

---

## Purpose

Plan the implementation of IPv6 support, TCP flow control, socket separation, QUIC migration, and P2P optimizations.

---

## Oracle Review Findings (Assigned to This Task)

From `oracle-review-01.md` — must be addressed as part of this task:

| Severity | Finding | Evidence | Description |
|----------|---------|----------|-------------|
| ~~**High**~~ | ~~IPv6 QAD panic~~ | ~~`qad.rs:53`~~ | ~~Quick mitigation done in Task 015 (panic → `Option` return). Full IPv6 QAD support remains in Phase 3 below.~~ |
| ~~**Medium**~~ | ~~Predictable P2P identifiers~~ | ~~`signaling.rs:311`, `connectivity.rs:132`~~ | ~~Fixed in Task 015 — replaced time+PID with `ring::rand::SystemRandom`.~~ |
| **Medium** | DATAGRAM size mismatch | `lib.rs:37`, `main.rs:55`, `app-connector:36` | Constants aligned at 1350, but effective writable limit ~1307. Needs `dgram_max_writable_len()` investigation. |
| **Medium** | Interface enumeration endian bug — **DISPUTED** | `candidate.rs:280` | Oracle says `to_ne_bytes()` may be correct. Do NOT blindly change. Investigate `getifaddrs()` byte order on macOS. |
| **Low** | Hot-path per-packet allocations | `lib.rs:362,395,411,849`, `app-connector:792` | Pre-allocate buffers in struct constructors. Refactor, not one-liner. |

**Note:** Findings 6 and 8 resolved by Task 015 (Oracle Quick Fixes). Finding 10 disputed by Oracle — requires investigation before any code change. Finding 13 added from triage.

---

## Phases (To Be Defined)

### Phase 1: Separate P2P/Relay Sockets
- Connector uses different ports for P2P and relay
- Simplifies firewall testing and network policy
- Independent failure domains

### Phase 2: TCP Window Flow Control
- Back-pressure from QUIC to TCP window size
- Prevent sender overwhelming DATAGRAM capacity
- Per-session flow tracking

### Phase 3: IPv6 QAD
- Extend QAD response to support IPv6 addresses
- Dual-stack candidate gathering for P2P
- IPv6 NAT traversal testing

### Phase 4: QUIC 0-RTT Resumption
- Session ticket management
- 0-RTT connection establishment
- Security hardening against replay

### Phase 5: P2P Warm-Up Optimization
- Pre-warm P2P during relay phase
- Faster candidate gathering
- Reduced handshake time

### Phase 6: Multiplexed Streams
- QUIC streams for reliable control channel
- Stream-based TCP transport (vs DATAGRAM)
- Performance comparison

---

## Success Criteria

- [ ] Connector uses separate sockets for P2P and relay
- [ ] TCP connections don't overflow QUIC capacity
- [ ] IPv6 addresses supported in QAD and P2P
- [ ] Reconnection uses 0-RTT where possible
- [ ] P2P warm-up under 500ms
