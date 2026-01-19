# TODO: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Branch:** `feature/004-e2e-relay-testing`
**Depends On:** Tasks 001, 002, 003
**Last Updated:** 2026-01-19 (Oracle review integrated)

---

## Prerequisites

- [x] Task 002 (Intermediate Server) complete and merged
- [x] Task 003 (App Connector) complete and merged
- [ ] Create feature branch: `git checkout -b feature/004-e2e-relay-testing`

---

## Phase 1: Local Process Test Environment (MVP)

> **Note:** Use local processes, NOT Docker Compose for MVP. macOS Network Extension requires host execution. Docker Compose is optional for CI later.

- [ ] Create `tests/e2e/` directory structure:
  ```
  tests/e2e/
  ├── run-mvp.sh           # Main orchestrator
  ├── lib/common.sh        # Start/stop/wait/log helpers
  ├── scenarios/           # Test scripts
  │   ├── udp-echo.sh
  │   ├── udp-boundary.sh
  │   └── udp-concurrent.sh
  └── config/env.local     # Environment config
  ```
- [ ] Write `lib/common.sh` with component lifecycle helpers
- [ ] Write `run-mvp.sh` orchestrator script
- [ ] Create simple UDP echo server for testing
- [ ] Configure test environment (ports, addresses, certs)

---

## Phase 2: Protocol Validation Tests

> **Critical:** These tests validate core protocol invariants.

- [ ] Test: ALPN validation (`b"ztna-v1"`)
  - [ ] Verify connection succeeds with correct ALPN
  - [ ] Verify connection fails with wrong ALPN (negative test)
- [ ] Test: Connector registration format `[0x11][len][service_id]`
  - [ ] Verify registration succeeds with valid format
  - [ ] Verify behavior with invalid length (negative test)
  - [ ] Verify behavior with unknown service_id (negative test)
- [ ] Test: MAX_DATAGRAM_SIZE boundary (1350 bytes)
  - [ ] Verify datagram at exactly 1350 bytes succeeds
  - [ ] Verify datagram at 1351 bytes is rejected/dropped

---

## Phase 3: Basic UDP Connectivity

- [ ] Test: Agent connects to Intermediate
- [ ] Test: Connector connects to Intermediate
- [ ] Test: QAD works (both receive observed addresses)
- [ ] Test: DATAGRAM relay works (Agent → Intermediate → Connector)
- [ ] Test: Return path works (Connector → Intermediate → Agent)

---

## Phase 4: UDP Test Scenarios

### 4.1 Size Boundary Tests
- [ ] 0-byte payload
- [ ] 1-byte payload
- [ ] 1350-byte payload (MAX_DATAGRAM_SIZE)
- [ ] 1351-byte payload (expect drop/reject)

### 4.2 Echo Integrity Tests
- [ ] Send UDP packet through tunnel to echo server
- [ ] Verify response matches request
- [ ] Test with various payload patterns (random, sequential, all-zeros)

### 4.3 Concurrent Flow Tests
- [ ] Multiple simultaneous UDP flows
- [ ] Different service_ids (when supported)
- [ ] Verify isolation between flows

### 4.4 Long-Running Tests
- [ ] Long-lived UDP stream (stability)
- [ ] Burst traffic (packets per second stress)
- [ ] Idle timeout behavior (30s IDLE_TIMEOUT_MS)

---

## Phase 5: Reliability Tests

### 5.1 Component Restart
- [ ] Restart Intermediate, verify reconnect behavior
- [ ] Restart Connector, verify reconnect behavior
- [ ] Test with active flows during restart

### 5.2 Error Conditions
- [ ] Invalid packets (malformed headers)
- [ ] Unknown destinations
- [ ] Invalid certificates (negative test)

### 5.3 Network Impairment (Stretch)
- [ ] Packet loss simulation
- [ ] Packet reorder simulation
- [ ] Packet duplication simulation
- [ ] NAT rebinding (port change)

---

## Phase 6: Performance Metrics

### 6.1 Latency
- [ ] Measure baseline (no tunnel)
- [ ] Measure tunneled RTT
- [ ] Calculate overhead
- [ ] Capture p50/p95/p99 percentiles

### 6.2 Throughput
- [ ] Measure baseline throughput
- [ ] Measure tunneled throughput (Mbps + PPS)
- [ ] Compare and document overhead

### 6.3 Timing
- [ ] Time to first datagram (handshake timing)
- [ ] Reconnection time after interruption
- [ ] Record CPU/memory per component

---

## Phase 7: Documentation

- [ ] Write test README with instructions
- [ ] Document test scenarios and expected results
- [ ] Document metrics collection
- [ ] Add troubleshooting guide

---

## Phase 8: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Deferred Tests (Post-MVP)

> **Reason:** App Connector is UDP-only. These require TCP support.

- [ ] ICMP/Ping tests (requires ICMP support)
- [ ] TCP connection tests (requires TCP support)
- [ ] HTTP tests via tunnel (requires TCP support)
- [ ] Large file transfer with fragmentation (requires app-layer segmentation)

---

## Stretch Goals (Optional)

- [ ] Docker Compose for CI (non-macOS environments)
- [ ] NAT testing with cloud Intermediate
- [ ] Automated CI integration
- [ ] Chaos engineering tests (tc/netem)
- [ ] Interface switch/sleep-wake behavior
