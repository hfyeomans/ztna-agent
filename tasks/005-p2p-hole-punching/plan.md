# Implementation Plan: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching
**Branch:** `feature/005-p2p-hole-punching`
**Depends On:** 002, 003, 004

---

## Goal

Implement direct peer-to-peer connectivity via NAT hole punching. This is the **primary architectural goal** - relay is only used when direct connection fails.

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/005-p2p-hole-punching

# While working:
git add . && git commit -m "005: descriptive message"

# When complete:
git push -u origin feature/005-p2p-hole-punching
# Create PR → Review → Merge to master
```

---

## Phase 1: Candidate Gathering

### 1.1 Candidate Types
- [ ] Local candidates (host IPs, all interfaces)
- [ ] Reflexive candidates (from QAD response)
- [ ] Relay candidates (Intermediate server as TURN-like relay)

### 1.2 Candidate Format
```rust
struct Candidate {
    candidate_type: CandidateType,  // Host, ServerReflexive, Relay
    address: SocketAddr,
    priority: u32,
    foundation: String,
}

enum CandidateType {
    Host,           // Local IP
    ServerReflexive, // Public IP from QAD
    Relay,          // Via Intermediate
}
```

### 1.3 Priority Calculation
- Host candidates: highest priority (local network = lowest latency)
- Reflexive candidates: medium priority (direct through NAT)
- Relay candidates: lowest priority (relay overhead)

---

## Phase 2: Candidate Exchange Protocol

### 2.1 Signaling via Intermediate
- [ ] Agent sends candidates to Intermediate
- [ ] Intermediate relays to Connector
- [ ] Connector sends its candidates back
- [ ] Both sides receive full candidate lists

### 2.2 Message Format
```rust
enum SignalingMessage {
    CandidateOffer {
        session_id: u64,
        candidates: Vec<Candidate>,
    },
    CandidateAnswer {
        session_id: u64,
        candidates: Vec<Candidate>,
    },
}
```

### 2.3 Serialization
- Use bincode or MessagePack for compact encoding
- Send via QUIC stream (not DATAGRAM for reliability)

---

## Phase 3: Connectivity Checks

### 3.1 Check Algorithm
- [ ] Form candidate pairs (local × remote)
- [ ] Sort by priority
- [ ] Send binding requests to each pair
- [ ] Track successful paths

### 3.2 Binding Request/Response
```rust
struct BindingRequest {
    transaction_id: [u8; 12],
    priority: u32,
}

struct BindingResponse {
    transaction_id: [u8; 12],
    success: bool,
    mapped_address: Option<SocketAddr>,
}
```

### 3.3 Timing
- Initial check interval: 20ms (aggressive)
- Retransmit timeout: 100ms
- Max retransmits: 5

---

## Phase 4: Hole Punching Coordination

### 4.1 Simultaneous Open
- [ ] Both sides must send packets ~simultaneously
- [ ] Intermediate coordinates timing
- [ ] "Start punching at T+100ms" message

### 4.2 NAT Keepalive
- [ ] Send keepalive every 15 seconds
- [ ] Detect path failure (3 missed responses)
- [ ] Fall back to relay on failure

### 4.3 Symmetric NAT Handling
- [ ] Detect symmetric NAT (different port per destination)
- [ ] Port prediction (increment pattern)
- [ ] Fall back to relay if prediction fails

---

## Phase 5: Connection Migration

### 5.1 QUIC Connection Migration
- [ ] Use QUIC's built-in path migration
- [ ] Migrate from relay path to direct path
- [ ] Validate new path before abandoning old

### 5.2 Migration Trigger
```rust
fn should_migrate(direct_rtt: Duration, relay_rtt: Duration) -> bool {
    // Migrate if direct path is significantly better
    direct_rtt < relay_rtt * 0.7
}
```

### 5.3 Seamless Transition
- [ ] No packet loss during migration
- [ ] Maintain session state
- [ ] Update routing atomically

---

## Phase 6: Testing

### 6.1 Same Network Test
- [ ] Agent and Connector on same LAN
- [ ] Should use host candidates
- [ ] Verify lowest latency

### 6.2 NAT Test (Home Network)
- [ ] Agent behind home NAT
- [ ] Connector on public IP (or different NAT)
- [ ] Verify hole punching works

### 6.3 Symmetric NAT Test
- [ ] Use carrier-grade NAT simulation
- [ ] Verify relay fallback works
- [ ] Test port prediction (if implemented)

### 6.4 Migration Test
- [ ] Start with relay connection
- [ ] Verify migration to direct
- [ ] Measure latency improvement

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Candidate Gathering                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │   Agent     │    │Intermediate │    │  Connector  │         │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘         │
│         │                   │                   │                │
│         │ 1. Get local IPs  │                   │                │
│         │◄──────────────────│                   │                │
│         │                   │                   │                │
│         │ 2. QAD Request ───►                   │                │
│         │◄─── Reflexive IP ─┤                   │                │
│         │                   │                   │                │
│         │ 3. Candidates ────►                   │                │
│         │                   │────► Relay ───────►                │
│         │                   │◄──── Candidates ──┤                │
│         │◄── Relay ─────────┤                   │                │
│         │                   │                   │                │
└─────────┴───────────────────┴───────────────────┴────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                      Hole Punching                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Agent                    NAT A        NAT B        Connector   │
│    │                        │            │              │        │
│    │─── UDP to Connector ──►│            │              │        │
│    │    (creates mapping)   │──── X ────►│              │        │
│    │                        │            │              │        │
│    │                        │            │◄── UDP ──────┤        │
│    │                        │◄───────────│  (creates    │        │
│    │                        │            │   mapping)   │        │
│    │                        │            │              │        │
│    │─── UDP (retransmit) ──►│────────────►──────────────►        │
│    │◄── UDP response ───────│◄───────────│◄─────────────┤        │
│    │                        │            │              │        │
│    │═══ Direct P2P Path Established ════════════════════│        │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

1. [ ] Host candidates work (same LAN)
2. [ ] Reflexive candidates work (standard NAT)
3. [ ] Connection migration from relay to direct
4. [ ] Graceful fallback when hole punching fails
5. [ ] No data loss during migration
6. [ ] Latency improvement measurable

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Symmetric NAT | Relay fallback always works |
| Timing sensitivity | Intermediate coordinates start time |
| Port prediction | Only attempt for known patterns |
| Mobile networks | More aggressive keepalives |

---

## File Structure

```
core/packet_processor/src/
├── p2p/
│   ├── mod.rs           # P2P module
│   ├── candidate.rs     # Candidate types and gathering
│   ├── signaling.rs     # Candidate exchange protocol
│   ├── connectivity.rs  # Connectivity checks
│   ├── hole_punch.rs    # Hole punching coordination
│   └── migration.rs     # Connection migration
```

---

## References

- [RFC 8445 - ICE](https://tools.ietf.org/html/rfc8445) - Interactive Connectivity Establishment
- [RFC 5389 - STUN](https://tools.ietf.org/html/rfc5389) - Session Traversal Utilities for NAT
- [QUIC Connection Migration](https://www.rfc-editor.org/rfc/rfc9000#section-9)
- [NAT Behavior Discovery](https://tools.ietf.org/html/rfc5780)
