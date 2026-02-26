# Placeholder Documentation: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching

---

## Purpose

Document intentional placeholder/scaffolding code that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

### P2P Hole Punching E2E Test Script (Stub)

**File:** `tests/e2e/scenarios/p2p-hole-punching.sh`
**Line:** Entire file
**Added:** 2026-01-20
**Status:** Stub - Pending Phase 4 Integration

**Purpose:**
E2E test script for P2P hole punching functionality including candidate exchange, direct QUIC connections, path selection, and fallback to relay.

**Current Implementation (Stub):**
- Script structure and test framework in place
- 6 test functions defined but log warnings and return success
- Prerequisites check for P2P certificates
- Connector P2P mode startup function (uses `--p2p-cert` and `--p2p-key` flags)

**Stub Tests:**
| Test | Function | Status |
|------|----------|--------|
| 7.1 Candidate exchange via Intermediate | `test_candidate_exchange()` | Stub |
| 7.2 Direct QUIC connection (localhost) | `test_direct_connection()` | Stub |
| 7.3 Path selection prefers direct | `test_path_selection_direct()` | Stub |
| 7.4 Fallback to relay on failure | `test_fallback_to_relay()` | Stub |
| 7.5 Multi-host simulation | `test_multihost_simulation()` | Stub |
| 7.6 Keepalive maintains connection | `test_keepalive()` | Stub |

**To Complete:**

1. **QUIC Test Client P2P Support** (Blocking)
   - Add `--enable-p2p` flag to `quic-test-client`
   - Add `--verify-direct` flag to verify direct path used
   - Add candidate gathering and signaling support

2. **App Connector P2P Integration** (Blocking)
   - Wire `HolePunchCoordinator` into main loop
   - Parse `--p2p-cert` and `--p2p-key` CLI flags
   - Enable signaling message handling

3. **Test Implementations:**
   - `test_candidate_exchange()`: Send CandidateOffer, verify CandidateAnswer received, check logs for signaling messages
   - `test_direct_connection()`: Verify connection established without Intermediate relay, check source/dest addresses
   - `test_path_selection_direct()`: Measure RTT on both paths, verify direct path selected when faster
   - `test_fallback_to_relay()`: Block direct path (invalid address), verify relay used
   - `test_multihost_simulation()`: Setup loopback aliases (127.0.0.2/3), verify cross-"host" connection
   - `test_keepalive()`: Wait 15s, verify keepalive sent, verify connection alive

4. **Multi-Host Setup Script:**
   - Create `setup-multihost.sh` to add loopback aliases
   - `sudo ifconfig lo0 alias 127.0.0.2`
   - `sudo ifconfig lo0 alias 127.0.0.3`

**Blocked By:**
- Phase 4 Integration: Wire HolePunchCoordinator into Agent/Connector
- Phase 5: Keepalive implementation
- QUIC test client P2P support

---

---

## Template

When adding placeholder code, document it here:

```markdown
### [Short Description]

**File:** `path/to/file.rs`
**Line:** 123
**Added:** YYYY-MM-DD
**Status:** Pending / In Progress / Resolved

**Purpose:**
Why this placeholder exists and what it should eventually do.

**Current Implementation:**
What the code does now (stub, hardcoded value, etc.)

**Target Implementation:**
What it should do when complete.

**Blocked By:**
Any dependencies or decisions needed.
```

---

## Anticipated Placeholders

### Port Prediction for Symmetric NAT

**Purpose:** Predict next port allocation for symmetric NAT.

**Likely placeholder:**
- Return "no prediction" initially
- Implement pattern detection later

### IPv6 Candidate Gathering

**Purpose:** Support IPv6 addresses alongside IPv4.

**Likely placeholder:**
- IPv4 only initially
- Add IPv6 when testing infrastructure ready

### Multiple Path Support

**Purpose:** QUIC multipath for redundant connectivity.

**Likely placeholder:**
- Single path initially
- Multipath when quiche supports it

### UPnP/NAT-PMP Port Mapping

**Purpose:** Automatic port forwarding on supported routers.

**Likely placeholder:**
- No port mapping initially
- Add UPnP library integration later

### Mobile Handoff

**Purpose:** Seamless WiFi to Cellular transition.

**Likely placeholder:**
- Reconnect on network change initially
- Seamless handoff later
