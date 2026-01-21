# TODO: Swift Agent Integration

**Task ID:** 005a-swift-agent-integration
**Branch:** `feature/005a-swift-agent-integration`
**Depends On:** Task 005 (P2P Hole Punching complete)
**Last Updated:** 2026-01-20

---

## Prerequisites

- [x] Task 005 (P2P Hole Punching) Phases 0-5 complete
- [x] All 79 unit tests passing in packet_processor
- [x] FFI functions implemented in lib.rs
- [ ] Create feature branch: `git checkout -b feature/005a-swift-agent-integration`

---

## Phase 1: Update Bridging Header

### 1.1 P2P Connection Functions
- [ ] Add `agent_connect_p2p()` declaration
- [ ] Add `agent_is_p2p_connected()` declaration
- [ ] Add `agent_poll_p2p()` declaration
- [ ] Add `agent_send_datagram_p2p()` declaration

### 1.2 Hole Punching Functions
- [ ] Add `agent_start_hole_punch()` declaration
- [ ] Add `agent_poll_hole_punch()` declaration
- [ ] Add `agent_poll_binding_request()` declaration
- [ ] Add `agent_process_binding_response()` declaration

### 1.3 Path Resilience Functions
- [ ] Add `agent_poll_keepalive()` declaration
- [ ] Add `agent_get_active_path()` declaration
- [ ] Add `agent_is_in_fallback()` declaration
- [ ] Add `agent_get_path_stats()` declaration

### 1.4 Verification
- [ ] Header compiles without errors
- [ ] Function signatures match Rust FFI exactly

---

## Phase 2: Swift FFI Wrapper

### 2.1 Create AgentWrapper.swift
- [ ] Create `ios-macos/Shared/AgentWrapper.swift`
- [ ] Implement `AgentError` enum matching `AgentResult`
- [ ] Implement `AgentWrapper` class with lifecycle management
- [ ] Implement `connect(host:port:)` method
- [ ] Implement `recv(data:from:)` method
- [ ] Implement `poll()` method returning optional tuple
- [ ] Implement `sendDatagram(_:)` method
- [ ] Implement `onTimeout()` method
- [ ] Implement `timeoutMs` computed property
- [ ] Implement `getObservedAddress()` method

### 2.2 P2P Methods (Optional for MVP)
- [ ] Implement `connectP2P(host:port:)` method
- [ ] Implement `isP2PConnected(host:port:)` method
- [ ] Implement `pollP2P()` method
- [ ] Implement `sendDatagramP2P(_:to:)` method

### 2.3 Hole Punching Methods (Optional for MVP)
- [ ] Implement `startHolePunch(serviceId:)` method
- [ ] Implement `pollHolePunch()` method
- [ ] Implement `pollBindingRequest()` method
- [ ] Implement `processBindingResponse(_:from:)` method

### 2.4 Verification
- [ ] AgentWrapper compiles without errors
- [ ] All methods properly handle memory
- [ ] Error cases throw appropriate errors

---

## Phase 3: Update PacketTunnelProvider

### 3.1 Properties and Lifecycle
- [ ] Add `agent: AgentWrapper?` property
- [ ] Add `udpConnection: NWConnection?` property
- [ ] Add `timeoutTimer: DispatchSourceTimer?` property
- [ ] Add `quicQueue: DispatchQueue` property
- [ ] Update `deinit` to clean up resources

### 3.2 Tunnel Startup
- [ ] Create Agent instance in `startTunnel()`
- [ ] Configure tunnel network settings
- [ ] Set up UDP connection to Intermediate Server
- [ ] Initiate Agent connection
- [ ] Start packet read loop
- [ ] Start timeout handler
- [ ] Call completion handler on success

### 3.3 UDP Connection
- [ ] Implement `setupUDPConnection()` method
- [ ] Implement `receiveUDP()` for incoming packets
- [ ] Implement `processIncomingUDP(_:)` to call `agent.recv()`
- [ ] Implement `pollAndSend()` to send QUIC packets

### 3.4 Packet Flow
- [ ] Implement `startPacketLoop()` for reading from packetFlow
- [ ] Implement `processOutboundPacket(_:)` to call `agent.sendDatagram()`
- [ ] Handle IPv4 packets (AF_INET check)
- [ ] Ignore non-IPv4 packets gracefully

### 3.5 Timeout Handling
- [ ] Implement `startTimeoutHandler()` method
- [ ] Implement `scheduleNextTimeout()` with dynamic timing
- [ ] Call `agent.onTimeout()` and `pollAndSend()` on timer fire
- [ ] Cancel timer on tunnel stop

### 3.6 Tunnel Shutdown
- [ ] Cancel timeout timer in `stopTunnel()`
- [ ] Cancel UDP connection
- [ ] Destroy agent (set to nil)
- [ ] Call completion handler

### 3.7 Verification
- [ ] PacketTunnelProvider compiles without errors
- [ ] No memory leaks (check with Instruments)
- [ ] All code paths handle errors

---

## Phase 4: Build Configuration

### 4.1 Rust Library Build
- [ ] Install `aarch64-apple-darwin` target: `rustup target add aarch64-apple-darwin`
- [ ] Build release library: `cargo build --release --target aarch64-apple-darwin`
- [ ] Verify library exists: `target/aarch64-apple-darwin/release/libpacket_processor.a`

### 4.2 Xcode Project Configuration
- [ ] Add `libpacket_processor.a` to Extension target
- [ ] Set Library Search Paths
- [ ] Set Header Search Paths
- [ ] Add `-lpacket_processor` to Other Linker Flags
- [ ] Verify bridging header is set for Extension target

### 4.3 Build Verification
- [ ] Extension target builds successfully
- [ ] App target builds successfully
- [ ] No undefined symbol errors

---

## Phase 5: Local Testing

### 5.1 Test Environment Setup
- [ ] Start Echo Server on port 9999
- [ ] Start Intermediate Server on port 4433
- [ ] Start App Connector connecting to Intermediate

### 5.2 App Testing
- [ ] Build and run ZtnaAgent app in Xcode
- [ ] Click "Start" button
- [ ] Verify tunnel starts (Console.app logs)
- [ ] Verify Agent connects to Intermediate
- [ ] Verify QAD address is received

### 5.3 Packet Flow Testing
- [ ] Configure test route (e.g., 1.1.1.1)
- [ ] Send traffic to routed address
- [ ] Verify packets appear in Intermediate logs
- [ ] Verify packets reach App Connector
- [ ] Verify response packets return

### 5.4 Connection Health Testing
- [ ] Let connection idle for 30+ seconds
- [ ] Verify connection stays alive (timeout handling works)
- [ ] Click "Stop" button
- [ ] Verify clean disconnect

---

## Phase 6: Documentation

- [ ] Update `docs/architecture.md` with Swift integration details
- [ ] Document build steps in README or separate guide
- [ ] Add troubleshooting section for common issues
- [ ] Update `tasks/_context/components.md` with 005a status

---

## Phase 7: PR & Merge

- [ ] Run all tests (Rust + Swift build)
- [ ] Update state.md with completion status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## MVP Deliverables Checklist

> Minimum viable for basic tunnel functionality

- [ ] Agent creates and connects to Intermediate
- [ ] UDP socket sends/receives QUIC packets
- [ ] IP packets are tunneled via DATAGRAMs
- [ ] QAD address is received
- [ ] Timeout handling keeps connection alive
- [ ] Start/Stop buttons work correctly
- [ ] App runs without crashes

---

## Deferred (Post-MVP)

> Can be added after basic tunnel works

- [ ] P2P hole punching integration
- [ ] Keepalive support
- [ ] Path selection (direct vs relay)
- [ ] Fallback handling
- [ ] iOS device support
- [ ] Configuration UI (server address, etc.)
- [ ] Certificate validation options

---

## Testing Commands

```bash
# Build Rust library
cd core/packet_processor
cargo build --release --target aarch64-apple-darwin

# Start test infrastructure
cd tests/e2e
./scenarios/basic-connectivity.sh

# View macOS logs
log show --predicate 'subsystem == "com.ztna-agent"' --last 5m

# Console.app filter
subsystem:com.ztna-agent
```

---

## Files to Modify/Create

| File | Action | Purpose |
|------|--------|---------|
| `ios-macos/Shared/PacketProcessor-Bridging-Header.h` | Modify | Add all FFI declarations |
| `ios-macos/Shared/AgentWrapper.swift` | Create | Swift wrapper for Agent FFI |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | Rewrite | Full QUIC integration |
| `ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj/project.pbxproj` | Modify | Build settings |

---

## Risk Tracking

| Risk | Status | Mitigation |
|------|--------|------------|
| FFI signature mismatch | 🔲 Open | Careful header/Rust alignment |
| Memory management bugs | 🔲 Open | Use Swift's safe APIs, test with Instruments |
| Thread safety issues | 🔲 Open | Serialize FFI calls on single queue |
| Build configuration | 🔲 Open | Document all Xcode settings |
| NWConnection limitations | 🔲 Open | Test UDP throughput |
