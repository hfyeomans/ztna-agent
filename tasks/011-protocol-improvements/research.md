# Research: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research IPv6 support, TCP flow control improvements, QUIC migration, and P2P optimizations to improve protocol robustness and performance.

---

## Research Areas

### IPv6 QAD (Quick Address Discovery)
- Current QAD returns IPv4 only (4 bytes IP + 2 bytes port)
- Need: IPv6 address support (16 bytes IP + 2 bytes port)
- Dual-stack QAD responses
- IPv6 NAT traversal implications

### TCP Window Flow Control
- Current TCP proxy: simple forwarding without window management
- Problem: fast sender can overwhelm QUIC DATAGRAM capacity
- Need: proper TCP window advertisement based on QUIC congestion
- Back-pressure from Connector to Agent via TCP window size

### Separate P2P/Relay Sockets
- Current: Connector shares single `quic_socket` (port 4434) for P2P and relay QUIC
- Problem: interface-specific iptables needed for failover testing
- Solution: separate sockets for P2P and Intermediate relay
- Impact on firewall rules and port allocation

### QUIC Migration (0-RTT)
- Current: full QUIC handshake on reconnect
- 0-RTT resumption for faster reconnection
- Session ticket management
- Security implications of 0-RTT (replay attacks)

### Multiplexed Streams
- Current: DATAGRAM-only (unreliable)
- QUIC streams for reliable control channel
- Stream multiplexing for concurrent TCP connections
- Stream vs DATAGRAM decision matrix

### P2P Warm-Up Reduction
- Current: ~1.8s warm-up before P2P accepts traffic
- Optimize QUIC handshake for P2P connection
- Pre-warm P2P connection during relay phase
- Reduce candidate gathering time

---

## References

- QAD protocol: `[0x01][4 bytes IP][2 bytes port]` (IPv4 only)
- TCP proxy: `app-connector/src/main.rs` TcpSession struct
- Shared socket: `quic_socket` on port 4434 in Connector
- P2P warm-up: ~1.8s observed in Phase 6.8 testing
- Deferred from `_context/components.md`: TCP flow control, separate sockets
- Deferred from `_context/README.md`: IPv6, QUIC migration
