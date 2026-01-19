# Project State: ZTNA Agent

**Last Updated:** 2026-01-18 (Phase 2 Complete)

## Overview

Zero Trust Network Access (ZTNA) agent for macOS that intercepts packets, encapsulates them in QUIC tunnels, and routes through an intermediate system to application connectors.

## Current Phase: Phase 3 - Intermediate System

### What's Done
- ✅ MVP packet interception working
- ✅ Swift 6.2 / macOS 26+ modernization
- ✅ Build system fixed (explicit modules disabled for Extension)
- ✅ **Phase 1: Rust QUIC Client** (commit 958ce3f)
- ✅ **Phase 1.5: Code Quality Fixes** (commit 229448b)
  - Fixed Rust connection ID generation (security - now uses `ring::rand::SystemRandom`)
  - Fixed Swift `isRunning` data race (now uses `OSAllocatedUnfairLock`)
  - Removed dead code (OutboundPacket struct, outbound_queue, etc.)
- ✅ **Phase 2: Swift UDP Integration** (commit pending)
  - Full QUIC agent integration in PacketTunnelProvider
  - NWConnection for UDP transport
  - Send/receive loops, timeout handling
  - Packet tunneling via agent_send_datagram
  - **Note:** Build verified, functional testing requires Phase 3 server

### What's Next
1. **Phase 3: Intermediate System** - Build QUIC server for testing and relay
   - Create `intermediate-server/` crate
   - Implement QUIC server with quiche (listens on 127.0.0.1:4433)
   - Accept agent connections, echo OBSERVED_ADDRESS (QAD)
   - Enable end-to-end testing of Phase 2 implementation
- See `todo.md` for detailed tasks

---

## Phase 1: Rust QUIC Client ✅ COMPLETE (2026-01-18)

Implemented full QUIC agent in Rust with FFI interface for Swift integration:

**Rust Implementation (`core/packet_processor/src/lib.rs` - ~700 lines):**
- `Agent` struct with QUIC connection state management
- quiche 0.22 library integration with DATAGRAM support
- QAD (QUIC Address Discovery) message parsing
- Full FFI API exposed to Swift:
  - `agent_create`, `agent_destroy` - lifecycle
  - `agent_connect`, `agent_is_connected`, `agent_get_state` - connection
  - `agent_recv`, `agent_poll` - packet I/O
  - `agent_send_datagram` - IP tunneling
  - `agent_on_timeout`, `agent_timeout_ms` - timer handling
  - `agent_get_observed_address` - QAD result
- Panic-safe FFI with `AssertUnwindSafe` wrappers
- 3 unit tests passing

**Bridging Header (`ios-macos/Shared/PacketProcessor-Bridging-Header.h`):**
- All agent FFI functions exposed to Swift
- `AgentState` and `AgentResult` enums for C interop
- Comprehensive documentation for Swift developers

**Build Verification:**
- Rust: `cargo build` ✅ and `cargo test` ✅ (3/3 pass)
- Xcode: Full project builds successfully ✅ (Extension + Host App)

---

## Session Resume Instructions

After restarting Claude Code with clangd plugin:

1. Read this file: `tasks/001-quic-tunnel-integration/state.md`
2. Read the todo list: `tasks/001-quic-tunnel-integration/todo.md`
3. Continue with **Phase 2: Swift UDP Integration**

Key files for Phase 2:
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - needs UDP integration
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - FFI definitions
- `core/packet_processor/src/lib.rs` - Rust agent implementation

### Build Blocker: RESOLVED ✅

**Problem:** swift-frontend entered UE (Uninterruptible Exit) state during compilation.

**Root Cause:** Compiler deadlock between Explicit Modules and NonisolatedNonsendingByDefault when compiling NetworkExtension targets.

**Fix Applied:**
- `SWIFT_ENABLE_EXPLICIT_MODULES = NO` for Extension target (was already set)
- Cleared DerivedData and ModuleCache
- Restored bridging header and Rust FFI calls

**Verified:** 2026-01-18 - Build succeeds, packets captured, Rust FFI working.

**Research:** See `research.md` for full Gemini + Oracle analysis

---

#### Toolchain Verified (NOT beta)

| Component | Version | Status |
|-----------|---------|--------|
| macOS | 26.2 (25C56) | Stable (Dec 12, 2025) |
| Xcode | 26.2 (17C52) | Stable (updated from 26.0) |
| Swift | 6.2.3 | Current stable |

---

#### Root Cause Analysis

The hang occurs when swift-frontend compiles `PacketTunnelProvider.swift` with NetworkExtension framework. Through systematic isolation testing:

| Hypothesis | Test Result |
|------------|-------------|
| Bridging header causes hang | ❌ **Disproven** — Removed all bridging header references; hang persists |
| Rust FFI causes hang | ❌ **Disproven** — Commented out all FFI calls; hang persists |
| Explicit modules cause hang | ⚠️ **Suspected** — All stuck processes use `-disable-implicit-swift-modules` |
| Concurrency features cause hang | ⚠️ **Suspected** — All have `-enable-upcoming-feature NonisolatedNonsendingByDefault` |

**Key Evidence**: A simple Swift file with `NEPacketTunnelProvider` compiles fine with `swiftc` directly (~5 seconds). The hang **only** occurs when building through Xcode with explicit modules enabled.

---

#### Root Cause Confirmed (Research 2026-01-18)

**Compiler deadlock** between:
1. **Explicit Modules** (`SWIFT_ENABLE_EXPLICIT_MODULES`) - pre-compiles module variants
2. **NonisolatedNonsendingByDefault** - changes nonisolated async function behavior

The hang occurs because:
- NetworkExtension is an `@objc` system framework
- Explicit Modules attempts to synthesize "sending" variants of Foundation/Network
- Compiler enters infinite recursion when processing system class inheritance
- Code itself is correct Swift 6.2 (verified by Oracle review)

**See:** `tasks/001-quic-tunnel-integration/research.md` for full analysis.

---

#### Changes Applied to Test (pending reboot)

**Removed from Extension target:**
- `SWIFT_OBJC_BRIDGING_HEADER` (all 4 occurrences in project)
- `SWIFT_APPROACHABLE_CONCURRENCY = YES`
- `SWIFT_UPCOMING_FEATURE_MEMBER_IMPORT_VISIBILITY = YES`

**Added to Extension target:**
- `SWIFT_ENABLE_EXPLICIT_MODULES = NO`

This follows the Xcode 26 release notes: *"Starting from Xcode 26, Swift explicit modules will be the default mode... When encountering severe issues, projects could opt-out by specifying `SWIFT_ENABLE_EXPLICIT_MODULES=NO`."*

---

#### Code Changes Made

**PacketTunnelProvider.swift**: Rust FFI calls temporarily commented out:
```swift
private func processPacket(_ data: Data, isIPv6: Bool) {
    // Temporarily bypass Rust FFI to isolate build hang
    logger.debug("Forwarding packet (\(data.count) bytes, IPv\(isIPv6 ? "6" : "4"))")
}
```

---

#### Next Steps After Reboot

1. Clear DerivedData and module caches
2. Build with explicit modules disabled
3. If successful:
   - File Apple Feedback about explicit modules + NetworkExtension hang
   - Re-enable features one by one to identify specific trigger
4. Restore Rust FFI calls once build works

---

## Architecture

```
┌─────────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
│   macOS Endpoint    │     │  Intermediate System │     │  App Connector      │
│                     │     │                      │     │                     │
│  ┌───────────────┐  │     │  - QUIC Server       │     │  - QUIC Client      │
│  │ SwiftUI App   │  │     │  - Address Discovery │     │  - Decapsulates     │
│  └───────┬───────┘  │     │  - Relay/Rendezvous  │     │  - Forwards to App  │
│          │          │     │                      │     │                     │
│  ┌───────▼───────┐  │     └──────────▲───────────┘     └──────────▲──────────┘
│  │ NEPacketTun.  │  │                │                            │
│  │ Provider      │──┼────────────────┴────────────────────────────┘
│  └───────┬───────┘  │           QUIC Tunnel (planned)
│          │ FFI      │
│  ┌───────▼───────┐  │
│  │ Rust Core     │  │
│  │ (quiche)      │  │
│  └───────────────┘  │
└─────────────────────┘
```

---

## Completed Work

### 1. Project Structure
- [x] Repository initialized with git
- [x] Xcode project with host app + Network Extension targets
- [x] Rust crate `packet_processor` with FFI exports
- [x] Documentation (architecture, implementation plan, research bookmarks)
- [x] Task folder structure (`tasks/001-quic-tunnel-integration/`)

### 2. macOS Network Extension
- [x] `NEPacketTunnelProvider` implementation
- [x] Virtual tunnel interface (`100.64.0.1`)
- [x] Split tunneling — routes only `1.1.1.1` through tunnel
- [x] Async packet read loop via `await packetFlow.readPackets()`
- [x] Modern async lifecycle (`startTunnel`, `stopTunnel`)
- [x] `isRunning` flag for clean shutdown
- [x] IPv6 packet handling support

### 3. Rust Packet Processor
- [x] Static library build (`libpacket_processor.a`)
- [x] C-compatible FFI: `process_packet(data, len) -> PacketAction`
- [x] IP packet parsing with `etherparse`
- [x] Drop/Forward decision framework
- [x] Removed `println!` (doesn't work in extensions)
- [x] Removed `cdylib` from Cargo.toml (static only)

### 4. Swift ↔ Rust Integration
- [x] Bridging header with `PacketAction` enum
- [x] Static library linked to Extension target
- [x] Verified FFI calls work in sandboxed extension

### 5. Host App (Modernized)
- [x] `@Observable` VPNManager (Swift 6.2 pattern)
- [x] async/await for VPN operations
- [x] Modern SwiftUI with `symbolEffect`, `buttonStyle(.borderedProminent)`
- [x] Start/Stop buttons with proper state management
- [x] VPN status observer via NotificationCenter

---

## Key Files

| Component | Path |
|-----------|------|
| Host App | `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` |
| Extension | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| Rust Core | `core/packet_processor/src/lib.rs` |
| Bridging Header | `ios-macos/Shared/PacketProcessor-Bridging-Header.h` |
| Architecture Doc | `docs/architecture.md` |

---

## Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Packet Interception | `NEPacketTunnelProvider` | Modern API, userspace, supported |
| QUIC Library | `quiche` | Sans-IO model fits packet loop |
| NAT Traversal | QUIC Address Discovery | Replace STUN with native QUIC |
| FFI Strategy | Static library (`.a`) | Required for sandboxed extensions |
| Logging | `Logger` (os framework) | Modern replacement for OSLog |
| Swift Concurrency | async/await | Modern Swift 6.2 patterns |
| State Management | `@Observable` | Modern SwiftUI (replaces ObservableObject) |

---

## Issues Resolved

1. **Extension crash on launch** — Was linking to `.dylib` instead of `.a`. Fixed by removing `cdylib` from Cargo.toml.
2. **"Missing protocol" error** — Was using `NEVPNManager` instead of `NETunnelProviderManager`.
3. **Tunnel stuck on "Starting"** — Extra entitlements (`app-proxy-provider`, `content-filter-provider`) causing rejection.
4. **Rust println! not visible** — Removed; logging now via Swift `os_log`.

---

## Environment

- **macOS**: 26.0 (Tahoe)
- **Xcode**: Latest
- **Swift**: 6.2
- **Rust**: Stable (2021 edition)
- **Target**: macOS 26+ (arm64)
- **SwiftUI**: Modern patterns (Observation, async/await)

---

## Test Procedure

1. Build Release: `xcodebuild -scheme ZtnaAgent -configuration Release build`
2. Launch app from DerivedData
3. Click "Install & Start VPN"
4. Run: `ping 1.1.1.1`
5. Verify logs: `log stream --predicate 'subsystem == "com.hankyeomans.ztna-agent"'`

Expected output:
```
Extension: Forwarding packet of size 84 (Rust)
```
