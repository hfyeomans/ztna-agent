# Placeholder Documentation: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching

---

## Purpose

Document intentional placeholder/scaffolding code that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

*None yet - task not started*

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
