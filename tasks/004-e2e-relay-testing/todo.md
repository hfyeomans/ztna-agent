# TODO: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Branch:** `feature/004-e2e-relay-testing`
**Depends On:** Tasks 001, 002, 003

---

## Prerequisites

- [ ] Task 002 (Intermediate Server) complete and merged
- [ ] Task 003 (App Connector) complete and merged
- [ ] Create feature branch: `git checkout -b feature/004-e2e-relay-testing`

---

## Phase 1: Test Environment Setup

- [ ] Create `tests/e2e/` directory structure
- [ ] Create Docker Compose for test environment
- [ ] Write setup.sh script
- [ ] Write teardown.sh script
- [ ] Create simple echo server for testing
- [ ] Generate test certificates

---

## Phase 2: Basic Connectivity

- [ ] Test: Agent connects to Intermediate
- [ ] Test: Connector connects to Intermediate
- [ ] Test: QAD works (both get observed addresses)
- [ ] Test: DATAGRAM relay works

---

## Phase 3: ICMP/Ping Test

- [ ] Configure Agent to route test IP through tunnel
- [ ] Start all components
- [ ] Run ping from Agent machine
- [ ] Verify ping reaches Connector
- [ ] Verify response returns
- [ ] Log round-trip time

---

## Phase 4: TCP Tests

- [ ] Start HTTP server on Connector side
- [ ] curl through tunnel from Agent
- [ ] Verify request received
- [ ] Verify response returned
- [ ] Test POST with payload
- [ ] Test large file download

---

## Phase 5: UDP Tests

- [ ] Start UDP echo server
- [ ] Send UDP packets through tunnel
- [ ] Verify echo response
- [ ] Test multiple concurrent UDP flows

---

## Phase 6: Reliability Tests

- [ ] Test Intermediate restart
- [ ] Test Connector restart
- [ ] Test network interruption
- [ ] Test timeout scenarios
- [ ] Verify error handling

---

## Phase 7: Performance Tests

- [ ] Measure baseline latency (no tunnel)
- [ ] Measure tunnel latency
- [ ] Calculate overhead
- [ ] Run throughput test
- [ ] Document results

---

## Phase 8: Documentation

- [ ] Write test README with instructions
- [ ] Document test scenarios
- [ ] Document expected results
- [ ] Add troubleshooting guide

---

## Phase 9: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] NAT testing with cloud Intermediate
- [ ] Automated CI integration
- [ ] Stress testing
- [ ] Chaos engineering tests
