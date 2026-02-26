# Research: Swift Agent Integration

**Task ID:** 005a-swift-agent-integration
**Date:** 2026-01-20
**Branch:** `feature/005a-swift-agent-integration`

---

## Current State Analysis

### Existing macOS Agent Components

The macOS Agent already exists with functional UI, but is not using the new QUIC Agent FFI.

| File | Status | Purpose |
|------|--------|---------|
| `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` | ✅ Works | SwiftUI app with Start/Stop buttons |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | ⚠️ Outdated | Uses old `process_packet()` API |
| `ios-macos/Shared/PacketProcessor-Bridging-Header.h` | ⚠️ Incomplete | Missing P2P and resilience FFI |

### Current PacketTunnelProvider Implementation

The current Swift code (line 84) uses the legacy API:
```swift
return process_packet(baseAddress.assumingMemoryBound(to: UInt8.self), data.count)
```

This was the original "hello world" implementation that just filters packets. It does NOT:
- Create a QUIC connection
- Send packets through the tunnel
- Handle incoming QUIC data
- Support QAD, P2P, or keepalives

### Bridging Header Status

**Currently Exposed (Partial):**
```c
// Basic lifecycle
Agent* agent_create(void);
void agent_destroy(Agent* agent);
AgentState agent_get_state(const Agent* agent);

// Connection
AgentResult agent_connect(Agent* agent, const char* host, uint16_t port);
bool agent_is_connected(const Agent* agent);

// Packet I/O
AgentResult agent_recv(Agent* agent, const uint8_t* data, size_t len, ...);
AgentResult agent_poll(Agent* agent, uint8_t* out_data, size_t* out_len, ...);
AgentResult agent_send_datagram(Agent* agent, const uint8_t* data, size_t len);

// Timeout
void agent_on_timeout(Agent* agent);
uint64_t agent_timeout_ms(const Agent* agent);

// QAD
AgentResult agent_get_observed_address(const Agent* agent, ...);
```

**Missing from Header (Need to Add):**
```c
// P2P Connections
AgentResult agent_connect_p2p(Agent* agent, const char* host, uint16_t port);
bool agent_is_p2p_connected(const Agent* agent, const char* host, uint16_t port);
AgentResult agent_poll_p2p(Agent* agent, uint8_t* out_data, size_t* out_len, ...);
AgentResult agent_send_datagram_p2p(Agent* agent, const uint8_t* data, size_t len, ...);

// Hole Punching
AgentResult agent_start_hole_punch(Agent* agent, const char* service_id);
AgentResult agent_poll_hole_punch(Agent* agent, uint8_t* out_ip, uint16_t* out_port, ...);
AgentResult agent_poll_binding_request(Agent* agent, uint8_t* out_data, ...);
AgentResult agent_process_binding_response(Agent* agent, const uint8_t* data, ...);

// Path Resilience
AgentResult agent_poll_keepalive(Agent* agent, uint8_t* out_ip, uint16_t* out_port, ...);
uint8_t agent_get_active_path(const Agent* agent);
bool agent_is_in_fallback(const Agent* agent);
AgentResult agent_get_path_stats(const Agent* agent, ...);
```

---

## Swift NetworkExtension Architecture

### NEPacketTunnelProvider Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    NEPacketTunnelProvider Lifecycle                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  User taps "Start" → VPNManager.start()                                     │
│        │                                                                     │
│        ▼                                                                     │
│  startTunnel(options:completionHandler:)                                    │
│        │                                                                     │
│        ├── 1. Create Agent: agent_create()                                  │
│        ├── 2. Create UDP socket                                             │
│        ├── 3. Set tunnel network settings                                   │
│        ├── 4. Connect to Intermediate: agent_connect()                      │
│        ├── 5. Start packet read loop                                        │
│        ├── 6. Start UDP receive loop                                        │
│        ├── 7. Start timeout handler                                         │
│        └── 8. Call completionHandler(nil)                                   │
│                                                                              │
│  Running State:                                                              │
│        │                                                                     │
│        ├── packetFlow.readPackets → agent_send_datagram() → agent_poll()   │
│        │                                                                     │
│        ├── UDP recv → agent_recv() → process QUIC → agent_poll()           │
│        │                                                                     │
│        └── Timer tick → agent_on_timeout() → agent_poll()                   │
│                                                                              │
│  User taps "Stop" → stopTunnel(with:completionHandler:)                     │
│        │                                                                     │
│        ├── agent_destroy()                                                  │
│        ├── Close UDP socket                                                 │
│        └── Call completionHandler()                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### UDP Socket Requirements

The Agent FFI expects Swift to manage the UDP socket:

1. **Socket Creation:** `NWConnection` with UDP or raw `socket()` API
2. **Receiving:** When UDP packet arrives, call `agent_recv()`
3. **Sending:** After `agent_poll()` returns data, send via UDP socket
4. **Timeout:** Timer fires → `agent_on_timeout()` → `agent_poll()`

### Packet Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OUTBOUND FLOW (App → Network)                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  App generates IP packet                                                     │
│         │                                                                    │
│         ▼                                                                    │
│  packetFlow.readPackets { packets in ... }                                  │
│         │                                                                    │
│         ▼                                                                    │
│  agent_send_datagram(agent, packet.data, packet.len)  // Encapsulate       │
│         │                                                                    │
│         ▼                                                                    │
│  agent_poll(agent, &buffer, &len, &port)  // Get QUIC packet               │
│         │                                                                    │
│         ▼                                                                    │
│  udpSocket.send(buffer, to: intermediateServer)  // Send to server         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                         INBOUND FLOW (Network → App)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  UDP packet received from server                                             │
│         │                                                                    │
│         ▼                                                                    │
│  agent_recv(agent, data, len, from_ip, from_port)  // Process QUIC         │
│         │                                                                    │
│         ▼                                                                    │
│  agent_poll(agent, &buffer, &len, &port)  // Get response packets          │
│         │                                                                    │
│         ▼                                                                    │
│  udpSocket.send(buffer, to: server)  // Send any pending QUIC data         │
│         │                                                                    │
│  NOTE: Decapsulated IP packets come through DATAGRAM callback               │
│  (Currently the Rust side handles this internally)                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Rust Library Build Requirements

### Current Build Targets

```bash
# Check current targets
rustup target list --installed
```

### Required Targets

| Platform | Target Triple | Notes |
|----------|---------------|-------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` | Primary development |
| macOS (Intel) | `x86_64-apple-darwin` | Legacy support |
| iOS (Device) | `aarch64-apple-ios` | For iPhone/iPad |
| iOS Simulator (AS) | `aarch64-apple-ios-sim` | For testing |

### Build Commands

```bash
# Install required targets
rustup target add aarch64-apple-darwin
rustup target add aarch64-apple-ios
rustup target add aarch64-apple-ios-sim

# Build static library for macOS
cd core/packet_processor
cargo build --release --target aarch64-apple-darwin

# Build for iOS device
cargo build --release --target aarch64-apple-ios

# Output location
ls target/aarch64-apple-darwin/release/libpacket_processor.a
```

### Xcode Integration

1. Add `libpacket_processor.a` to Xcode project
2. Set Library Search Paths to `$(PROJECT_DIR)/../core/packet_processor/target/$(CURRENT_ARCH)-apple-$(PLATFORM_NAME)/release`
3. Add `-lpacket_processor` to Other Linker Flags
4. Set bridging header path

---

## Implementation Considerations

### Thread Safety

- NetworkExtension runs on a background thread
- Agent FFI is `Send` (can be called from any thread)
- All FFI calls must be from the same thread (not thread-safe internally)
- Use `DispatchQueue` to serialize access

### Error Handling

```swift
// Swift wrapper pattern
func agentConnect(host: String, port: UInt16) throws {
    let result = agent_connect(agent, host, port)
    guard result == AgentResultOk else {
        throw AgentError(result)
    }
}
```

### Memory Management

- `agent_create()` returns owned pointer
- `agent_destroy()` must be called to avoid leak
- Use Swift's `deinit` or explicit cleanup

### Timeout Handling

```swift
// Timer-based timeout
func scheduleTimeout() {
    let ms = agent_timeout_ms(agent)
    guard ms > 0 else { return }

    DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(Int(ms))) {
        agent_on_timeout(self.agent)
        self.pollAndSend()
        self.scheduleTimeout()
    }
}
```

---

## Testing Strategy

### Local Testing (localhost)

1. Start Intermediate Server on localhost:4433
2. Start App Connector on localhost
3. Run macOS Agent app, click Start
4. Agent should connect and show QAD address

### Verification Points

- [ ] Agent connects to Intermediate (state = Connected)
- [ ] QAD returns observed address (127.0.0.1:XXXXX locally)
- [ ] IP packets are tunneled through QUIC
- [ ] Timeout handling works (connection stays alive)
- [ ] Stop button cleanly disconnects

### E2E Test Flow

```bash
# Terminal 1: Echo Server
cd tests/e2e/fixtures/echo-server && cargo run -- 9999

# Terminal 2: Intermediate Server
cd intermediate-server && cargo run -- --cert certs/cert.pem --key certs/key.pem

# Terminal 3: App Connector
cd app-connector && cargo run -- --server 127.0.0.1:4433 --service test-svc --forward 127.0.0.1:9999

# macOS: Run ZtnaAgent.app, click Start, send traffic to 1.1.1.1 (routed to tunnel)
```

---

## Related Documentation

- `docs/architecture.md` - System architecture with P2P details
- `tasks/005-p2p-hole-punching/plan.md` - P2P implementation details
- `core/packet_processor/src/lib.rs` - All FFI function signatures
- Apple: [NEPacketTunnelProvider](https://developer.apple.com/documentation/networkextension/nepackettunnelprovider)
