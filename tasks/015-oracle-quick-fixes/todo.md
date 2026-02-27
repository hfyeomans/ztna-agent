# Task 015: Oracle Quick Fixes — Todo

**Description:** Implementation checklist for the 4 quick-fix oracle findings.

**Purpose:** Track granular progress through each fix phase.

---

## Phase 1: IPv6 QAD Panic (Finding 6 — High)

- [x] Change `build_observed_address()` return type to `Option<Vec<u8>>` in `intermediate-server/src/qad.rs`
- [x] Replace `panic!()` with `log::warn!()` + `return None`
- [x] Update IPv4 path to return `Some(msg)`
- [x] Update call-site `send_qad()` in `intermediate-server/src/main.rs` to handle `Option`
- [x] Update existing test `test_build_observed_address` for new return type
- [x] Add test: IPv6 SocketAddr returns None without panicking (`test_ipv6_returns_none`)
- [x] Run `cargo test --manifest-path intermediate-server/Cargo.toml` — 41 pass

## Phase 2: Predictable P2P Identifiers (Finding 8 — Medium)

- [x] Replace `generate_session_id()` in `core/packet_processor/src/p2p/signaling.rs` with `ring::rand::SystemRandom`
- [x] Replace `generate_transaction_id()` in `core/packet_processor/src/p2p/connectivity.rs` with `ring::rand::SystemRandom`
- [x] Remove unused `std::time` / `std::process` imports from both files
- [x] Add test: consecutive session IDs differ (`test_session_id_uniqueness`)
- [x] Add test: consecutive transaction IDs differ (`test_transaction_id_uniqueness`)
- [x] Run `cargo test --manifest-path core/packet_processor/Cargo.toml` — 84 pass

## Phase 3: Legacy FFI Dead Code (Finding 11 — Medium)

- [x] Remove `process_packet()` function from `core/packet_processor/src/lib.rs`
- [x] Remove `test_process_packet` test from `core/packet_processor/src/lib.rs`
- [x] Remove `PacketAction` enum from `core/packet_processor/src/lib.rs` (no remaining references)
- [x] Remove `process_packet` declaration + `PacketAction` typedef from `ios-macos/Shared/PacketProcessor-Bridging-Header.h`
- [x] Update `docs/architecture_design.md` reference (line 73)
- [x] Update `docs/walkthrough.md` reference (line 26)
- [x] Run `cargo test --manifest-path core/packet_processor/Cargo.toml` — 84 pass

## Phase 4: UDP Length Sanity (Finding 15 — Low)

- [x] Add `if udp_len < 8` early return with log in `app-connector/src/main.rs` (before `saturating_sub`)
- [x] Add test: crafted packet with udp_len < 8 is dropped (`test_malformed_udp_length_detected`)
- [x] Run `cargo test --manifest-path app-connector/Cargo.toml` — 21 pass

## Final Verification

- [x] Run clippy on all 3 crates — 0 warnings (intermediate-server, packet_processor, app-connector)
- [x] Run full test suites on all 3 crates — 143 tests pass, 0 failures
- [x] Oracle post-implementation review (gpt-5.3-codex) — no defects identified

## Deferred: Incorporate Findings into Target Tasks

- [x] Add findings 2, 3, 5 to `tasks/009-multi-service-architecture/` (research, plan, todo, placeholder)
- [x] Add findings 7, 14 to `tasks/008-production-operations/` (research, plan, todo, placeholder)
- [x] Add findings 9, 10, 13 to `tasks/011-protocol-improvements/` (research, plan, todo, placeholder)
- [x] Update `tasks/_context/README.md` deferred items table
- [x] Oracle review of triage completeness and correctness
- [x] Fix Oracle feedback: _context/README.md premature ✅ marks → "→ Task 015"
- [x] Fix Oracle feedback: Task 015 placeholder.md missing findings 8/11/15
- [x] Fix Oracle feedback: Task 009 Finding 3 needs stronger connector binding
- [x] Fix Oracle feedback: Task 009 Finding 5 needs per-flow socket/metadata strategy (not just 4-tuple map)
- [x] Fix Oracle feedback: Task 015 research.md "Confirmed Fixed (5 findings)" heading → "(3 findings)"
