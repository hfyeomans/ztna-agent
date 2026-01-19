# Task State: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Status:** In Progress - Phase 1.5 Complete, E2E Relay Verified
**Branch:** `feature/004-e2e-relay-testing`
**Last Updated:** 2026-01-19

---

## Overview

Comprehensive end-to-end testing of the relay infrastructure. Validates that traffic flows correctly: Agent → Intermediate → Connector → Local Service and back.

**Important:** App Connector is **UDP-only** (TCP support deferred). All tests must account for this constraint.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Phase 1.5 Complete - E2E Relay Verified ✅

### Latest Test Run (2026-01-19)

```
=== E2E Relay Test Results ===

Test Command:
  quic-test-client --service test-service --send-udp "HELLO_E2E_TEST" --dst 127.0.0.1:9999

Flow:
  1. QUIC Client (Agent) → Intermediate Server: 42-byte IP/UDP packet
  2. Intermediate Server → App Connector: Relayed 42 bytes
  3. App Connector → Echo Server: Extracted 14-byte payload ("HELLO_E2E_TEST")
  4. Echo Server → App Connector: Echoed 14 bytes back
  5. App Connector → Intermediate Server: Re-encapsulated as 42-byte IP/UDP packet
  6. Intermediate Server → QUIC Client: Relayed 42 bytes back

Result: ✅ SUCCESS - Full round-trip verified
```

### What Was Built

#### QUIC Test Client (`tests/e2e/fixtures/quic-client/`)
- Rust binary using `quiche` crate
- Supports Agent registration (`--service <id>`)
- Raw DATAGRAM sending (`--send`, `--send-hex`)
- **IP/UDP packet construction** (`--send-udp --dst ip:port --src ip:port`)
- Proper IPv4 header checksum calculation (RFC 1071)
- Response receiving and hex display

#### Bug Fixes Applied

1. **App Connector: Initial QUIC Handshake Not Sent**
   - **Problem:** After `quiche::connect()`, the initial handshake packet was queued but never flushed to the network. The event loop blocked waiting for events that never arrived.
   - **Fix:** Added `self.send_pending()?` immediately after `self.connect()` in `app-connector/src/main.rs:207`

2. **App Connector: Local Socket Not Registered with mio Poll**
   - **Problem:** The `local_socket` was using `std::net::UdpSocket` (not registered with mio). When the Echo Server responded, `poll.poll()` never woke up because it was only watching the QUIC socket. Return traffic sat unprocessed.
   - **Fix:** Changed `local_socket` to `mio::net::UdpSocket` and registered it with the poll instance. Also added `LOCAL_SOCKET_TOKEN` event handling in the main event loop.

---

## Phase Summary

### Phase 1 - Test Infrastructure ✅ COMPLETE
- Created `tests/e2e/` directory structure
- Implemented `lib/common.sh` with component lifecycle helpers (zsh compatible)
- Implemented `run-mvp.sh` orchestrator script
- Created UDP echo server fixture (Rust)
- Created test scenario scripts (udp-connectivity, udp-echo, udp-boundary)
- Configured test environment and generated certificates
- **All 14 infrastructure tests passing**

### Phase 1.5 - QUIC Test Client & E2E Relay ✅ COMPLETE
- Built QUIC test client with IP/UDP packet construction
- Fixed App Connector bugs (handshake + local socket polling)
- **Verified full E2E relay path works**

### Test Coverage Now Achieved
| Category | Status | Notes |
|----------|--------|-------|
| Component startup | ✅ | Processes run without crashes |
| Echo server (direct) | ✅ | UDP port 9999 responds |
| QUIC relay path | ✅ | Data flows through Intermediate Server |
| Agent registration | ✅ | `0x10 + len + service_id` format |
| Connector registration | ✅ | `0x11 + len + service_id` format |
| IP/UDP parsing | ✅ | Connector extracts UDP payload |
| Return traffic | ✅ | Full round-trip working |

### What's NOT Tested Yet
- ❌ ALPN validation (wrong protocol rejection)
- ❌ MAX_DATAGRAM_SIZE boundary (1350 bytes)
- ❌ Connection recovery after timeout
- ❌ Multiple concurrent agents
- ❌ Various payload patterns

---

## Key Files

| File | Purpose |
|------|---------|
| `tests/e2e/fixtures/quic-client/src/main.rs` | QUIC test client with IP/UDP packet construction |
| `tests/e2e/fixtures/echo-server/main.rs` | UDP echo server for testing |
| `tests/e2e/lib/common.sh` | Component lifecycle helpers |
| `tests/e2e/run-mvp.sh` | Test orchestrator |
| `app-connector/src/main.rs` | UDP forwarding (fixed: handshake + poll) |
| `intermediate-server/src/main.rs` | DATAGRAM relay |

---

## How to Run E2E Test

```bash
cd /Users/hank/dev/src/agent-driver/ztna-agent

# 1. Start Echo Server
tests/e2e/fixtures/echo-server/udp-echo-server --port 9999 &

# 2. Start Intermediate Server
RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
  intermediate-server/certs/cert.pem intermediate-server/certs/key.pem &

# 3. Start App Connector
RUST_LOG=info app-connector/target/release/app-connector \
  --server 127.0.0.1:4433 --service test-service --forward 127.0.0.1:9999 &

# 4. Wait for components to initialize
sleep 2

# 5. Run E2E Test
RUST_LOG=info tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --send-udp "HELLO_E2E_TEST" \
  --dst 127.0.0.1:9999 \
  --wait 3000

# Expected output includes:
#   Received DATAGRAM: 42 bytes
#   Hex: 4500002a...48454c4c4f5f4532455f54455354
#                   ^^^^^^^^^^^^^^^^^^^^^^^^^^ "HELLO_E2E_TEST"
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 001 (Agent) | ✅ Complete | QUIC client, QAD support |
| Task 002 (Intermediate) | ✅ Complete | QUIC server, DATAGRAM relay |
| Task 003 (Connector) | ✅ Complete | UDP-only forwarding, mio event loop |

---

## What's Next

1. **Phase 2: Protocol Validation**
   - ALPN wrong-protocol rejection test
   - MAX_DATAGRAM_SIZE boundary tests
   - Malformed packet handling

2. **Phase 3: Automated Test Suite**
   - Integrate QUIC test client into `run-mvp.sh`
   - Add relay test scenarios

3. **Phase 4: Edge Cases**
   - Multiple concurrent connections
   - Idle timeout behavior
   - Connection recovery

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/004-e2e-relay-testing`
5. Continue with Phase 2 protocol validation tests
