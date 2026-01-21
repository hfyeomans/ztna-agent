# Task State: Swift Agent Integration

**Task ID:** 005a-swift-agent-integration
**Status:** Not Started
**Branch:** `feature/005a-swift-agent-integration`
**Last Updated:** 2026-01-20

---

## Overview

Update the existing macOS ZtnaAgent app to use the new QUIC Agent FFI from `core/packet_processor`. This enables the macOS app to establish real QUIC connections and tunnel IP packets through the ZTNA infrastructure.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Not Started

### Prerequisites ✅ READY
- [x] Task 005 (P2P Hole Punching) Phases 0-5 complete
- [x] All 79 unit tests passing in packet_processor
- [x] FFI functions implemented and documented
- [x] Bridging header exists (needs updates)
- [x] macOS Agent app exists (needs updates)

---

## Problem Statement

The macOS ZtnaAgent app exists and has a working UI with Start/Stop buttons, but:

1. **PacketTunnelProvider uses old API:** Line 84 calls `process_packet()` which is the original "hello world" function that just filters packets
2. **No QUIC connection:** The app doesn't connect to the Intermediate Server
3. **No packet tunneling:** IP packets aren't sent through QUIC DATAGRAMs
4. **Missing FFI declarations:** Bridging header doesn't expose P2P, hole punching, or resilience functions

---

## What Exists

### Working Components

| Component | Status | Location |
|-----------|--------|----------|
| SwiftUI App | ✅ Works | `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` |
| VPNManager | ✅ Works | Same file - handles NETunnelProviderManager |
| Start/Stop Buttons | ✅ Works | UI triggers tunnel start/stop |
| PacketTunnelProvider | ⚠️ Outdated | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| Bridging Header | ⚠️ Incomplete | `ios-macos/Shared/PacketProcessor-Bridging-Header.h` |

### Missing Components

| Component | Status | Needed For |
|-----------|--------|------------|
| AgentWrapper.swift | ❌ Missing | Swift-friendly FFI wrapper |
| UDP socket handling | ❌ Missing | QUIC packet transport |
| Timeout management | ❌ Missing | QUIC connection health |
| P2P FFI declarations | ❌ Missing | Direct connections |
| Resilience FFI declarations | ❌ Missing | Path management |

---

## What's Done

Nothing yet - task not started.

---

## What's Next

1. **Phase 1: Update Bridging Header**
   - Add P2P connection function declarations
   - Add hole punching function declarations
   - Add path resilience function declarations

2. **Phase 2: Swift FFI Wrapper**
   - Create AgentWrapper.swift
   - Wrap all FFI calls with Swift error handling
   - Handle memory management correctly

3. **Phase 3: Update PacketTunnelProvider**
   - Replace `process_packet()` with Agent struct
   - Add UDP socket for QUIC transport
   - Implement packet read/write loops
   - Add timeout handling

4. **Phase 4: Build Configuration**
   - Build Rust library for macOS
   - Configure Xcode project
   - Link library to Extension target

5. **Phase 5: Testing**
   - Local test with Intermediate + Connector
   - Verify end-to-end packet flow
   - Test connection health

---

## Phase Summary

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Bridging Header | 🔲 Not Started | Add all FFI declarations |
| Phase 2: Swift Wrapper | 🔲 Not Started | Create AgentWrapper.swift |
| Phase 3: PacketTunnelProvider | 🔲 Not Started | Full rewrite |
| Phase 4: Build Configuration | 🔲 Not Started | Xcode + Cargo |
| Phase 5: Testing | 🔲 Not Started | Local E2E |
| Phase 6: Documentation | 🔲 Not Started | |
| Phase 7: PR & Merge | 🔲 Not Started | |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 005 (P2P Hole Punching) | ✅ Ready | FFI functions available |
| Rust packet_processor | ✅ Ready | 79 tests passing |
| Xcode project | ✅ Exists | Needs configuration |
| Intermediate Server | ✅ Ready | For testing |
| App Connector | ✅ Ready | For testing |

---

## Key Files

| File | Status | Purpose |
|------|--------|---------|
| `PacketProcessor-Bridging-Header.h` | ⚠️ Incomplete | C declarations for FFI |
| `AgentWrapper.swift` | ❌ Create | Swift wrapper |
| `PacketTunnelProvider.swift` | ⚠️ Outdated | Tunnel logic |
| `libpacket_processor.a` | ⚠️ Build | Rust static library |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read `tasks/_context/components.md` for component status
3. Read this file for task state
4. Read `plan.md` for implementation details
5. Check `todo.md` for current progress
6. Create branch: `git checkout -b feature/005a-swift-agent-integration`
7. Start with Phase 1: Update Bridging Header

---

## Testing Commands

```bash
# Build Rust library
cd core/packet_processor
cargo build --release --target aarch64-apple-darwin

# Start test infrastructure
cd tests/e2e
./scenarios/basic-connectivity.sh

# Build Xcode project
xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
  -scheme ZtnaAgent -configuration Debug build
```
