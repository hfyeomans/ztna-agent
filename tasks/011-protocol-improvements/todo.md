# TODO: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation tasks for protocol improvements including IPv6, TCP flow control, socket separation, and QUIC optimizations.

---

## Phase 1: Separate P2P/Relay Sockets

- [ ] Create separate UDP socket for relay QUIC connection in Connector
- [ ] Keep existing socket for P2P QUIC
- [ ] Update port allocation (e.g., 4434 for P2P, 4435 for relay)
- [ ] Update firewall documentation
- [ ] Test failover with global iptables rules (no longer need `-i ens5`)

## Phase 2: TCP Window Flow Control

- [ ] Track QUIC DATAGRAM send capacity in Connector
- [ ] Advertise TCP window size based on available capacity
- [ ] Implement back-pressure when DATAGRAM queue is full
- [ ] Test with large HTTP transfers
- [ ] Benchmark throughput improvement

## Phase 3: IPv6 QAD

- [ ] Extend QAD response format: `[0x01][1 byte version][IP bytes][2 bytes port]`
- [ ] Add IPv6 candidate gathering in P2P module
- [ ] Test with IPv6-only network
- [ ] Update quic-test-client for IPv6

## Phase 4: QUIC 0-RTT

- [ ] Implement session ticket storage in Agent
- [ ] Implement session ticket issuance in Intermediate
- [ ] Use 0-RTT on reconnection
- [ ] Add replay protection
- [ ] Measure reconnection time improvement

## Phase 5: P2P Warm-Up Optimization

- [ ] Pre-establish P2P QUIC parameters during relay phase
- [ ] Optimize candidate gathering (parallel probes)
- [ ] Target: P2P active within 500ms of hole punch start
- [ ] Benchmark warm-up time

## Phase 6: Multiplexed Streams

- [ ] Evaluate QUIC streams vs DATAGRAM for TCP transport
- [ ] Implement stream-based reliable channel for control messages
- [ ] Performance comparison: streams vs DATAGRAM for HTTP

## Oracle Findings (Cross-Cutting)

### Finding 9 (Medium): DATAGRAM Size Mismatch
- [ ] Measure `dgram_max_writable_len()` during live connections across different MTU scenarios
- [ ] Compare observed limit (~1307) against `MAX_DATAGRAM_SIZE` (1350)
- [ ] Decide: reduce constant or query dynamically and clamp payloads
- [ ] Update all 3 crates if constant changes
- [ ] Test: payload at boundary size sends without `BufferTooShort`

### Finding 10 (Medium): Endian Bug Investigation — DISPUTED
- [ ] Trace `getifaddrs()` path on macOS to determine byte order of `sin_addr.s_addr`
- [ ] Compare gathered candidate IPs against known interface IPs on Apple Silicon
- [ ] If `to_ne_bytes()` produces correct IPs: document finding as false positive
- [ ] If incorrect: fix to appropriate byte order conversion
- [ ] Do NOT change code without investigation — Oracle says current code may be correct

### Finding 13 (Low): Hot-Path Allocations
- [ ] Pre-allocate send/recv buffers in `Agent` struct constructor
- [ ] Reuse buffers across poll iterations in `agent_poll()` path
- [ ] Pre-allocate buffers in `Connector` struct for `process_quic_socket()`
- [ ] Benchmark: measure allocation reduction under sustained traffic
- [ ] Verify no regressions in Network Extension memory usage (50MB limit)
