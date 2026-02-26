# Placeholder Documentation: App Connector

**Task ID:** 003-app-connector

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

### TCP Connection State Management

**Purpose:** Properly handle TCP connection state for proxying.

**Likely placeholder:**
- Simple timeout-based cleanup initially
- Full state machine later

### Multi-Protocol Support

**Purpose:** Support protocols beyond TCP/UDP.

**Likely placeholder:**
- Log and drop non-TCP/UDP packets initially
- Add ICMP support later

### Response Timeout Handling

**Purpose:** Handle cases where local service doesn't respond.

**Likely placeholder:**
- Simple fixed timeout initially
- Configurable timeouts later
