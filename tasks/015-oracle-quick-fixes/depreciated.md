# Task 015: Oracle Quick Fixes — Deprecated/Legacy Code

**Description:** Code removed or replaced during this task.

**Purpose:** Record what was removed and why, per project convention.

---

## To Be Removed: `process_packet()` FFI Function (Finding 11)

**What:** Legacy C FFI function `process_packet(data: *const u8, len: size_t) -> PacketAction`

**Where:**
- `core/packet_processor/src/lib.rs:2118-2132` — function definition
- `core/packet_processor/src/lib.rs:2143-2146` — test `test_process_packet`
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h:19` — C declaration

**Why removed:**
- Always returns `PacketAction::Forward` regardless of input — performs no filtering
- Superseded by `agent_create()` + `agent_process_packet()` and the full Agent struct API (Task 001)
- No Swift callers exist (verified via grep of `ios-macos/` directory)
- Flagged by Oracle Review 01 (Finding 11, Medium severity) as dead code
- Flagged independently by Rust code review (`tasks/rust-code-review/review.md`)

**Replacement:** Use `agent_create()` + `agent_process_packet()` for packet processing.
The `PacketAction` enum itself is retained (used by `agent_process_packet`).

---

## To Be Replaced: Time+PID Random Generation (Finding 8)

**What:** `generate_session_id()` and `generate_transaction_id()` using `SystemTime + PID`

**Where:**
- `core/packet_processor/src/p2p/signaling.rs:311-323`
- `core/packet_processor/src/p2p/connectivity.rs:132-143`

**Why replaced:**
- Predictable on same machine (time is observable, PID is enumerable)
- Oracle Review 01 (Finding 8, Medium severity) flagged spoofing/replay risk
- Will be replaced with `ring::rand::SystemRandom` (cryptographically secure)
