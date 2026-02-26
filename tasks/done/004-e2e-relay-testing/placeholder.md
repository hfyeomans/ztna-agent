# Placeholder Documentation: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing

---

## Purpose

Document intentional placeholder/scaffolding code that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

### Simplified Flow Mapping (Single Flow Only)

**File:** `app-connector/src/main.rs`
**Line:** ~270 (in `process_local_socket()` method)
**Added:** 2026-01-19
**Status:** In Progress (works for MVP, needs enhancement for production)

**Purpose:**
Map return traffic from local services back to the correct QUIC connection. In production, this requires proper 5-tuple matching to support multiple concurrent flows.

**Current Implementation:**
```rust
// Simplified lookup - gets first flow
let flow_key = self.flow_map.keys().next().cloned();
```
Takes the first (and only) entry in the flow map. Works correctly with a single active flow.

**Target Implementation:**
Proper 5-tuple matching based on source IP:port from the local socket response:
```rust
// Proper lookup by 5-tuple
let src_addr = recv_addr; // From local socket
let flow_key = self.flow_map.get(&(src_addr.ip(), src_addr.port()));
```

**Blocked By:**
- Multiple concurrent flows test scenario (Phase 4.3)
- Decision on flow tracking data structure (HashMap vs custom index)

---

## Template

When adding placeholder code, document it here:

```markdown
### [Short Description]

**File:** `path/to/file.sh`
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

### NAT Testing Infrastructure

**Purpose:** Test with Intermediate deployed to cloud.

**Likely placeholder:**
- Local-only testing initially
- Cloud deployment later

### Automated Performance Benchmarks

**Purpose:** CI-integrated performance regression testing.

**Likely placeholder:**
- Manual measurement initially
- Automated benchmarks later

### Chaos Testing

**Purpose:** Network failure injection for reliability testing.

**Likely placeholder:**
- Manual failure testing initially
- Automated chaos engineering later
