# Task 015: Oracle Quick Fixes — State

**Description:** Current task state and progress tracking.

**Purpose:** Enable session resumption — read this file to understand where work left off.

---

## Status: Complete — Awaiting PR Review

## Branch: `fix/015-oracle-quick-fixes`

## Current Phase: Complete

### Completed (Triage Phase)
- [x] Read oracle-review-01.md (15 findings)
- [x] Triage against current codebase (post-Task 007)
- [x] Oracle verification of triage (gpt-5.3-codex, xhigh)
- [x] Oracle corrections applied (findings 2, 3, 9, 10, 15 reclassified)
- [x] Quick fixes identified: findings 6, 8, 11, 15
- [x] Deferred items mapped to target tasks: 008, 009, 011
- [x] Task folder created with 6 required files

### Completed (Planning Phase)
- [x] Deferred findings incorporated into tasks 008, 009, 011 (all 6 files per task)
- [x] _context/README.md updated with corrected triage and Task 015 entry
- [x] Oracle review of triage completeness — corrections applied
- [x] Oracle feedback: Finding 3 needs stronger connector binding in Task 009
- [x] Oracle feedback: Finding 5 needs per-flow socket strategy in Task 009
- [x] Oracle feedback: _context/README.md premature completion marks fixed

### Completed (Implementation Phase)
- [x] Phase 1: IPv6 QAD panic fix (finding 6) — `build_observed_address()` returns `Option<Vec<u8>>`, panic replaced with `log::warn` + None
- [x] Phase 2: Predictable P2P identifiers fix (finding 8) — `ring::rand::SystemRandom` CSPRNG replaces time+PID
- [x] Phase 3: Legacy FFI dead code removal (finding 11) — `process_packet()`, `PacketAction`, bridging header, docs all removed
- [x] Phase 4: UDP length sanity fix (finding 15) — `udp_len < 8` guard drops malformed packets
- [x] Build & test verification — 143 tests pass, 0 clippy warnings across all 3 crates
- [x] Oracle post-implementation review (gpt-5.3-codex) — no defects found

## Key Decisions
- Finding 10 (endian bug): Oracle says `to_ne_bytes()` may be correct — deferred to Task 011 for investigation rather than blind fix
- Finding 3 (signaling hijack): Oracle confirmed NOT fixed by Task 007 — deferred to Task 009
- Finding 2 (registration auth): Conditionally fixed but needs policy decision on SAN-less certs — deferred to Task 009
