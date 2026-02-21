# Plan: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan the implementation of IPv6 support, TCP flow control, socket separation, QUIC migration, and P2P optimizations.

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
