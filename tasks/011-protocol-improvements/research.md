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

## Oracle Review Findings (Assigned to This Task)

From `oracle-review-01.md`, verified by Codex Oracle (gpt-5.3-codex, xhigh) on 2026-02-26.

**Note:** Findings 6 (IPv6 QAD panic) and 8 (predictable P2P identifiers) were originally assigned to this task but have been resolved by Task 015 (Oracle Quick Fixes) as quick mitigations. Full IPv6 QAD support remains in this task's Phase 3.

### Finding 9 (Medium): DATAGRAM Size Mismatch

- **Severity:** Medium
- **Component:** All Rust crates
- **Location:** `core/packet_processor/src/lib.rs:37`, `intermediate-server/src/main.rs:55`, `app-connector/src/main.rs:36`
- **Current code:** All three crates define `MAX_DATAGRAM_SIZE = 1350`. Tests documented effective writable limit of ~1307 bytes in `tasks/_context/components.md:171`.
- **Oracle assessment:** Constants are aligned at 1350, but the effective writable limit (after QUIC framing overhead) may be lower. Risks `BufferTooShort` errors at boundary when payloads approach 1350 bytes.
- **Investigation needed:** Measure actual `dgram_max_writable_len()` during live connections across different QUIC negotiation outcomes. The ~1307 figure from tests may vary with MTU discovery.
- **Proposed fix:** Either reduce `MAX_DATAGRAM_SIZE` to match observed limit, or query `dgram_max_writable_len()` dynamically and clamp payloads accordingly.

### Finding 10 (Medium): Interface Enumeration Endian Bug — DISPUTED

- **Severity:** Medium (may be false positive)
- **Component:** packet_processor
- **Location:** `core/packet_processor/src/p2p/candidate.rs:280`
- **Current code:** `let ip_bytes = (*sockaddr_in).sin_addr.s_addr.to_ne_bytes();`
- **Original claim:** `sin_addr.s_addr` is network byte order (big-endian per BSD socket API), so `to_ne_bytes()` reverses bytes on little-endian hosts, yielding incorrect IPs.
- **Oracle assessment:** Disputed the original finding. Oracle says `to_ne_bytes()` is likely **correct** here. On macOS/BSD, `sin_addr.s_addr` may already be in host byte order depending on the API path used to populate the struct. Blindly changing to `to_be_bytes()` could introduce a bug rather than fix one.
- **Investigation needed:** Trace exactly how `sin_addr.s_addr` is populated in the `getifaddrs()` path on macOS. Check whether the struct stores network-order or host-order. Test on actual hardware — compare gathered candidate IPs against known interface IPs.
- **Action:** Do NOT blindly change `to_ne_bytes()` → `to_be_bytes()`. Investigate first. If the current code produces correct candidate IPs on Apple Silicon (little-endian), it's correct as-is.

### Finding 13 (Low): Hot-Path Per-Packet Allocations

- **Severity:** Low
- **Component:** packet_processor, app-connector
- **Locations:**
  - `core/packet_processor/src/lib.rs:362` — `data.to_vec()` per packet
  - `core/packet_processor/src/lib.rs:395, 411` — `vec![0u8; MAX_DATAGRAM_SIZE]` per send/recv
  - `core/packet_processor/src/lib.rs:849` — `buf.to_vec()` in DATAGRAM handling
  - `app-connector/src/main.rs:792` — `vec![0u8; 65535]` in `process_quic_socket`
- **Risk:** Low at current traffic levels. Network Extensions have ~50MB memory limit and queue bounds (MAX_QUEUED_DATAGRAMS = 4096). Allocations are per-poll-iteration, not truly per-packet in most paths.
- **Oracle assessment:** Confirmed still open. This is a refactor, not a one-liner — requires introducing reusable buffers across multiple hot paths.
- **Proposed fix:** Pre-allocate buffers in the `Agent` struct constructor and reuse across poll iterations. Consider using a buffer pool pattern for variable-size allocations.

---

## References

- QAD protocol: `[0x01][4 bytes IP][2 bytes port]` (IPv4 only)
- TCP proxy: `app-connector/src/main.rs` TcpSession struct
- Shared socket: `quic_socket` on port 4434 in Connector
- P2P warm-up: ~1.8s observed in Phase 6.8 testing
- Deferred from `_context/components.md`: TCP flow control, separate sockets
- Deferred from `_context/README.md`: IPv6, QUIC migration
- Oracle findings triage: `tasks/015-oracle-quick-fixes/research.md`
