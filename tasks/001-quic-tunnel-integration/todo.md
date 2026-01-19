# TODO: QUIC Tunnel Integration

**Task ID:** 001-quic-tunnel-integration

---

## Immediate: Build Fix ✅ COMPLETE

**Status:** RESOLVED (2026-01-18)
**See:** `research.md` for root cause analysis

### Environment Reset ✅
- [x] Clear DerivedData: `rm -rf ~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*`
- [x] Clear Module Cache: `rm -rf ~/Library/Developer/Xcode/ModuleCache.noindex/`

### Build Setting Fixes ✅
- [x] `SWIFT_ENABLE_EXPLICIT_MODULES = NO` was already set for Extension
- [x] Added bridging header to Extension (Debug + Release)

### Verify & Complete ✅
- [x] Build from xcodebuild (no hang, BUILD SUCCEEDED)
- [x] Test: start tunnel, `ping 1.1.1.1`, packets logged
- [x] Restored Rust FFI calls in PacketTunnelProvider.swift
- [x] Verified `_process_packet` symbol linked
- [ ] Commit all modernization + build fix changes

---

## Completed (Modernization)

- [x] Commit current working MVP state
- [x] Modernize Swift code to Swift 6.2 patterns (@Observable, async/await)
- [x] Update PacketTunnelProvider to async lifecycle
- [x] Add IPv6 support to packet loop
- [x] Update state.md with current state
- [x] Research build hang issue (Gemini + Oracle)
- [x] Document findings in research.md

---

## QUIC Integration ✅ COMPLETE

- [x] Add `quiche` dependency to Cargo.toml
- [x] Verify quiche builds as static library for macOS arm64

---

## Phase 1: Rust QUIC Client ✅ COMPLETE

- [x] Create `Agent` struct with QUIC connection state
- [x] Implement FFI lifecycle: `agent_create`, `agent_destroy`
- [x] Implement `agent_connect(server_addr)`
- [x] Enable DATAGRAM support in quiche config
- [x] Implement `agent_send_datagram(data)` for IP encapsulation
- [x] Implement `agent_poll()` to get outbound UDP packets
- [x] Implement `agent_recv(data)` to feed inbound UDP to quiche
- [x] Implement `agent_on_timeout()` for timer handling
- [x] Add `#[repr(C)]` to all FFI enums (not u32 - C ABI)
- [x] Wrap all FFI with `catch_unwind` for panic safety
- [x] Update bridging header with new FFI functions
- [x] Rust tests pass (3/3)
- [x] Full Xcode project builds successfully

---

## Phase 1.5: Code Quality Fixes ← IN PROGRESS

**Based on code review findings - must complete before Phase 2**

### Rust Fixes (CRITICAL)
- [ ] Fix connection ID generation - use `ring::rand::SystemRandom` instead of time-based PRNG (lib.rs:307-318)

### Rust Fixes (MEDIUM - Dead Code Removal)
- [ ] Delete `outbound_queue` field (lib.rs:104)
- [ ] Delete `current_outbound` field (lib.rs:106)
- [ ] Delete `OutboundPacket` struct (lib.rs:77-85)
- [ ] Delete empty if-block in `recv()` (lib.rs:180-183)

### Swift Fixes (CRITICAL)
- [ ] Fix data race on `isRunning` flag (use `OSAllocatedUnfairLock` or actor isolation)

### Swift Fixes (LOW)
- [ ] Remove unreachable `default` case in switch (lines 82-83)

### Performance (Defer to Phase 2.5)
- [ ] Reuse buffer in `recv()` instead of `.to_vec()` (lib.rs:194)
- [ ] Reuse `scratch_buffer` in `poll()` (lib.rs:213)

---

## Phase 2: Swift UDP Integration

- [ ] Create UDP socket/NWConnection in PacketTunnelProvider
- [ ] Implement send loop: `agent_poll()` → UDP send
- [ ] Implement receive loop: UDP recv → `agent_recv()`
- [ ] Add timer for quiche timeout handling
- [ ] Wire `readPackets` to `agent_send_datagram()`
- [ ] Handle connection state changes (connected, failed, etc.)
- [ ] Add server address configuration (hardcoded for MVP)

---

## Phase 3: Intermediate System

- [ ] Create new crate `intermediate-server/`
- [ ] Implement QUIC server with quiche
- [ ] Extract client source IP:Port on connection
- [ ] Send OBSERVED_ADDRESS frame to client (QAD)
- [ ] Implement client registry (track connected agents/connectors)
- [ ] Implement DATAGRAM relay between matched pairs
- [ ] Add basic authentication (token-based for MVP)
- [ ] Test locally with agent connecting

---

## Phase 4: App Connector

- [ ] Create new crate `app-connector/`
- [ ] Implement QUIC client connecting to Intermediate
- [ ] Register as destination endpoint
- [ ] Receive DATAGRAMs from Intermediate
- [ ] Decapsulate IP payload
- [ ] Forward to configured local port (TCP/UDP)
- [ ] Test end-to-end: ping → agent → intermediate → connector → local service

---

## Phase 5: Testing & Validation

- [ ] Local end-to-end test script
- [ ] Verify QAD reports correct address
- [ ] Test with agent behind NAT (cloud intermediate)
- [ ] Measure round-trip latency
- [ ] Test reconnection after network change
- [ ] Document test procedures

---

## Code Quality

- [ ] Add Rust unit tests for packet encapsulation
- [ ] Add integration test for QUIC handshake
- [ ] Remove unused dependencies (env_logger if not used)
- [ ] Update documentation with QUIC architecture
- [x] AGENTS.md exists at project root with build/test commands

---

## Backlog (Future)

- [ ] P2P hole punching
- [ ] Connection migration on network change
- [ ] Multiple connector support
- [ ] Policy engine (allow/deny rules)
- [ ] iOS support
- [ ] Metrics/observability
- [ ] Production deployment guide

---

## Bugs / Issues

- [x] **swift-frontend hang** — RESOLVED ✅
  - Root cause: Explicit Modules + NonisolatedNonsendingByDefault deadlock
  - Fix: `SWIFT_ENABLE_EXPLICIT_MODULES = NO` for Extension target + clear caches
  - See: `research.md` for full analysis
- [ ] Debug build links to `.debug.dylib` — use Release for testing
- [ ] Need to delete VPN config between major changes (System Settings → VPN)

---

## Notes

- `quiche` uses BoringSSL — may need vendored build for static linking
- Network Extensions can't load dylibs — must use static `.a` only (RESOLVED)
- `println!` doesn't work in extensions — use Swift Logger (RESOLVED)
- Test with Release builds to avoid debug dylib issues
- Follow AGENTS.md patterns: use `fd`, `rg`, `sg` for searches
- No `// TODO` comments — document in placeholder.md instead
- Remove deprecated/legacy code instead of marking it
