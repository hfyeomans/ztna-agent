# Task 015: Oracle Quick Fixes — Todo

**Description:** Implementation checklist for the 4 quick-fix oracle findings.

**Purpose:** Track granular progress through each fix phase.

---

## Phase 1: IPv6 QAD Panic (Finding 6 — High)

- [ ] Change `build_observed_address()` return type to `Option<Vec<u8>>` in `intermediate-server/src/qad.rs`
- [ ] Replace `panic!()` with `log::warn!()` + `return None`
- [ ] Update IPv4 path to return `Some(msg)`
- [ ] Update call-site `send_qad()` in `intermediate-server/src/main.rs` to handle `Option`
- [ ] Update existing test `test_build_observed_address` for new return type
- [ ] Add test: IPv6 SocketAddr returns None without panicking
- [ ] Run `cargo test --manifest-path intermediate-server/Cargo.toml`

## Phase 2: Predictable P2P Identifiers (Finding 8 — Medium)

- [ ] Replace `generate_session_id()` in `core/packet_processor/src/p2p/signaling.rs` with `ring::rand::SystemRandom`
- [ ] Replace `generate_transaction_id()` in `core/packet_processor/src/p2p/connectivity.rs` with `ring::rand::SystemRandom`
- [ ] Remove unused `std::time` / `std::process` imports from both files
- [ ] Add test: consecutive session IDs differ
- [ ] Add test: consecutive transaction IDs differ
- [ ] Run `cargo test --manifest-path core/packet_processor/Cargo.toml`

## Phase 3: Legacy FFI Dead Code (Finding 11 — Medium)

- [ ] Remove `process_packet()` function from `core/packet_processor/src/lib.rs`
- [ ] Remove `test_process_packet` test from `core/packet_processor/src/lib.rs`
- [ ] Remove `process_packet` declaration from `ios-macos/Shared/PacketProcessor-Bridging-Header.h`
- [ ] Update `docs/architecture_design.md` reference (line 73)
- [ ] Update `docs/walkthrough.md` reference (line 26)
- [ ] Run `cargo test --manifest-path core/packet_processor/Cargo.toml`

## Phase 4: UDP Length Sanity (Finding 15 — Low)

- [ ] Add `if udp_len < 8` early return with log in `app-connector/src/main.rs` (before `saturating_sub`)
- [ ] Add test: crafted packet with udp_len < 8 is dropped
- [ ] Run `cargo test --manifest-path app-connector/Cargo.toml`

## Final Verification

- [ ] Run clippy on all 3 crates (intermediate-server, packet_processor, app-connector)
- [ ] Run full test suites on all 3 crates
- [ ] Oracle post-implementation review

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
