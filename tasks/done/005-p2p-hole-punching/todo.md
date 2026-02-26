# TODO: P2P Hole Punching ✅ COMPLETE

**Task ID:** 005-p2p-hole-punching
**Status:** ✅ COMPLETE - Merged to Master
**Branch:** `master` (merged from `feature/005-p2p-hole-punching`)
**PR:** https://github.com/hfyeomans/ztna-agent/pull/5
**Depends On:** Tasks 002, 003, 004
**Last Updated:** 2026-01-23
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
- [x] All unit tests pass without network (79 tests)
- [x] Candidate gathering (enumerate interfaces)
- [x] Signaling message encode/decode (13 tests)
- [x] Binding request/response protocol (17 tests)
- [x] Keepalive mechanism (12 tests)
- [x] Candidate exchange via Intermediate (verified via unit tests)
- [x] Direct QUIC connection Agent → Connector (verified via unit tests)
- [x] Fallback to relay when direct "fails" (verified via unit tests)

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

## Phase 2: Signaling Infrastructure ✅ COMPLETE

### 2.1 Message Format
- [x] Define `SignalingMessage` enum:
  - [x] `CandidateOffer { session_id, service_id, candidates }`
  - [x] `CandidateAnswer { session_id, candidates }`
  - [x] `StartPunching { session_id, start_delay_ms, peer_candidates }`
  - [x] `PunchingResult { session_id, success, working_address }`
  - [x] `Error { session_id, code, message }`
- [x] Add bincode/serde dependencies to packet_processor and intermediate-server
- [x] Define message framing (4-byte BE length prefix + bincode payload)
- [x] `encode_message()` / `decode_message()` / `decode_messages()`
- [x] Unit tests: 13 tests in packet_processor

### 2.2 Intermediate Server Changes
- [x] Create `signaling.rs` module with same message types
- [x] Implement `SignalingSession` for session state tracking
- [x] Implement `SessionManager` for managing active sessions
- [x] Unit tests: 6 tests in intermediate-server
- [x] Integration with main event loop (completed in Phase 4)

### 2.3 Agent/Connector Signaling Client
- [x] Stream-based I/O helpers (`write_message()` / `read_message()`)
- [x] `generate_session_id()` for unique session IDs
- [x] Integration with Agent/Connector (completed in Phase 4)

---

## Phase 3: Direct Path Establishment ✅ COMPLETE

### 3.1 Binding Protocol
- [x] Define `BindingRequest` struct (transaction_id, priority, use_candidate)
- [x] Define `BindingResponse` struct (transaction_id, success, mapped_address)
- [x] Define `BindingMessage` enum (Request | Response)
- [x] Implement bincode serialization (`encode_binding()` / `decode_binding()`)
- [x] Unit tests: 5 binding protocol tests

### 3.2 Candidate Pair Management
- [x] Implement `CandidatePair` with local/remote candidates
- [x] Implement `calculate_pair_priority()` per RFC 8445 §6.1.2.3
- [x] Track pair states via `CheckState` enum (Frozen, Waiting, InProgress, Succeeded, Failed)
- [x] IPv4/IPv6 family matching (only pair same family)
- [x] Unit tests: 5 pair management tests

### 3.3 Connectivity Checks
- [x] Implement `CheckList` for managing all candidate pairs
- [x] Priority-based pair sorting (highest first)
- [x] Foundation-based unfreezing logic
- [x] Pacing interval (20ms between checks)
- [x] Exponential backoff for retransmissions (100ms → 1600ms max)
- [x] `next_request()` returns next binding request to send
- [x] `handle_response()` processes responses and unfreezes pairs
- [x] `nominate()` marks successful pair as nominated
- [x] Unit tests: 7 check list tests
- [x] Integration with Agent/Connector (completed in Phase 4)

---

## Phase 4: Hole Punch Coordination ✅ COMPLETE

### 4.1 HolePunchCoordinator Module ✅ COMPLETE
- [x] Create `p2p/hole_punch.rs` module
- [x] Implement `HolePunchCoordinator` struct with state machine
- [x] Implement `HolePunchState` enum (Idle → Gathering → Signaling → Checking → Connected/Failed)
- [x] Implement `HolePunchResult` enum (DirectPath or UseRelay)
- [x] Implement candidate gathering (host, reflexive, relay)
- [x] Implement signaling message handling
- [x] Implement binding request/response orchestration
- [x] Implement timeout handling
- [x] 17 unit tests for hole punch module

### 4.2 Path Selection ✅ COMPLETE
- [x] Implement `select_path()` decision logic (RTT + reliability)
- [x] Implement `should_switch_to_direct()` threshold (50% faster)
- [x] Implement `should_switch_to_relay()` (failure-based)
- [x] Unit tests for path selection

### 4.3 Integration ✅ COMPLETE
- [x] Integration test: Agent ↔ Connector direct QUIC (localhost)
  - `test_hole_punch_coordinator_integration` added to packet_processor
- [x] Wire HolePunchCoordinator into Agent main loop
  - Added hole punching methods: `start_hole_punching`, `process_signaling_streams`, etc.
  - Added FFI functions: `agent_start_hole_punch`, `agent_poll_hole_punch`, etc.
- [x] Wire HolePunchCoordinator into Connector main loop
  - Created `signaling.rs` with full signaling types
  - Added signaling stream processing methods
- [x] Wire HolePunchCoordinator into Intermediate Server
  - Added SessionManager for P2P signaling sessions
  - Added `process_streams()` and message handlers

---

## Phase 5: Resilience ✅ COMPLETE

### 5.1 Keepalive
- [x] Implement keepalive sender (15s interval)
- [x] Implement keepalive receiver
- [x] Track missed keepalives

### 5.2 Failure Detection
- [x] Define "hole punch failed" criteria:
  - [x] All candidate pairs exhausted
  - [x] Total timeout (5s) exceeded
- [x] Implement failure detection state machine
- [x] Trigger fallback to relay

### 5.3 Fallback
- [x] Implement graceful fallback transition
- [x] Maintain session state during fallback
- [x] No packet loss during switch (PathManager handles)
- [x] Integration test: fallback to relay (test_path_manager_integration)

### 5.4 NAT Type Detection (Deferred for Cloud)
- [ ] Design detection algorithm (QAD to multiple servers)
- [ ] Document symmetric NAT handling (use relay)
- [ ] Skip reflexive candidates if symmetric NAT detected

---

## Phase 6: Testing

### 6.1 Unit Tests
- [x] All candidate module tests pass (11 tests)
- [x] All signaling module tests pass (13 tests)
- [x] All connectivity module tests pass (17 tests)
- [x] All hole punch module tests pass (17 tests)
- [x] All resilience module tests pass (12 tests)
- [x] All agent integration tests pass (9 tests)
- **Total: 79 tests passing**

### 6.2 Integration Tests (Localhost)
> **Note:** Full integration requires iOS/macOS Agent (Task 006). Current tests verify via unit tests.
- [x] Candidate exchange via Intermediate (verified via signaling unit tests)
- [x] Direct QUIC connection establishment (verified via connectivity unit tests)
- [x] Path selection prefers direct (verified via hole_punch unit tests)
- [x] Fallback to relay on failure (verified via resilience unit tests)
- [x] Keepalive maintains connection (verified via resilience unit tests)

### 6.3 Simulated Multi-Host Test
> **Note:** Full multi-host test requires iOS/macOS Agent binding to specific addresses.
- [x] Address enumeration verified (candidate unit tests)
- [x] Architecture supports multi-host (verified in test script)
- [ ] Agent on 127.0.0.2 (requires Task 006)
- [ ] Connector on 127.0.0.3 (requires Task 006)
- [ ] Verify direct path established (requires Task 006)

### 6.4 E2E Test Script ✅ COMPLETE
- [x] Create `tests/e2e/scenarios/p2p-hole-punching.sh`
- [x] Verify module implementation via unit tests
- [x] Verify Connector P2P mode startup
- [x] Verify protocol constants and message format
- [x] All 6 tests passing

---

## Phase 7: Documentation ✅ COMPLETE

- [x] Update `docs/architecture.md` with P2P details
- [x] Document local testing limitations
- [x] Document NAT compatibility (what works, what doesn't)
- [x] Add troubleshooting guide for P2P
- [x] Update `tasks/_context/components.md` status
- [x] Prepare test plan for Task 006 (Cloud testing)

---

## Phase 8: PR & Merge ✅ COMPLETE

- [x] Update state.md with completion status
- [x] Push branch to origin
- [x] Create PR for review: https://github.com/hfyeomans/ztna-agent/pull/5
- [x] Address review feedback
- [x] Merge to master (2026-01-20)

---

## MVP Deliverables Checklist ✅ IMPLEMENTED

> Implementation complete. Real E2E testing requires Task 005a (Swift Agent Integration).

- [x] Agent gathers host candidates (local IPs) - `enumerate_local_addresses()`
- [x] Candidate exchange via Intermediate works - signaling module
- [x] Both sides attempt direct connection to each other's host candidates - CheckList
- [x] If ANY direct path works, use it - HolePunchCoordinator
- [x] If all direct paths fail within 5 seconds, use relay - PathManager fallback
- [x] Basic connectivity maintained via keepalives - resilience module
- [x] All localhost tests pass - 79 unit tests + 6 E2E tests

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
| quiche API correctness | ✅ Closed | Validated in Phase 4, 79 tests passing |
| Symmetric NAT | ✅ Closed | Relay fallback implemented in PathManager |
