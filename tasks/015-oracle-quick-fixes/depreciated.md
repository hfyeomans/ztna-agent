# Task 015: Oracle Quick Fixes — Deprecated/Legacy Code

**Description:** Code removed or replaced during this task.

**Purpose:** Record what was removed and why, per project convention.

---

## Removed: `process_packet()` FFI Function (Finding 11)

**Status:** Removed in Task 015 implementation

**What:** Legacy C FFI function `process_packet(data: *const u8, len: size_t) -> PacketAction` and `PacketAction` enum

**Removed from:**
- `core/packet_processor/src/lib.rs` — function definition, `PacketAction` enum, and `test_process_packet` test
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` — C declaration and `PacketAction` typedef
- `docs/architecture_design.md:73` — updated to reference QUIC agent FFI
- `docs/walkthrough.md:26` — updated Console.app filter text

**Why removed:**
- Always returned `PacketAction::Forward` regardless of input — performed no filtering
- Superseded by `agent_create()` and the full Agent struct API (Task 001)
- No Swift callers existed (verified via grep of `ios-macos/` directory)
- Flagged by Oracle Review 01 (Finding 11, Medium severity) as dead code

**Replacement:** Use `agent_create()` + `agent_send_datagram()` / `agent_recv_datagram()` for packet processing.
The `PacketAction` enum was also removed — it had no remaining references outside `process_packet`.

---

## Replaced: Time+PID Random Generation (Finding 8)

**Status:** Replaced in Task 015 implementation

**What:** `generate_session_id()` and `generate_transaction_id()` using `SystemTime + PID`

**Replaced in:**
- `core/packet_processor/src/p2p/signaling.rs` — `generate_session_id()` now uses `ring::rand::SystemRandom`
- `core/packet_processor/src/p2p/connectivity.rs` — `generate_transaction_id()` now uses `ring::rand::SystemRandom`

**Why replaced:**
- Predictable on same machine (time is observable, PID is enumerable)
- Oracle Review 01 (Finding 8, Medium severity) flagged spoofing/replay risk
- Replaced with `ring::rand::SystemRandom` (cryptographically secure PRNG)
