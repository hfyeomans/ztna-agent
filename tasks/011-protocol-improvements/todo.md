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
