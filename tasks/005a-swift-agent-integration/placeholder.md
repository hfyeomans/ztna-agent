# Placeholder Documentation: Swift Agent Integration

**Task ID:** 005a-swift-agent-integration
**Last Updated:** 2026-01-20

---

## Purpose

This document tracks intentional placeholder/scaffolding code in the codebase that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

### None Currently

Task has not started implementation yet.

---

## Placeholders to Create During Implementation

The following placeholders are anticipated during implementation:

### 1. Server Address Configuration

**Location:** `PacketTunnelProvider.swift`
**Purpose:** Currently hardcodes `127.0.0.1:4433` for local testing
**Action:** Add configuration via app IPC or tunnel options
**Priority:** Medium (post-MVP)

### 2. P2P Integration

**Location:** `AgentWrapper.swift`
**Purpose:** P2P methods implemented but not wired to UI
**Action:** Add P2P triggering based on service access
**Priority:** Medium (after basic tunnel works)

### 3. Keepalive Integration

**Location:** `PacketTunnelProvider.swift`
**Purpose:** Keepalive polling not integrated into main loop
**Action:** Add keepalive handling alongside timeout
**Priority:** Low (after P2P works)

### 4. iOS Device Support

**Location:** Build configuration
**Purpose:** Currently only builds for macOS
**Action:** Add iOS targets and signing
**Priority:** Low (post-MVP)

### 5. Inbound Packet Injection

**Location:** `PacketTunnelProvider.swift`
**Purpose:** Decapsulated packets need to be written back to packetFlow
**Action:** Implement `packetFlow.writePackets()` for responses
**Priority:** High (required for bidirectional communication)

---

## Completed Placeholders

None yet - task not started.

---

## Review Process

During implementation:
1. When creating placeholder code, document it here
2. Include file path, line number (if stable), purpose, and action needed
3. Assign priority: High (blocks MVP), Medium (post-MVP), Low (nice-to-have)
4. Review this file during Phase 7 (Documentation) to ensure nothing is forgotten
