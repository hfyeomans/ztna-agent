# ZTNA Testing & Demo Guide

**Last Updated:** 2026-01-19
**Status:** Phase 5 Complete (Reliability Tests)

---

## Quick Start Demo

### 1. Build All Components

```bash
cd /Users/hank/dev/src/agent-driver/ztna-agent

# Build Intermediate Server
(cd intermediate-server && cargo build --release)

# Build App Connector
(cd app-connector && cargo build --release)

# Build Test Fixtures
(cd tests/e2e/fixtures/echo-server && cargo build --release)
(cd tests/e2e/fixtures/quic-client && cargo build --release)
```

### 2. Start Components (Manual)

```bash
# Terminal 1: Echo Server (test service)
tests/e2e/fixtures/echo-server/target/release/udp-echo --port 9999

# Terminal 2: Intermediate Server
# Note: E2E tests use certs/ at project root (see tests/e2e/config/env.local)
RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
  certs/cert.pem certs/key.pem

# Terminal 3: App Connector
RUST_LOG=info app-connector/target/release/app-connector \
  --server 127.0.0.1:4433 \
  --service test-service \
  --forward 127.0.0.1:9999
```

### 3. Run E2E Demo

```bash
# Send "HELLO" through the relay to echo server and back
tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --send-udp "HELLO_FROM_DEMO" \
  --dst 127.0.0.1:9999 \
  --wait 3000
```

**Expected output:**
```
[INFO] Connection established!
[INFO] Registering as Agent for service: test-service
[INFO] Built IP/UDP packet: 43 bytes (payload: 15 bytes)
[INFO] Received DATAGRAM: 43 bytes
RECV:4500002b...48454c4c4f5f46524f4d5f44454d4f
```

---

## Automated Test Suites

### Phase 1: Infrastructure Tests

```bash
# Run full MVP test suite (14 tests)
tests/e2e/run-mvp.sh
```

**Tests included:**
- Component startup/shutdown
- Direct UDP echo (bypasses relay)
- Port configuration validation
- Basic connectivity checks

### Phase 2 & 3.5: Protocol Validation Tests

```bash
# Run protocol validation suite (14 tests)
tests/e2e/scenarios/protocol-validation.sh
```

**Tests included:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| ALPN correct | Connect with `ztna-v1` | Connection established |
| ALPN wrong | Connect with wrong ALPN | Connection rejected |
| DATAGRAM at limit | Programmatic `max-1` sizing | Accepted + E2E verified |
| DATAGRAM over limit | Programmatic `max+1` sizing | BufferTooShort |
| Registration valid | `[0x10][len][id]` format | Accepted |
| Registration invalid | Malformed length | Handled gracefully |
| Zero-byte payload | Empty payload relay | OK |
| One-byte payload | Minimal payload relay | Echoed |
| Connector registration | `[0x11][len][id]` format | Accepted |
| Zero-length service ID | Empty ID (negative) | Handled gracefully |
| Overlong service ID | >255 bytes (negative) | Rejected |
| Unknown opcode | `0xFF` opcode | Handled gracefully |
| Multiple datagrams | Back-to-back sends | All queued |
| Malformed IP header | Non-UDP protocol | Dropped |

### Phase 4: Advanced UDP Tests

```bash
# Run advanced UDP test suite (11 tests)
tests/e2e/scenarios/udp-advanced.sh
```

**Tests included:**

**4.2 Echo Integrity Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| All-zeros payload | 64-byte zeros pattern | Echoed + verified |
| All-ones payload | 64-byte 0xFF pattern | Echoed + verified |
| Sequential payload | 256-byte 0x00..0xFF | Echoed + verified |
| Random payload | 128-byte random | Echoed + verified |
| Multiple payloads | 5 packets, 500ms delay | Multiple echoes |

**4.3 Concurrent Flow Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Parallel clients | 3 simultaneous clients | All receive responses |
| Flow isolation | Different source addresses | Independent flows |

**4.4 Long-Running Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Stream stability | 10 packets, 500ms interval | ≥80% success |
| Burst stress | 50 packets rapid-fire | All sent |
| Idle timeout | 5s idle within 30s limit | Connection alive |

### Phase 5: Reliability Tests

```bash
# Run reliability test suite (11 tests)
tests/e2e/scenarios/reliability-tests.sh
```

**Tests included:**

**5.1 Component Restart Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Intermediate restart | Stop/restart server, reconnect | Connectivity restored |
| Connector restart | Stop/restart connector | Data flow resumes |
| Active flow restart | Restart connector during stream | Partial delivery (≥1 packet) |

**5.2 Error Condition Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Unknown service ID | Send to non-existent service | No data echo (QAD only) |
| Unknown destination | Send to TEST-NET address | No data echo |
| Invalid certificates | Start server with bad cert path | Server refuses to start |
| Non-listening port | Connect to port 59999 | Connection fails/timeout |
| Rapid reconnection | 5 connections in 2 seconds | All succeed |

**5.3 Network Impairment Tests (Stretch):**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Packet loss | Simulate with pfctl/tc | Skipped (requires root) |
| Packet reorder | Simulate with tc netem | Skipped (requires root) |
| NAT rebinding | Port change simulation | Skipped (needs namespace) |

---

## Test Component Reference

### QUIC Test Client

**Location:** `tests/e2e/fixtures/quic-client/`

**Usage:**
```bash
quic-test-client [OPTIONS]

Options:
  --server ADDR      Intermediate server (default: 127.0.0.1:4433)
  --service ID       Register as Agent for service
  --send TEXT        Send raw text as DATAGRAM
  --send-hex HEX     Send hex-encoded data
  --send-udp TEXT    Send text wrapped in IP/UDP packet
  --dst IP:PORT      Destination for --send-udp
  --src IP:PORT      Source for --send-udp (default: 10.0.0.100:12345)
  --wait MS          Wait time for responses (default: 2000)

Protocol Validation (Phase 2):
  --alpn PROTO       Override ALPN (default: ztna-v1)
  --payload-size N   Generate N-byte payload (or 'max', 'max-1', 'max+1')
  --expect-fail      Expect connection to fail

Phase 3.5 - Programmatic DATAGRAM Sizing:
  --query-max-size   Print MAX_DGRAM_SIZE and MAX_UDP_PAYLOAD after connection

Phase 4 - Advanced Testing:
  --payload-pattern P  Payload pattern: zeros, ones, sequential, random
  --repeat N           Send N packets (default: 1)
  --delay MS           Delay between packets in repeat mode (default: 0)
  --burst N            Burst mode: send N packets as fast as possible
  --verify-echo        Verify echoed responses match sent data
```

**Examples:**
```bash
# Full E2E relay test
quic-test-client --service test-service --send-udp "Hello" --dst 127.0.0.1:9999

# ALPN negative test
quic-test-client --alpn "wrong" --expect-fail

# Boundary test (programmatic max)
quic-test-client --service test-service --payload-size max-1 --dst 127.0.0.1:9999

# Phase 4: Echo integrity with random payload
quic-test-client --service test-service --payload-size 100 --payload-pattern random \
  --dst 127.0.0.1:9999 --verify-echo

# Phase 4: Burst stress test (50 packets)
quic-test-client --service test-service --burst 50 --payload-size 100 --dst 127.0.0.1:9999
```

### UDP Echo Server

**Location:** `tests/e2e/fixtures/echo-server/`

**Usage:**
```bash
udp-echo --port 9999
```

Echoes back any UDP payload received.

---

## Log Locations

| Component | Log File |
|-----------|----------|
| Intermediate Server | `tests/e2e/artifacts/logs/intermediate-server.log` |
| App Connector | `tests/e2e/artifacts/logs/app-connector.log` |
| Echo Server | `tests/e2e/artifacts/logs/echo-server.log` |
| QUIC Test Client | `tests/e2e/artifacts/logs/quic-client.log` |

**View logs in real-time:**
```bash
# All components
tail -f tests/e2e/artifacts/logs/*.log

# Specific component with color
RUST_LOG=debug intermediate-server/target/release/intermediate-server ...
```

**Log levels:**
- `error` - Errors only
- `warn` - Warnings and errors
- `info` - Standard operation (default)
- `debug` - Detailed flow
- `trace` - QUIC packet-level detail

---

## Test Framework Reference

### Common Functions (lib/common.sh)

```bash
source tests/e2e/lib/common.sh

# Component lifecycle
start_intermediate           # Start with logging
start_connector             # Start with service ID
start_echo_server           # Start UDP echo
stop_all_components         # Clean shutdown

# Test helpers
test_start "Test name"      # Log test start
test_pass "Message"         # Log success
test_fail "Message"         # Log failure
test_warn "Message"         # Log warning

# QUIC helpers
send_via_quic "$data" "$server" "$wait_ms"
send_hex_via_quic "$hex" "$server" "$wait_ms"
```

### Environment Configuration

**File:** `tests/e2e/config/env.local`

```bash
# Network
INTERMEDIATE_HOST="127.0.0.1"
INTERMEDIATE_PORT="4433"
ECHO_SERVER_PORT="9999"

# Protocol
ALPN_PROTOCOL="ztna-v1"
MAX_DATAGRAM_SIZE="1350"  # Note: effective is ~1307

# Service
TEST_SERVICE_ID="test-service"

# Certificates
CERT_DIR="$PROJECT_ROOT/certs"
```

---

## Key Protocol Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `ALPN_PROTOCOL` | `b"ztna-v1"` | QUIC ALPN identifier |
| `MAX_DATAGRAM_SIZE` | 1350 | Config value |
| `EFFECTIVE_MAX` | ~1307 | Actual limit (QUIC overhead) |
| `IDLE_TIMEOUT_MS` | 30000 | 30 seconds |
| `Agent Registration` | `0x10` | `[0x10][len][service_id]` |
| `Connector Registration` | `0x11` | `[0x11][len][service_id]` |
| `QAD Observed Address` | `0x01` | `[0x01][4 bytes IP][2 bytes port]` |

---

## Relay Path Verification

**How tests verify traffic flows through the QUIC relay (not directly):**

### 1. Port Isolation
| Test Type | Destination Port | Path |
|-----------|-----------------|------|
| Baseline | 9999 | Client → Echo Server (direct UDP) |
| Tunneled | 4433 | Client → Intermediate → Connector → Echo Server |

The QUIC test client connects to port **4433** (Intermediate Server), not port 9999. Traffic only reaches the Echo Server after being relayed through the Connector.

### 2. Protocol Enforcement
- **Agent registration** (`0x10`): QUIC client registers with a service ID
- **Connector registration** (`0x11`): App Connector registers with matching service ID
- **Intermediate Server**: Only routes between matching Agent↔Connector pairs
- Without both registrations, data won't flow

### 3. IP Encapsulation
The `--send-udp` flag wraps payloads in IP/UDP headers:
```
QUIC DATAGRAM payload (42+ bytes):
  ├─ IPv4 Header (20 bytes): src=10.0.0.100, dst=127.0.0.1
  ├─ UDP Header (8 bytes): src_port=12345, dst_port=9999
  └─ Application Data (N bytes): "HELLO"
```

The Connector **must parse** these headers to extract and forward the inner UDP payload. This proves the relay path is active.

### 4. Dependency Verification
| Component Stopped | Baseline Test | Tunneled Test |
|-------------------|---------------|---------------|
| Echo Server | ❌ Fails | ❌ Fails |
| Intermediate | ✅ Works | ❌ Fails |
| Connector | ✅ Works | ❌ Fails |

If Intermediate or Connector are stopped, tunneled tests fail immediately, proving traffic depends on the relay.

### 5. Latency Evidence
- **Baseline RTT**: ~30-100 µs (direct UDP loopback)
- **Tunneled RTT**: ~300-500 µs (QUIC + relay overhead)

The ~200-400 µs overhead demonstrates the additional QUIC protocol processing and relay hops.

---

## Troubleshooting

### Connection Timeout

**Symptom:** `Connection timeout` after 5 seconds

**Causes:**
1. Intermediate Server not running
2. Wrong port (check 4433)
3. Firewall blocking UDP

**Debug:**
```bash
# Check server is listening
lsof -i :4433

# Check with trace logging
RUST_LOG=trace quic-test-client --server 127.0.0.1:4433
```

### ALPN Mismatch

**Symptom:** `Connection closed during handshake`

**Cause:** Client and server ALPN don't match

**Fix:** Ensure both use `ztna-v1`:
```bash
# Check client
quic-test-client --alpn "ztna-v1" ...

# Check server logs for ALPN
grep ALPN tests/e2e/artifacts/logs/intermediate-server.log
```

### BufferTooShort

**Symptom:** `Failed to queue DATAGRAM: BufferTooShort`

**Cause:** Payload exceeds ~1307 byte effective limit

**Fix:** Reduce payload size:
```bash
# Max safe payload for IP/UDP wrapped data
# IP (20) + UDP (8) + payload (1278) = 1306 bytes OK
quic-test-client --payload-size 1278 --dst 127.0.0.1:9999
```

### No Response from Echo Server

**Symptom:** `No DATAGRAMs received` after sending

**Causes:**
1. Echo server not running
2. App Connector not forwarding
3. Flow mapping issue (single flow only currently)

**Debug:**
```bash
# Check echo server
nc -u 127.0.0.1 9999
# Type "test" and press Enter - should echo back

# Check connector logs
tail tests/e2e/artifacts/logs/app-connector.log
```

---

## Test Coverage Summary

| Phase | Tests | Status | Validates |
|-------|-------|--------|-----------|
| 1 | 14 | ✅ Complete | Component lifecycle, direct UDP |
| 1.5 | 1 | ✅ Complete | Full E2E relay path |
| 2 | 8 | ✅ Complete | ALPN, boundaries, registration |
| 3 | 5 | ✅ Complete | Relay validation, connectivity |
| 3.5 | 6 | ✅ Complete | Coverage gaps (connector reg, malformed headers) |
| 4.2 | 5 | ✅ Complete | Echo integrity (payload patterns) |
| 4.3 | 2 | ✅ Complete | Concurrent flows, isolation |
| 4.4 | 3 | ✅ Complete | Long-running, burst, idle timeout |
| 5.1 | 3 | ✅ Complete | Component restart behavior |
| 5.2 | 5 | ✅ Complete | Error conditions (invalid certs, ports) |
| 5.3 | 3 | ⚠️ Skipped | Network impairment (requires root) |
| 6.1 | 2 | ✅ Complete | Latency (baseline vs tunneled RTT, percentiles) |
| 6.2 | 1 | ✅ Complete | Throughput (PPS, Mbps) |
| 6.3 | 3 | ✅ Complete | Timing (handshake, resources, reconnect) |

**Total Tests: 61+**

---

## Phase 6: Performance Metrics

Run performance benchmarks:
```bash
tests/e2e/scenarios/performance-metrics.sh
```

**Configurable via environment:**
```bash
RTT_SAMPLES=100 BURST_COUNT=500 tests/e2e/scenarios/performance-metrics.sh
```

**Key Metrics Collected:**

| Metric | Description | Typical Value |
|--------|-------------|---------------|
| `BASELINE_RTT_*` | Direct UDP to echo server | 30-100 µs |
| `TUNNELED_RTT_*` | Through QUIC relay | 300-500 µs |
| `THROUGHPUT_PPS` | Packets per second (burst) | 200K-400K |
| `THROUGHPUT_MBPS` | Megabits per second | 2-4 Gbps (theoretical) |
| `HANDSHAKE_*` | QUIC connection setup | 750-900 µs |
| `*_MEM_KB` | Memory usage per component | 5-7 MB |

**Output:** `tests/e2e/artifacts/metrics/perf_YYYYMMDD_HHMMSS.txt`

---

## Next Steps

After running the demo, you can:

1. **Explore logs** - See packet flow through components
2. **Modify tests** - Add scenarios in `tests/e2e/scenarios/`
3. **Run performance tests** - `tests/e2e/scenarios/performance-metrics.sh`
4. **Deploy to cloud** - See Task 006 for cloud deployment

---

## File Reference

| Purpose | Path |
|---------|------|
| Test framework | `tests/e2e/lib/common.sh` |
| Test runner (Phase 1) | `tests/e2e/run-mvp.sh` |
| Protocol validation (Phase 2 & 3.5) | `tests/e2e/scenarios/protocol-validation.sh` |
| Advanced UDP tests (Phase 4) | `tests/e2e/scenarios/udp-advanced.sh` |
| Reliability tests (Phase 5) | `tests/e2e/scenarios/reliability-tests.sh` |
| Performance metrics (Phase 6) | `tests/e2e/scenarios/performance-metrics.sh` |
| QUIC test client | `tests/e2e/fixtures/quic-client/` |
| Echo server | `tests/e2e/fixtures/echo-server/` |
| Environment config | `tests/e2e/config/env.local` |
| Logs | `tests/e2e/artifacts/logs/` |
| Test certificates | `certs/` |
