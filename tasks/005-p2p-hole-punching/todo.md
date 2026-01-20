# TODO: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Branch:** `feature/005-p2p-hole-punching`
**Depends On:** Tasks 002, 003, 004
**Last Updated:** 2026-01-20
**Oracle Review:** 2026-01-20

---

## Prerequisites

- [x] Task 002 (Intermediate Server) complete and merged
- [x] Task 003 (App Connector) complete and merged
- [x] Task 004 (E2E Relay Testing) complete and merged
- [x] Create feature branch: `git checkout -b feature/005-p2p-hole-punching`

---

## Local Testing Scope

> **Important:** This PoC/MVP runs entirely on localhost. Some features cannot be fully validated without cloud deployment (Task 006).

### Testable Locally
- [x] All unit tests pass without network
- [x] Candidate gathering (enumerate interfaces)
- [ ] Candidate exchange via Intermediate (localhost)
- [ ] Binding request/response protocol
- [ ] Direct QUIC connection Agent → Connector (localhost)
- [ ] Fallback to relay when direct "fails" (simulated)
- [ ] Keepalive mechanism

### Requires Cloud (Task 006)
- [ ] Real NAT hole punching
- [ ] Reflexive candidate accuracy
- [ ] NAT type detection
- [ ] Cross-network latency comparison

---

## Phase 0: Socket Architecture (Foundation)

> **Critical:** Must be completed first. Single socket reuse is required for hole punching.

- [x] Audit current socket usage in Agent (`core/packet_processor/`)
- [x] Audit current socket usage in Connector (`app-connector/`)
- [x] Design single-socket architecture for Agent
- [x] Design single-socket architecture for Connector
- [x] Implement QUIC server mode for Connector
  - [x] Generate self-signed TLS certificate for Connector P2P
  - [x] Add server listener on same socket as client
  - [x] Handle incoming QUIC connections
- [x] Unit test: Connector accepts incoming QUIC connection
- [x] Add multi-connection support to Agent
- [x] Document socket architecture in plan.md

---

## Phase 1: Candidate Gathering ✅ COMPLETE

- [x] Create `p2p/` module in `core/packet_processor/src/`
- [x] Implement `Candidate` struct with fields:
  - [x] `candidate_type: CandidateType`
  - [x] `address: SocketAddr`
  - [x] `priority: u32`
  - [x] `foundation: String`
  - [x] `related_address: Option<SocketAddr>`
- [x] Implement `CandidateType` enum (Host, ServerReflexive, PeerReflexive, Relay)
- [x] Implement `calculate_priority()` per RFC 8445
- [x] Implement `gather_host_candidates()`:
  - [x] Accept local addresses from caller
  - [x] Filter loopback addresses (configurable)
  - [x] Return list of host candidates
- [x] Implement `enumerate_local_addresses()` via libc getifaddrs
- [x] Implement `gather_reflexive_candidate()` from QAD response
- [x] Implement `gather_relay_candidate()` (Intermediate address)
- [x] Implement `sort_candidates_by_priority()`
- [x] Unit tests (11 tests):
  - [x] `test_candidate_type_preference()`
  - [x] `test_calculate_priority()`
  - [x] `test_host_candidate_creation()`
  - [x] `test_srflx_candidate_creation()`
  - [x] `test_relay_candidate_creation()`
  - [x] `test_gather_host_candidates()`
  - [x] `test_gather_reflexive_candidate()`
  - [x] `test_sort_candidates_by_priority()`
  - [x] `test_candidate_display()`
  - [x] `test_is_loopback()`
  - [x] `test_enumerate_local_addresses()`

---

## Phase 2: Signaling Infrastructure

### 2.1 Message Format
- [ ] Define `SignalingMessage` enum:
  - [ ] `CandidateOffer { session_id, candidates }`
  - [ ] `CandidateAnswer { session_id, candidates }`
  - [ ] `StartPunching { target_time_ms, peer_candidates }`
- [ ] Add bincode serialization (add dependency)
- [ ] Define message framing (4-byte length prefix)
- [ ] Unit tests for serialization/deserialization

### 2.2 Intermediate Server Changes
- [ ] Add signaling stream handler
- [ ] Implement candidate storage per session
- [ ] Implement candidate relay logic
- [ ] Add `StartPunching` broadcast command
- [ ] Integration test: Agent → Intermediate → Connector signaling

### 2.3 Agent/Connector Signaling Client
- [ ] Implement `send_candidates()` via QUIC stream
- [ ] Implement `receive_candidates()` via QUIC stream
- [ ] Handle `StartPunching` command
- [ ] Add timeout/retry logic (5 second timeout)

---

## Phase 3: Direct Path Establishment

### 3.1 Binding Protocol
- [ ] Define `BindingRequest` struct (transaction_id, priority)
- [ ] Define `BindingResponse` struct (transaction_id, success, mapped_address)
- [ ] Implement serialization for binding messages
- [ ] Unit tests for binding protocol

### 3.2 Candidate Pair Management
- [ ] Implement candidate pair formation (local × remote)
- [ ] Implement priority-based sorting
- [ ] Track pair states (Waiting, In-Progress, Succeeded, Failed)
- [ ] Unit tests for pair management

### 3.3 Connectivity Checks
- [ ] Implement check sender with retransmit
- [ ] Implement check receiver and responder
- [ ] Handle timing coordination from Intermediate
- [ ] Detect successful path (bidirectional)
- [ ] Integration test: localhost connectivity checks

---

## Phase 4: QUIC Connection and Path Selection

### 4.1 Direct QUIC Connection
- [ ] Agent: Establish QUIC connection to Connector's address
- [ ] Connector: Accept QUIC connection from Agent
- [ ] Verify data can flow on direct connection
- [ ] Integration test: Agent ↔ Connector direct QUIC

### 4.2 Path Selection
- [ ] Implement `should_use_direct()` decision logic
- [ ] Measure RTT on both paths (relay vs direct)
- [ ] Implement path switching logic
- [ ] Atomic routing update (no packet loss)
- [ ] Integration test: path selection works

### 4.3 quiche API Integration
- [ ] Validate quiche `probe_path()` API
- [ ] Validate quiche `migrate()` API
- [ ] Handle `PathEvent` variants
- [ ] Integration test: quiche path operations

---

## Phase 5: Resilience

### 5.1 Keepalive
- [ ] Implement keepalive sender (15s interval)
- [ ] Implement keepalive receiver
- [ ] Track missed keepalives

### 5.2 Failure Detection
- [ ] Define "hole punch failed" criteria:
  - [ ] All candidate pairs exhausted
  - [ ] Total timeout (5s) exceeded
- [ ] Implement failure detection state machine
- [ ] Trigger fallback to relay

### 5.3 Fallback
- [ ] Implement graceful fallback transition
- [ ] Maintain session state during fallback
- [ ] No packet loss during switch
- [ ] Integration test: fallback to relay

### 5.4 NAT Type Detection (Deferred for Cloud)
- [ ] Design detection algorithm (QAD to multiple servers)
- [ ] Document symmetric NAT handling (use relay)
- [ ] Skip reflexive candidates if symmetric NAT detected

---

## Phase 6: Testing

### 6.1 Unit Tests
- [ ] All candidate module tests pass
- [ ] All signaling module tests pass
- [ ] All connectivity module tests pass
- [ ] All path selection tests pass

### 6.2 Integration Tests (Localhost)
- [ ] Candidate exchange via Intermediate
- [ ] Direct QUIC connection establishment
- [ ] Path selection prefers direct
- [ ] Fallback to relay on failure
- [ ] Keepalive maintains connection

### 6.3 Simulated Multi-Host Test
- [ ] Agent on 127.0.0.2
- [ ] Connector on 127.0.0.3
- [ ] Intermediate on 127.0.0.1
- [ ] Verify direct path established

### 6.4 E2E Test Script
- [ ] Create `tests/e2e/scenarios/p2p-hole-punching.sh`
- [ ] Test direct connection success
- [ ] Test fallback on failure
- [ ] Add to test suite

---

## Phase 7: Documentation

- [ ] Update `docs/architecture.md` with P2P details
- [ ] Document local testing limitations
- [ ] Document NAT compatibility (what works, what doesn't)
- [ ] Add troubleshooting guide for P2P
- [ ] Update `tasks/_context/components.md` status
- [ ] Prepare test plan for Task 006 (Cloud testing)

---

## Phase 8: PR & Merge

- [ ] Update state.md with completion status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## MVP Deliverables Checklist

> Minimum viable for Phase 1 completion (local PoC)

- [ ] Agent gathers host candidates (local IPs)
- [ ] Candidate exchange via Intermediate works
- [ ] Both sides attempt direct connection to each other's host candidates
- [ ] If ANY direct path works, use it
- [ ] If all direct paths fail within 5 seconds, use relay
- [ ] Basic connectivity maintained via keepalives
- [ ] All localhost tests pass

---

## Deferred (Post-MVP / Task 006)

> These require cloud deployment or real NAT scenarios

- [ ] Reflexive candidate validation (requires real NAT)
- [ ] Port prediction for symmetric NAT
- [ ] Multiple simultaneous paths (QUIC multipath)
- [ ] IPv6 support
- [ ] UPnP/NAT-PMP port mapping
- [ ] Mobile handoff (WiFi → Cellular)
- [ ] ICE restart on path failure

---

## Risks Tracked

| Risk | Status | Mitigation |
|------|--------|------------|
| Connector as QUIC server | ✅ Closed | Implemented dual-mode QUIC (client+server) |
| Single socket constraint | ✅ Closed | Architecture designed, Connector uses single socket |
| quiche API correctness | 🔲 Open | Validate during Phase 4 |
| Symmetric NAT | 🔲 Open | Use relay fallback |
