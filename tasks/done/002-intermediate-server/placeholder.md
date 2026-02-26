# Placeholder Documentation: Intermediate Server

**Task ID:** 002-intermediate-server

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

## Example Entry

### Authentication Token Validation

**File:** `src/server.rs`
**Line:** TBD
**Added:** 2026-01-XX
**Status:** Pending

**Purpose:**
Validate client authentication tokens during connection.

**Current Implementation:**
```rust
// Placeholder: accept all connections for MVP
fn validate_token(_token: &[u8]) -> bool {
    true
}
```

**Target Implementation:**
- Parse JWT or custom token format
- Verify signature with shared secret or public key
- Check expiration and claims
- Return authorized destinations

**Blocked By:**
- Authentication system design
- Token format decision
