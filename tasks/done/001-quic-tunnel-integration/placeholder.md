# Placeholder Documentation: QUIC Tunnel Integration

**Task ID:** 001-quic-tunnel-integration
**Status:** ✅ COMPLETE

---

## Purpose

Document intentional placeholder/scaffolding code that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

*None - task complete*

---

## Resolved Placeholders

### Connection State Logging

**File:** `core/packet_processor/src/lib.rs`
**Resolved:** 2026-01-18

**Was:**
Initial implementation used println! for debugging.

**Resolution:**
Removed all println! calls. Logging now handled by Swift via `Logger` (os framework). Extension sandboxing doesn't support stdout.

---

### Thread Safety for isRunning

**File:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`
**Resolved:** 2026-01-18 (Phase 1.5)

**Was:**
`isRunning` was a plain Bool, causing potential data race.

**Resolution:**
Changed to `OSAllocatedUnfairLock<Bool>` for thread-safe access from multiple async contexts.

---

### Connection ID Generation

**File:** `core/packet_processor/src/lib.rs`
**Resolved:** 2026-01-18 (Phase 1.5)

**Was:**
Connection IDs were generated predictably (static or sequential).

**Resolution:**
Changed to `ring::rand::SystemRandom` for cryptographically secure random connection IDs.
