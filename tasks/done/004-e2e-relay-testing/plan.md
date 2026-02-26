# Implementation Plan: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Branch:** `feature/004-e2e-relay-testing`
**Depends On:** 001, 002, 003
**Last Updated:** 2026-01-19 (Oracle review integrated)

---

## Goal

Validate the complete relay data path works correctly for **UDP traffic** (TCP deferred):
1. Agent intercepts traffic
2. Intermediate relays to Connector
3. Connector forwards to local service
4. Response returns through the same path

**Critical Constraint:** App Connector is UDP-only. All tests must account for this.

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/004-e2e-relay-testing

# While working:
git add . && git commit -m "004: descriptive message"

# When complete:
git push -u origin feature/004-e2e-relay-testing
# Create PR → Review → Merge to master
```

---

## Phase 1: Local Process Test Environment (MVP)

> **Decision:** Use local processes, NOT Docker Compose for MVP.
> - macOS Network Extension requires host execution
> - Docker Compose is optional for CI environments later

### 1.1 Test Directory Structure
```
tests/e2e/
├── run-mvp.sh              # Main orchestrator
├── lib/
│   └── common.sh           # Start/stop/wait/log helpers
├── scenarios/
│   ├── udp-echo.sh         # Basic echo test
│   ├── udp-boundary.sh     # Size boundary tests
│   ├── udp-concurrent.sh   # Concurrent flow tests
│   └── protocol-validation.sh  # ALPN/registration tests
├── config/
│   └── env.local           # Environment configuration
└── fixtures/
    └── echo-server/        # Simple UDP echo server
```

### 1.2 Test Infrastructure
- [ ] Shell scripts to start/stop components
- [ ] Log aggregation setup
- [ ] Cleanup handlers (trap for signals)
- [ ] Timeout wrappers for tests

### 1.3 Test Service
- [ ] Simple UDP echo server (primary for MVP)
- [ ] netcat-based manual testing

### 1.4 Configuration
- [ ] Test certificates (use existing dev certs)
- [ ] Port assignments (avoid conflicts)
- [ ] Routing configuration

---

## Phase 2: Protocol Validation Tests

> **Critical:** These tests validate core protocol invariants before other tests.

### 2.1 ALPN Validation
- [ ] Verify connection succeeds with ALPN `b"ztna-v1"`
- [ ] Verify connection fails with wrong/missing ALPN (negative test)

### 2.2 Registration Format
- [ ] Verify Connector registration succeeds with `[0x11][len][service_id]`
- [ ] Verify behavior with invalid length (negative test)
- [ ] Verify behavior with unknown service_id (negative test)

### 2.3 Datagram Size Enforcement
- [ ] Verify datagram at exactly 1350 bytes (MAX_DATAGRAM_SIZE) succeeds
- [ ] Verify datagram at 1351 bytes is rejected/dropped

---

## Phase 3: Basic UDP Connectivity Tests

### 3.1 Component Connectivity
- [ ] Agent connects to Intermediate
- [ ] Connector connects to Intermediate
- [ ] QAD works (both receive observed addresses)

### 3.2 DATAGRAM Relay
- [ ] Send UDP datagram through tunnel
- [ ] Verify reaches echo server via Connector
- [ ] Verify response returns through reverse path

---

## Phase 4: UDP Test Scenarios

### 4.1 Size Boundary Tests
| Size | Expected |
|------|----------|
| 0 bytes | Success (empty payload) |
| 1 byte | Success |
| 1350 bytes | Success (MAX_DATAGRAM_SIZE) |
| 1351 bytes | Drop/Reject |

### 4.2 Echo Integrity Tests
- [ ] Send various payload patterns
- [ ] Verify response matches request exactly
- [ ] Test patterns: random, sequential, all-zeros, all-ones

### 4.3 Concurrent Flow Tests
- [ ] Multiple simultaneous UDP flows
- [ ] Verify isolation between flows
- [ ] No cross-contamination of data

### 4.4 Long-Running Tests
- [ ] Long-lived UDP stream (10+ minutes)
- [ ] Burst traffic (high PPS)
- [ ] Idle timeout behavior (30s IDLE_TIMEOUT_MS)

---

## Phase 5: Reliability Tests

### 5.1 Component Restart
- [ ] Restart Intermediate, verify Agent/Connector behavior
- [ ] Restart Connector, verify traffic handling
- [ ] Test restart with active flows

### 5.2 Timeout Handling
- [ ] Verify idle timeout triggers at 30s
- [ ] Verify cleanup of stale flows

### 5.3 Error Conditions
- [ ] Invalid packets (malformed headers)
- [ ] Unknown destinations
- [ ] Invalid certificates (negative test)

### 5.4 Network Impairment (Stretch)
Using tc/netem or similar:
- [ ] Packet loss simulation (1%, 5%, 10%)
- [ ] Packet reorder simulation
- [ ] Packet duplication simulation
- [ ] Jitter simulation

---

## Phase 6: Performance Tests

### 6.1 Latency Measurement
- [ ] Baseline (no tunnel): direct UDP echo
- [ ] With relay tunnel: same echo through ZTNA
- [ ] Calculate overhead

**Expected Overhead (Local):**
| Component | Estimated Latency |
|-----------|-------------------|
| Agent processing | 1-5ms |
| QUIC encryption | 1-2ms |
| Intermediate relay | 5-10ms |
| QUIC decryption | 1-2ms |
| Connector processing | 1-5ms |
| **Total overhead** | **10-25ms locally** |

### 6.2 Throughput Test
- [ ] Maximum sustainable UDP throughput
- [ ] Packets per second (PPS)
- [ ] Compare to baseline

### 6.3 Key Metrics to Capture
| Metric | Method |
|--------|--------|
| Time to first datagram | Handshake timing |
| RTT p50/p95/p99 | Multiple samples |
| Jitter | RTT variance |
| Packet loss % | Send vs receive count |
| Throughput (Mbps) | Data volume / time |
| Throughput (PPS) | Packets / time |
| CPU/Memory | Per component |
| Reconnection time | After interruption |

---

## Phase 7: NAT Testing (Optional - Cloud)

> **Note:** Deferred to Task 006 (Cloud Deployment) or later phase.

### 7.1 Cloud Deployment
- [ ] Deploy Intermediate to cloud (public IP)
- [ ] Test Agent behind home NAT
- [ ] Verify QAD reports correct public IP

### 7.2 Different NAT Types
- [ ] Full cone NAT
- [ ] Restricted NAT
- [ ] Port-restricted NAT

---

## Success Criteria

### MVP Success (Must Pass)
1. [ ] Protocol validation: ALPN `b"ztna-v1"` enforced
2. [ ] Protocol validation: Registration format correct
3. [ ] Protocol validation: MAX_DATAGRAM_SIZE enforced
4. [ ] UDP echo works end-to-end
5. [ ] Multiple concurrent flows work
6. [ ] No data corruption
7. [ ] Latency overhead < 50ms locally

### Stretch Success
1. [ ] Graceful handling of component restarts
2. [ ] Performance metrics documented
3. [ ] NAT traversal works (cloud deployment)

---

## Deferred Tests (Require TCP Support)

| Test | Reason Deferred |
|------|-----------------|
| ICMP/Ping | Requires ICMP support |
| TCP connections | App Connector is UDP-only |
| HTTP via tunnel | Requires TCP support |
| Large file transfer | Requires app-layer segmentation or TCP |

---

## Test Scripts Structure

```
tests/
├── e2e/
│   ├── run-mvp.sh              # Main orchestrator
│   ├── lib/
│   │   └── common.sh           # Shared functions
│   ├── scenarios/
│   │   ├── protocol-validation.sh
│   │   ├── udp-echo.sh
│   │   ├── udp-boundary.sh
│   │   ├── udp-concurrent.sh
│   │   ├── reliability.sh
│   │   └── performance.sh
│   ├── config/
│   │   └── env.local
│   └── artifacts/
│       └── metrics/            # JSON/CSV output
└── fixtures/
    ├── echo-server/
    └── test-data/
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Flaky tests | Add retries, increase timeouts |
| Port conflicts | Use dynamic port allocation |
| Network Extension issues | Test on real macOS, not Docker |
| Performance variance | Multiple runs, statistical analysis |

---

## References

- [iperf3 for UDP throughput](https://iperf.fr/) (use `-u` flag)
- [tc/netem for network simulation](https://wiki.linuxfoundation.org/networking/netem)
- [tcpdump for packet capture](https://www.tcpdump.org/)
