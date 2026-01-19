# TODO: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Branch:** `feature/005-p2p-hole-punching`
**Depends On:** Tasks 002, 003, 004

---

## Prerequisites

- [ ] Task 002 (Intermediate Server) complete and merged
- [ ] Task 003 (App Connector) complete and merged
- [ ] Task 004 (E2E Relay Testing) complete and merged
- [ ] Create feature branch: `git checkout -b feature/005-p2p-hole-punching`

---

## Phase 1: Candidate Gathering

- [ ] Create `p2p/` module in packet_processor
- [ ] Implement `Candidate` struct
- [ ] Implement `CandidateType` enum
- [ ] Implement host candidate gathering (enumerate interfaces)
- [ ] Implement reflexive candidate from QAD response
- [ ] Implement relay candidate (Intermediate address)
- [ ] Implement priority calculation
- [ ] Unit tests for candidate gathering

---

## Phase 2: Candidate Exchange Protocol

- [ ] Define `SignalingMessage` enum
- [ ] Implement serialization (bincode or MessagePack)
- [ ] Agent: send candidates via QUIC stream
- [ ] Intermediate: relay candidates between peers
- [ ] Connector: receive and send candidates
- [ ] Integration test for candidate exchange

---

## Phase 3: Connectivity Checks

- [ ] Implement candidate pair formation
- [ ] Implement priority-based sorting
- [ ] Define `BindingRequest` struct
- [ ] Define `BindingResponse` struct
- [ ] Implement check sender with retransmit
- [ ] Implement check receiver and responder
- [ ] Track successful/failed pairs
- [ ] Unit tests for connectivity checks

---

## Phase 4: Hole Punching Coordination

- [ ] Implement timing coordination message
- [ ] Intermediate: broadcast "start punching" command
- [ ] Agent: send packets on schedule
- [ ] Connector: send packets on schedule
- [ ] Implement simultaneous open detection
- [ ] Implement NAT keepalive (15s interval)
- [ ] Implement path failure detection
- [ ] Integration test for hole punching

---

## Phase 5: Symmetric NAT Handling

- [ ] Detect symmetric NAT (port varies per destination)
- [ ] Implement port prediction (optional, P2)
- [ ] Graceful fallback to relay
- [ ] Log NAT type for debugging

---

## Phase 6: Connection Migration

- [ ] Implement `should_migrate()` decision logic
- [ ] Integrate QUIC connection migration
- [ ] Implement path validation
- [ ] Implement atomic routing update
- [ ] Ensure zero packet loss during migration
- [ ] Migration metrics (latency before/after)
- [ ] Integration test for migration

---

## Phase 7: Testing

### Same Network (LAN)
- [ ] Agent and Connector on same subnet
- [ ] Verify host candidates selected
- [ ] Measure latency (should be <1ms)

### Standard NAT
- [ ] Agent behind home router
- [ ] Connector on different network
- [ ] Verify hole punching succeeds
- [ ] Measure latency improvement vs relay

### Symmetric NAT
- [ ] Simulate carrier-grade NAT
- [ ] Verify relay fallback works
- [ ] Test port prediction (if implemented)

### Migration
- [ ] Start with relay-only
- [ ] Establish direct path
- [ ] Verify automatic migration
- [ ] No packet loss during switch

### Stress Test
- [ ] Multiple concurrent connections
- [ ] Rapid path changes
- [ ] Network interruption recovery

---

## Phase 8: Documentation

- [ ] Update architecture.md with P2P details
- [ ] Document NAT compatibility matrix
- [ ] Document troubleshooting guide
- [ ] Add P2P section to README

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

- [ ] ICE restart on path failure
- [ ] Multiple simultaneous paths (QUIC multipath)
- [ ] Mobile handoff (WiFi → Cellular)
- [ ] IPv6 support
- [ ] UPnP/NAT-PMP port mapping
