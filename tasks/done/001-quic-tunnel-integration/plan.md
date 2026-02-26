# Implementation Plan: QUIC Tunnel Integration

**Task ID:** 001-quic-tunnel-integration
**Status:** In Progress (Phase 2 Complete, Phase 3 Next)

## Architectural Goal: Direct P2P First

**Primary objective:** Establish direct peer-to-peer QUIC connections between Agent and App Connector.

**The Intermediate System serves two purposes:**
1. **Bootstrap:** Initial connection establishment, address discovery (QAD)
2. **Fallback:** Relay traffic when NAT/firewall prevents direct connection

```
PRIORITY 1 (Goal):     Agent ◄────── Direct QUIC ──────► Connector
PRIORITY 2 (Fallback): Agent ◄──► Intermediate ◄──► Connector
```

**Implementation approach:** Build relay infrastructure first (Phases 3-5), then add hole punching to achieve direct P2P (Phase 6). This ensures we always have a working fallback while optimizing for the direct path.

---

## Phase 0: Build System Fix ✅ COMPLETE

**Problem:** swift-frontend hangs during compilation with explicit modules enabled.
**Root Cause:** Compiler deadlock between Explicit Modules and NonisolatedNonsendingByDefault.
**Resolution:** `SWIFT_ENABLE_EXPLICIT_MODULES = NO` + clear caches.
**Research:** See `research.md` for full analysis.

### 0.1 Environment Reset ✅
- [x] Clear DerivedData: `rm -rf ~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*`
- [x] Clear Module Cache: `rm -rf ~/Library/Developer/Xcode/ModuleCache.noindex/`

### 0.2 Build Setting Fixes ✅
- [x] `SWIFT_ENABLE_EXPLICIT_MODULES = NO` already set for Extension target
- [x] No bridging header was set (added back for FFI)

### 0.3 Verify Build ✅
- [x] Build from xcodebuild: BUILD SUCCEEDED
- [x] No swift-frontend hang
- [x] Test: start tunnel, ping 1.1.1.1 - packets captured

### 0.4 Re-enable Rust FFI ✅
- [x] Added bridging header to Extension target (Debug + Release)
- [x] Restored FFI calls in `PacketTunnelProvider.swift`
- [x] Verified: `_process_packet` symbol linked, packets processed

### 0.5 Commit & Document ✅
- [x] Commit all modernization changes with build fix (958ce3f)
- [ ] File Apple Feedback about explicit modules + NetworkExtension hang (optional)

---

## Phase 1: Rust Core — QUIC Client (Agent Side) ✅ COMPLETE

### 1.1 Add quiche dependency
- Add `quiche` to `core/packet_processor/Cargo.toml`
- Configure for static linking (no BoringSSL dynamic deps)
- Build and verify compilation

### 1.2 Create Agent QUIC state machine
- Implement `Agent` struct with lifecycle:
  - `agent_create() -> *mut Agent`
  - `agent_destroy(agent: *mut Agent)`
  - `agent_connect(agent, server_addr) -> Result`
  - `agent_process_packet(agent, data, len) -> Action`
  - `agent_poll(agent) -> Vec<UdpPacket>` (outbound QUIC packets)
  - `agent_recv(agent, data, len)` (inbound QUIC packets from network)

### 1.3 QUIC DATAGRAM support
- Use QUIC DATAGRAM frames for IP packet encapsulation (not streams)
- Avoids head-of-line blocking for UDP-like traffic
- Configure `quiche::Config` with `enable_dgram(true, ...)`

### 1.4 Panic safety ✅
- Wrap all FFI entry points with `std::panic::catch_unwind`
- Return safe defaults on panic

---

## Phase 1.5: Code Quality Fixes ✅ COMPLETE

**Commit:** 229448b

### Rust Fixes
- [x] **Fix connection ID generation** - now uses `ring::rand::SystemRandom`
- [x] **Remove dead code** - deleted OutboundPacket, outbound_queue, current_outbound, empty if-block

### Swift Fixes
- [x] **Fix data race on `isRunning`** - now uses `OSAllocatedUnfairLock<Bool>`

### Deferred to Phase 2.5
- [ ] Buffer reuse optimizations in `recv()` and `poll()`

---

## Phase 2: Swift Integration — UDP I/O ✅ COMPLETE

**Commit:** 286df2a
**Verified:** Tunnel starts, packets captured, agent correctly reports "not connected" (expected - no server yet)

### Implementation Summary
- [x] Agent lifecycle (create/destroy) in PacketTunnelProvider
- [x] NWConnection UDP socket for QUIC transport
- [x] Send loop: `agent_poll()` → UDP send
- [x] Receive loop: UDP recv → `agent_recv()`
- [x] DispatchSourceTimer for quiche timeouts
- [x] Packet tunneling via `agent_send_datagram()`
- [x] Connection state monitoring and logging
- [x] Server address hardcoded (127.0.0.1:4433 for local testing)

---

## Phase 3: Intermediate System (Relay Server)

### 3.1 Create standalone Rust binary
- New crate: `intermediate-server/`
- Uses `quiche` as QUIC server
- Listens on public UDP port

### 3.2 Implement Address Discovery (QAD)
- On client connect: observe source IP:Port from UDP packet
- Send `OBSERVED_ADDRESS` in a custom application frame or initial stream
- Client now knows its public address without STUN

### 3.3 Basic relay mode
- Accept connections from Agents and Connectors
- Route DATAGRAMs between matched pairs
- Use connection metadata (auth token, destination ID) for routing

---

## Phase 4: Application Connector

### 4.1 Create connector binary
- New crate: `app-connector/`
- QUIC client that dials Intermediate System
- Registers as destination for specific service

### 4.2 Decapsulate and forward
- Receive DATAGRAM (encapsulated IP packet)
- For MVP: extract payload and forward to localhost TCP/UDP port
- For full implementation: inject into local TUN or NAT

---

## Phase 5: End-to-End Testing

### 5.1 Local testing setup
- Run Intermediate System locally
- Run App Connector pointing to local service (e.g., `nc -l 8080`)
- Connect Agent, send traffic to routed IP
- Verify data reaches local service

### 5.2 NAT testing
- Deploy Intermediate System to cloud (public IP)
- Test Agent behind NAT
- Verify Address Discovery reports correct public IP

---

## Phase 6: Direct P2P via Hole Punching ← PRIMARY GOAL

**This phase achieves the architectural goal: direct peer-to-peer connectivity.**

### 6.1 Address Exchange Protocol
- [ ] Define message format for peer address exchange via Intermediate
- [ ] Agent receives Connector's public IP:Port (from QAD on Connector's connection)
- [ ] Connector receives Agent's public IP:Port (from QAD on Agent's connection)
- [ ] Intermediate acts as signaling channel only

### 6.2 Simultaneous Open (Hole Punching)
- [ ] Agent sends QUIC packets to Connector's public IP:Port
- [ ] Connector sends QUIC packets to Agent's public IP:Port
- [ ] Both send simultaneously to create NAT bindings
- [ ] Implement retry logic with exponential backoff
- [ ] Detect successful direct path establishment

### 6.3 QUIC Connection Migration
- [ ] Migrate existing relay connection to direct path
- [ ] Use quiche connection migration API
- [ ] Verify packet delivery continues seamlessly
- [ ] Handle migration failures gracefully (fall back to relay)

### 6.4 Path Selection Logic
- [ ] Prefer direct path when available
- [ ] Monitor path quality (latency, packet loss)
- [ ] Automatic fallback to relay if direct path degrades
- [ ] Re-attempt hole punch on network changes (WiFi → cellular)

### 6.5 NAT Type Detection (Optional Enhancement)
- [ ] Detect NAT type (Full Cone, Restricted, Symmetric)
- [ ] Skip hole punch attempts for Symmetric NAT (will fail)
- [ ] Optimize hole punch timing based on NAT characteristics

### Success Criteria for Phase 6
- [ ] Agent and Connector establish direct QUIC connection
- [ ] Intermediate only used for initial signaling
- [ ] Latency matches direct network path (no relay hop)
- [ ] Graceful fallback to relay when hole punch fails

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `quiche` | latest | QUIC protocol implementation |
| `ring` | (quiche dep) | Cryptography |
| `mio` | optional | Event loop for server |
| `etherparse` | 0.13 | IP packet parsing |

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| quiche static build complexity | Use vendored BoringSSL, test on CI |
| DATAGRAM MTU issues | Fragment large packets or configure tunnel MTU |
| Timer driving in extension | Use `DispatchSource.timer` in Swift |
| Sandbox restrictions | All I/O through standard APIs (NWConnection/sockets) |

---

## Success Criteria

### Relay Mode (Phases 3-5)
1. Agent connects to Intermediate via QUIC
2. Agent learns its public IP via QAD (no STUN)
3. Ping to routed IP reaches App Connector via relay
4. Round-trip latency < 100ms via relay

### Direct P2P Mode (Phase 6 - Primary Goal)
5. Agent and Connector establish direct QUIC connection via hole punching
6. Intermediate used only for initial signaling, not data relay
7. Round-trip latency matches direct network path (< 50ms local, varies by distance)
8. Automatic fallback to relay when direct path unavailable
