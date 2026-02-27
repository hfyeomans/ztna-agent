# Task 015: Oracle Quick Fixes — Placeholders

**Description:** Track any stubs or placeholder code introduced during this task.

**Purpose:** Ensure no `// TODO` comments enter production; document any interim solutions.

---

## Placeholders

None yet — task has not entered implementation phase.

## Known Interim Solutions

### IPv6 QAD (Finding 6)

The fix for this task is itself an interim solution: IPv6 connections will have QAD **skipped** (returns `None`) rather than being fully supported. Full IPv6 QAD support is tracked in Task 011 (Protocol Improvements). No placeholder code or TODO comments will be left in the source — the `None` return is a deliberate graceful degradation, not a stub.

### Findings 8, 11, 15 — No Placeholders

These fixes are complete replacements/removals, not interim solutions:
- **Finding 8** (predictable IDs): Replaces time+PID with `ring::rand::SystemRandom` — full fix, no placeholder.
- **Finding 11** (legacy FFI): Removes dead code entirely — no replacement needed.
- **Finding 15** (UDP length): Adds early return for malformed packets — full fix, no placeholder.
