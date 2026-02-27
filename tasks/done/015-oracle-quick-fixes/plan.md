# Task 015: Oracle Quick Fixes — Plan

**Description:** Implementation plan for 4 quick-fix findings from Oracle Review 01 that can be resolved without architectural changes.

**Purpose:** Provide step-by-step implementation guidance for each fix, including exact file locations, proposed code changes, and test requirements.

---

## Scope

4 findings from `oracle-review-01.md`, verified by Oracle (gpt-5.3-codex, xhigh) as quick fixes:

| # | Severity | Finding | Component |
|---|----------|---------|-----------|
| 6 | High | IPv6 QAD panic (remote DoS) | intermediate-server |
| 8 | Medium | Predictable P2P identifiers | packet_processor |
| 11 | Medium | Legacy FFI dead code | packet_processor + bridging header |
| 15 | Low | UDP length sanity | app-connector |

---

## Phase 1: IPv6 QAD Panic (Finding 6)

**Goal:** Prevent server crash when an IPv6 client connects.

### Changes

**File: `intermediate-server/src/qad.rs`**
- Change `build_observed_address()` signature from `fn build_observed_address(addr: SocketAddr) -> Vec<u8>` to `fn build_observed_address(addr: SocketAddr) -> Option<Vec<u8>>`
- Replace `panic!("IPv6 QAD not yet implemented")` with `log::warn!("IPv6 QAD not yet supported, skipping"); return None;`

**File: `intermediate-server/src/main.rs`** (call-site ~line 871)
- Update `send_qad()` to handle `Option` return: if `None`, log and skip QAD send
- Connection continues without QAD (graceful degradation)

**Tests:**
- Update existing `test_build_observed_address` in `qad.rs` to verify IPv6 returns `None`
- Add test: IPv6 `SocketAddr` returns `None` without panicking

---

## Phase 2: Predictable P2P Identifiers (Finding 8)

**Goal:** Replace time+PID-based random generation with cryptographic randomness.

### Changes

**File: `core/packet_processor/src/p2p/signaling.rs`** (lines 311-323)
- Replace `generate_session_id()` body with `ring::rand::SystemRandom` to fill 8 bytes, convert to `u64`
- Remove `use std::time::{SystemTime, UNIX_EPOCH}` and PID logic

**File: `core/packet_processor/src/p2p/connectivity.rs`** (lines 132-143)
- Replace `generate_transaction_id()` body with `ring::rand::SystemRandom` to fill `TRANSACTION_ID_LEN` bytes
- Remove timestamp-based generation

**Dependency:** `ring = "0.17"` already in `core/packet_processor/Cargo.toml`

**Tests:**
- Existing tests should still pass (functions return same types)
- Add test: two consecutive calls produce different values (non-determinism check)

---

## Phase 3: Legacy FFI Dead Code (Finding 11)

**Goal:** Remove dead `process_packet()` function and all references.

### Changes

**File: `core/packet_processor/src/lib.rs`**
- Remove the `process_packet()` function (lines 2118-2132)
- Remove the `test_process_packet` test (lines 2143-2146)
- Keep the section header comment updated or remove if section is now empty

**File: `ios-macos/Shared/PacketProcessor-Bridging-Header.h`**
- Remove line 19: `PacketAction process_packet(const uint8_t *data, size_t len);`

**File: `docs/architecture_design.md`**
- Update line 73 reference to `process_packet` to reflect current `agent_process_packet` API

**File: `docs/walkthrough.md`**
- Update line 26 reference from `process_packet` to current API

**Verification:** No Swift callers exist (confirmed via grep of `ios-macos/` directory).

---

## Phase 4: UDP Length Sanity (Finding 15)

**Goal:** Drop malformed UDP packets where header claims < 8 bytes instead of forwarding empty payloads.

### Changes

**File: `app-connector/src/main.rs`** (~lines 1265-1270)
- After parsing `udp_len`, add explicit check:
  ```
  if udp_len < 8 {
      log::debug!("Malformed UDP: header length {} < minimum 8", udp_len);
      return Ok(());
  }
  ```
- Place this before the `saturating_sub(8)` call

**Tests:**
- Add unit test: crafted packet with `udp_len = 4` is dropped (returns Ok without forwarding)

---

## Build & Verify

After all changes:
```bash
cargo test --manifest-path intermediate-server/Cargo.toml
cargo test --manifest-path core/packet_processor/Cargo.toml
cargo test --manifest-path app-connector/Cargo.toml
cargo clippy --manifest-path intermediate-server/Cargo.toml -- -D warnings
cargo clippy --manifest-path core/packet_processor/Cargo.toml -- -D warnings
cargo clippy --manifest-path app-connector/Cargo.toml -- -D warnings
```

---

## Out of Scope

Findings deferred to their target tasks (see `research.md` for full details):
- Finding 2 (registration auth hardening) → Task 009
- Finding 3 (signaling session hijack) → Task 009
- Finding 5 (cross-tenant connector routing) → Task 009
- Finding 7 (local UDP injection) → Task 008
- Finding 9 (DATAGRAM size mismatch) → Task 011
- Finding 10 (endian bug investigation) → Task 011
- Finding 13 (hot-path allocations) → Task 011
- Finding 14 (recv buffer reuse) → Task 008
