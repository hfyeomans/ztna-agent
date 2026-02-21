# State: Protocol Improvements

**Task ID:** 011-protocol-improvements
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track the current state of protocol improvements implementation.

---

## Current State

Not started. MVP uses IPv4-only QAD, simple TCP forwarding, shared sockets, and full QUIC handshake on reconnect.

### What Exists (from MVP)
- QAD: IPv4 only (`[0x01][4 bytes IP][2 bytes port]`)
- TCP proxy: simple forwarding without flow control
- Connector: shared `quic_socket` on port 4434 for both P2P and relay
- QUIC: full handshake on every connection (no 0-RTT)
- P2P: ~1.8s warm-up before direct path active
- Transport: DATAGRAM-only (no QUIC streams)

### What This Task Delivers
- IPv6 QAD and dual-stack P2P
- TCP window flow control with back-pressure
- Separate P2P and relay sockets
- 0-RTT QUIC resumption
- Reduced P2P warm-up time
- Optional QUIC streams for reliable transport

---

## Decisions Log

(No decisions yet — task not started)
