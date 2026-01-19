# Research: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Last Updated:** 2026-01-19

---

## Purpose

Document research findings, test strategies, and discovered issues for comprehensive end-to-end testing.

---

## Key Discoveries (Phase 1.5)

### QUIC Sans-IO Model Gotchas

**Finding:** The `quiche` crate uses a "sans-IO" model where:
- `dgram_send()` queues data to an internal buffer
- `conn.send()` flushes queued data to an outgoing buffer
- The application must then send the buffer via socket

**Bug Found:** App Connector's `connect()` called `quiche::connect()` but never called `send_pending()` to flush the initial handshake. The event loop then blocked waiting for events that never arrived because the server never received the Client Hello.

**Fix:** Always call `send_pending()` or equivalent flush after connection initiation.

```rust
// WRONG - blocks forever waiting for handshake response
self.connect()?;
loop { self.poll.poll(...); }

// CORRECT - sends initial handshake before entering event loop
self.connect()?;
self.send_pending()?;  // <-- Critical!
loop { self.poll.poll(...); }
```

### mio Event Loop Integration

**Finding:** When using multiple sockets with mio, ALL sockets must be registered with the poll instance to receive events.

**Bug Found:** App Connector's `local_socket` was created as `std::net::UdpSocket` (not mio socket) and not registered with poll. When Echo Server responded, `poll.poll()` never woke up because it was only watching the QUIC socket.

**Fix:** Use `mio::net::UdpSocket` and register with poll:
```rust
let mut local_socket = UdpSocket::bind("0.0.0.0:0".parse()?)?;
poll.registry().register(&mut local_socket, LOCAL_SOCKET_TOKEN, Interest::READABLE)?;
```

### IP/UDP Packet Construction

**Finding:** The App Connector expects properly formatted IP packets (minimum 20 bytes for IPv4 header). Raw text sent as DATAGRAM payloads is rejected.

**Implementation:** QUIC test client now includes `build_ip_udp_packet()` function that constructs:
- IPv4 header (20 bytes) with proper checksum
- UDP header (8 bytes)
- Payload

**Key Constants:**
- IPv4 header: version=4, IHL=5, protocol=17 (UDP)
- Flags: DF (Don't Fragment)
- TTL: 64
- UDP checksum: 0 (optional in IPv4)

### Flow Mapping Simplification

**Current State:** App Connector uses simplified flow lookup:
```rust
let flow_key = self.flow_map.keys().next().cloned();
```

**Limitation:** Only works correctly with single active flow. Multiple concurrent flows would need proper 5-tuple matching.

**Deferred:** Proper flow tracking for multiple concurrent connections.

---

## Key Discoveries (Phase 2)

### Effective QUIC DATAGRAM Size Limit

**Finding:** The actual QUIC DATAGRAM payload limit is ~1307 bytes, NOT 1350 bytes.

**Test Results:**
```
IP header (20) + UDP header (8) + payload (1278) = 1306 bytes ✅ OK
IP header (20) + UDP header (8) + payload (1280) = 1308 bytes ❌ BufferTooShort
```

**Reason:** QUIC packet overhead includes:
- QUIC packet header (variable, ~20-30 bytes)
- DATAGRAM frame header (~2-3 bytes)
- AEAD authentication tag (16 bytes for AES-GCM)
- Padding for minimum packet size

**Implication:** When designing payloads, account for ~43 bytes of QUIC overhead:
- `MAX_DATAGRAM_SIZE` (1350) - QUIC overhead (~43) ≈ 1307 bytes effective payload

### ALPN Rejection Behavior

**Finding:** Server correctly rejects connections with wrong ALPN.

- Correct ALPN (`ztna-v1`): Connection established
- Wrong ALPN (`wrong-protocol`): Connection closed during handshake

**Mechanism:** QUIC handshake fails when ALPN negotiation fails (no common protocol).

### Malformed Registration Handling

**Finding:** Server handles malformed registration messages gracefully.

- Invalid length byte (claiming 255 bytes but only 4 present)
- Server does not crash
- Connection may be closed but no server errors

---

## Test Environment Options

### Option 1: Docker Compose

**Pros:**
- Isolated networking
- Reproducible
- Easy CI integration

**Cons:**
- macOS Agent requires host networking
- Network Extension complicates Docker

### Option 2: Local Processes ✅ SELECTED

**Pros:**
- Simple, no containers
- Direct testing of real components
- Easier debugging

**Cons:**
- Less isolated
- Manual cleanup needed

### Decision

**MVP: Local processes with scripts**
- Agent runs on host (requires Network Extension)
- Intermediate and Connector as local processes
- Scripts to manage lifecycle
- QUIC test client for automated relay testing

---

## Test Tools

### Networking

| Tool | Purpose | Status |
|------|---------|--------|
| `quic-test-client` | QUIC DATAGRAM relay testing | ✅ Built |
| `udp-echo-server` | Echo back UDP payloads | ✅ Built |
| `nc` (netcat) | Direct UDP testing | Used |
| `iperf3` | Throughput measurement | Planned |

### Monitoring

| Tool | Purpose |
|------|---------|
| `RUST_LOG=debug` | Component debug logs |
| `RUST_LOG=trace` | Detailed QUIC tracing |
| `tcpdump -i lo0 udp port 4433` | Packet capture |

---

## Test Scenarios

### Scenario 1: UDP Echo via Relay ✅ VERIFIED

```
QUIC Test Client (Agent role)
    │
    │ Register: 0x10 + "test-service"
    │ Send: IP/UDP packet with "HELLO" payload
    ▼
Intermediate Server (localhost:4433)
    │
    │ QUIC DATAGRAM relay
    ▼
App Connector
    │
    │ Parse IP/UDP, extract payload
    │ Forward to local service
    ▼
UDP Echo Server (localhost:9999)
    │
    │ Echo payload back
    ▼
App Connector
    │
    │ Build return IP/UDP packet
    │ Send via QUIC DATAGRAM
    ▼
Intermediate Server
    │
    │ Relay to Agent
    ▼
QUIC Test Client receives echoed data
```

**Verified:** 2026-01-19 - Full round-trip working

### Scenario 2: Protocol Boundary Tests (Planned)

- ALPN mismatch rejection
- MAX_DATAGRAM_SIZE (1350 bytes) enforcement
- Malformed packet handling

### Scenario 3: NAT Traversal (Deferred)

- Requires cloud deployment of Intermediate Server
- Test QAD with real NAT

---

## Latency Measurement

### Methodology

1. **Baseline:** Direct UDP to echo server (no tunnel)
2. **Tunneled:** Same via QUIC relay
3. **Overhead:** Tunneled - Baseline

### Expected Overhead (Local)

| Component | Estimated Latency |
|-----------|-------------------|
| QUIC encryption | 1-2ms |
| Intermediate relay | 1-5ms (local) |
| QUIC decryption | 1-2ms |
| IP/UDP parsing | <1ms |
| **Total overhead** | **5-10ms locally** |

---

## Error Scenarios

### Connection Failures

| Scenario | Expected Behavior |
|----------|-------------------|
| Intermediate down | Agent/Connector can't connect |
| Connector down | Traffic relayed but not forwarded |
| Echo server down | Connector forwards but no response |

### Protocol Errors

| Scenario | Expected Behavior | Status |
|----------|-------------------|--------|
| Wrong ALPN | Connection rejected | To test |
| Oversized DATAGRAM | Dropped by QUIC | To test |
| Malformed IP header | Dropped by Connector | To test |

---

---

## Oracle Review Findings (Phase 2)

**Date:** 2026-01-19

### Confirmed Correct Patterns

1. **quiche sans-IO pattern:** `quiche::connect()` followed by manual `flush()` is correct. The client calls `flush()` after `connect()` and in subsequent loops after processing.

2. **zsh arithmetic fix:** `: $((var += 1))` is the correct fix for `set -e` with zsh. `((var++))` returns old value as exit status, tripping errexit when it evaluates to 0.

3. **Programmatic DATAGRAM sizing:** quiche exposes `dgram_max_writable_len()` after handshake. Should use this to compute safe payload size and reduce flakiness across versions/MTU.

### Issues Found

**Medium Priority:**
- `protocol-validation.sh:150-154` uses hard-coded `test-service` instead of `$SERVICE_ID`
- `protocol-validation.sh:81-143` hard-codes datagram boundary sizes (brittle if QUIC overhead changes)

**Low Priority:**
- Boundary test only asserts "DATAGRAM queued" not actual relay (`RECV:`)
- `pkill -f` cleanup can kill unrelated processes on same host
- `nc -z -u` doesn't reliably confirm UDP service readiness
- Testing guide has wrong function names (`start_intermediate_server` vs `start_intermediate`)
- Testing guide has inconsistent cert paths (`intermediate-server/certs` vs `certs/`)

### Coverage Gaps

1. **Connector registration (0x11):** No validation tests (only agent 0x10 tested)
2. **Malformed IP/UDP headers:** No tests for bad checksum, non-UDP protocol, length mismatch
3. **Service ID edge cases:** No tests for zero-length or overlong (>255) service IDs
4. **Unknown opcode handling:** No test for unrecognized registration opcodes
5. **Multiple datagrams:** No test for back-to-back or interleaved send/recv

### Open Questions from Oracle Review

1. **End-to-end delivery assertion:** Should boundary tests assert `RECV:` (full relay verification) or is "DATAGRAM queued" (client-side acceptance) sufficient?
   - **Recommendation:** Assert `RECV:` for production confidence; current tests catch client-side issues only

2. **Canonical cert path:** Should scripts use `intermediate-server/certs/` or top-level `certs/`?
   - **Decision:** E2E tests use `certs/` at project root (per `common.sh:30`)
   - **Action:** Updated testing-guide.md to clarify

### Recommendations for Task 006 (Cloud Deployment)

When migrating to cloud services, plan for:
- Removing hard-coded IDs, addresses, ports
- Scalable configuration via environment/config files
- Dynamic certificate management (Let's Encrypt or cloud KMS)
- Service discovery for relay endpoints
- Token-based authentication for agents/connectors
- Multi-tenant service ID namespacing

---

## References

- [quiche documentation](https://docs.rs/quiche/)
- [RFC 9221 - QUIC Datagrams](https://www.rfc-editor.org/rfc/rfc9221)
- [RFC 1071 - IP Checksum](https://www.rfc-editor.org/rfc/rfc1071)
- [mio event loop](https://docs.rs/mio/)
