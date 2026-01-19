# Implementation Plan: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Branch:** `feature/004-e2e-relay-testing`
**Depends On:** 001, 002, 003

---

## Goal

Validate the complete relay data path works correctly:
1. Agent intercepts traffic
2. Intermediate relays to Connector
3. Connector forwards to local service
4. Response returns through the same path

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

## Phase 1: Local Test Environment

### 1.1 Test Infrastructure
- [ ] Create `tests/e2e/` directory
- [ ] Docker Compose file for all components
- [ ] Shell scripts to start/stop components
- [ ] Log aggregation setup

### 1.2 Test Service
- [ ] Simple echo server (TCP + UDP)
- [ ] HTTP server for HTTP testing
- [ ] netcat-based manual testing

### 1.3 Configuration
- [ ] Test certificates
- [ ] Port assignments
- [ ] Routing configuration

---

## Phase 2: Basic Connectivity Tests

### 2.1 ICMP/Ping Test
- [ ] Agent routes ping to test IP
- [ ] Verify ping reaches Connector
- [ ] Verify ICMP response returns
- [ ] Measure round-trip time

### 2.2 UDP Test
- [ ] Send UDP packet through tunnel
- [ ] Verify reaches echo server
- [ ] Verify response returns

### 2.3 TCP Test
- [ ] Establish TCP connection through tunnel
- [ ] Send data, receive response
- [ ] Test connection close

---

## Phase 3: Protocol Tests

### 3.1 HTTP Test
- [ ] curl through tunnel to local HTTP server
- [ ] Verify request/response integrity
- [ ] Test various HTTP methods

### 3.2 Large Payload Test
- [ ] Transfer file larger than MTU
- [ ] Verify fragmentation/reassembly
- [ ] Verify data integrity

### 3.3 Concurrent Connections
- [ ] Multiple simultaneous connections
- [ ] Verify isolation
- [ ] Check for race conditions

---

## Phase 4: Reliability Tests

### 4.1 Connection Recovery
- [ ] Restart Intermediate, verify reconnect
- [ ] Restart Connector, verify reconnect
- [ ] Network interruption simulation

### 4.2 Timeout Handling
- [ ] Slow server response
- [ ] No server response
- [ ] Verify cleanup

### 4.3 Error Conditions
- [ ] Invalid packets
- [ ] Unknown destinations
- [ ] Resource exhaustion

---

## Phase 5: Performance Tests

### 5.1 Latency Measurement
- [ ] Baseline (no tunnel)
- [ ] With relay tunnel
- [ ] Calculate overhead

### 5.2 Throughput Test
- [ ] Maximum sustainable throughput
- [ ] Compare to baseline

### 5.3 Connection Setup Time
- [ ] Time to first byte
- [ ] Cold start vs warm

---

## Phase 6: NAT Testing (Optional)

### 6.1 Cloud Deployment
- [ ] Deploy Intermediate to cloud (public IP)
- [ ] Test Agent behind home NAT
- [ ] Verify QAD reports correct public IP

### 6.2 Different NAT Types
- [ ] Full cone NAT
- [ ] Restricted NAT
- [ ] Port-restricted NAT

---

## Success Criteria

1. [ ] Ping works end-to-end
2. [ ] TCP connections work (HTTP curl succeeds)
3. [ ] UDP traffic works (DNS simulation)
4. [ ] Latency overhead < 50ms locally
5. [ ] No data corruption
6. [ ] Clean recovery from failures

---

## Test Scripts Structure

```
tests/
├── e2e/
│   ├── docker-compose.yml    # Full test environment
│   ├── setup.sh              # Initialize test env
│   ├── teardown.sh           # Cleanup
│   ├── run-all.sh            # Run all tests
│   ├── test-ping.sh          # ICMP test
│   ├── test-http.sh          # HTTP test
│   ├── test-udp.sh           # UDP test
│   └── test-performance.sh   # Performance tests
└── fixtures/
    ├── echo-server/          # Simple test server
    └── test-data/            # Test payloads
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Flaky tests | Add retries, increase timeouts |
| Port conflicts | Use dynamic port allocation |
| Platform differences | Test on CI with multiple platforms |

---

## References

- [Docker Compose networking](https://docs.docker.com/compose/networking/)
- [iperf3 for throughput testing](https://iperf.fr/)
