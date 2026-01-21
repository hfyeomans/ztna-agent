# Plan: Swift Agent Integration

**Task ID:** 005a-swift-agent-integration
**Branch:** `feature/005a-swift-agent-integration`
**Depends On:** Task 005 (P2P Hole Punching) - All FFI functions available

---

## Goal

Update the existing macOS ZtnaAgent app to use the new QUIC Agent FFI, enabling:
1. QUIC connection to Intermediate Server
2. IP packet tunneling through QUIC DATAGRAMs
3. QAD (QUIC Address Discovery) support
4. Foundation for P2P hole punching (Phase 2)

---

## Phase 1: Update Bridging Header

Update `ios-macos/Shared/PacketProcessor-Bridging-Header.h` with all FFI functions.

### 1.1 Add P2P Connection Functions

```c
// ============================================================================
// P2P Connections
// ============================================================================

/// Connect to a Connector via P2P (direct connection).
AgentResult agent_connect_p2p(Agent* agent, const char* host, uint16_t port);

/// Check if P2P connection is established to given address.
bool agent_is_p2p_connected(const Agent* agent, const char* host, uint16_t port);

/// Poll for outbound UDP packets from P2P connections.
AgentResult agent_poll_p2p(Agent* agent, uint8_t* out_data, size_t* out_len,
                           uint8_t* out_ip, uint16_t* out_port);

/// Send IP packet through P2P connection.
AgentResult agent_send_datagram_p2p(Agent* agent, const uint8_t* data, size_t len,
                                     const uint8_t* dest_ip, uint16_t dest_port);
```

### 1.2 Add Hole Punching Functions

```c
// ============================================================================
// Hole Punching
// ============================================================================

/// Start hole punching for a service.
AgentResult agent_start_hole_punch(Agent* agent, const char* service_id);

/// Poll hole punching progress.
/// out_complete: 1 if complete, 0 if in progress.
AgentResult agent_poll_hole_punch(Agent* agent, uint8_t* out_ip, uint16_t* out_port,
                                   uint8_t* out_complete);

/// Get binding request to send for connectivity checks.
AgentResult agent_poll_binding_request(Agent* agent, uint8_t* out_data, size_t* out_len,
                                        uint8_t* out_ip, uint16_t* out_port);

/// Process received binding response.
AgentResult agent_process_binding_response(Agent* agent, const uint8_t* data, size_t len,
                                            const uint8_t* from_ip, uint16_t from_port);
```

### 1.3 Add Path Resilience Functions

```c
// ============================================================================
// Path Resilience
// ============================================================================

/// Poll for keepalive message to send.
AgentResult agent_poll_keepalive(Agent* agent, uint8_t* out_ip, uint16_t* out_port,
                                  uint8_t* out_data);

/// Get current active path type.
/// Returns: 0 = Direct, 1 = Relay, 2 = None
uint8_t agent_get_active_path(const Agent* agent);

/// Check if agent is in fallback mode.
bool agent_is_in_fallback(const Agent* agent);

/// Get path statistics.
AgentResult agent_get_path_stats(const Agent* agent,
                                  uint32_t* out_missed_keepalives,
                                  uint64_t* out_rtt_ms,
                                  uint8_t* out_in_fallback);
```

---

## Phase 2: Swift FFI Wrapper

Create `ios-macos/Shared/AgentWrapper.swift` to wrap the C FFI in Swift.

### 2.1 Error Handling

```swift
enum AgentError: Error {
    case invalidPointer
    case invalidAddress
    case connectionFailed
    case notConnected
    case bufferTooSmall
    case noData
    case quicError
    case panicCaught
    case unknown(Int32)

    init(_ result: AgentResult) {
        self = switch result {
        case AgentResultInvalidPointer: .invalidPointer
        case AgentResultInvalidAddress: .invalidAddress
        case AgentResultConnectionFailed: .connectionFailed
        case AgentResultNotConnected: .notConnected
        case AgentResultBufferTooSmall: .bufferTooSmall
        case AgentResultNoData: .noData
        case AgentResultQuicError: .quicError
        case AgentResultPanicCaught: .panicCaught
        default: .unknown(result.rawValue)
        }
    }
}
```

### 2.2 Agent Wrapper Class

```swift
final class AgentWrapper: @unchecked Sendable {
    private let agent: UnsafeMutablePointer<Agent>
    private let queue = DispatchQueue(label: "com.ztna.agent")

    init() throws {
        guard let ptr = agent_create() else {
            throw AgentError.panicCaught
        }
        agent = ptr
    }

    deinit {
        agent_destroy(agent)
    }

    func connect(host: String, port: UInt16) throws {
        let result = host.withCString { cstr in
            agent_connect(agent, cstr, port)
        }
        guard result == AgentResultOk else {
            throw AgentError(result)
        }
    }

    var isConnected: Bool {
        agent_is_connected(agent)
    }

    var state: AgentState {
        agent_get_state(agent)
    }

    func recv(data: Data, from: (ip: [UInt8], port: UInt16)) throws {
        try data.withUnsafeBytes { ptr in
            var ip = from.ip
            let result = agent_recv(agent, ptr.baseAddress?.assumingMemoryBound(to: UInt8.self),
                                    data.count, &ip, from.port)
            guard result == AgentResultOk else {
                throw AgentError(result)
            }
        }
    }

    func poll() -> (data: Data, port: UInt16)? {
        var buffer = [UInt8](repeating: 0, count: 1500)
        var len = buffer.count
        var port: UInt16 = 0

        let result = agent_poll(agent, &buffer, &len, &port)
        guard result == AgentResultOk else { return nil }

        return (Data(buffer.prefix(len)), port)
    }

    func sendDatagram(_ data: Data) throws {
        try data.withUnsafeBytes { ptr in
            let result = agent_send_datagram(agent, ptr.baseAddress?.assumingMemoryBound(to: UInt8.self),
                                              data.count)
            guard result == AgentResultOk else {
                throw AgentError(result)
            }
        }
    }

    func onTimeout() {
        agent_on_timeout(agent)
    }

    var timeoutMs: UInt64 {
        agent_timeout_ms(agent)
    }

    func getObservedAddress() -> (ip: [UInt8], port: UInt16)? {
        var ip = [UInt8](repeating: 0, count: 4)
        var port: UInt16 = 0

        let result = agent_get_observed_address(agent, &ip, &port)
        guard result == AgentResultOk else { return nil }

        return (ip, port)
    }
}
```

---

## Phase 3: Update PacketTunnelProvider

Rewrite `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`.

### 3.1 Properties

```swift
@objc class PacketTunnelProvider: NEPacketTunnelProvider {
    private let log = OSLog(subsystem: "com.ztna-agent", category: "Tunnel")

    private var agent: AgentWrapper?
    private var udpConnection: NWConnection?
    private var serverAddress: NWEndpoint.Host = "127.0.0.1"
    private var serverPort: NWEndpoint.Port = 4433
    private var timeoutTimer: DispatchSourceTimer?

    private let quicQueue = DispatchQueue(label: "com.ztna.quic")
}
```

### 3.2 Tunnel Startup

```swift
override func startTunnel(options: [String: NSObject]?,
                          completionHandler: @escaping (Error?) -> Void) {
    os_log("Starting tunnel...", log: log)

    // 1. Create Agent
    do {
        agent = try AgentWrapper()
    } catch {
        os_log("Failed to create agent: %{public}@", log: log, "\(error)")
        completionHandler(error)
        return
    }

    // 2. Configure tunnel settings
    let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "192.0.2.1")
    settings.ipv4Settings = NEIPv4Settings(addresses: ["100.64.0.1"],
                                            subnetMasks: ["255.255.255.255"])

    // Route specific IPs through tunnel (split tunnel)
    let route = NEIPv4Route(destinationAddress: "1.1.1.1", subnetMask: "255.255.255.255")
    settings.ipv4Settings?.includedRoutes = [route]
    settings.dnsSettings = NEDNSSettings(servers: ["8.8.8.8"])

    setTunnelNetworkSettings(settings) { [weak self] error in
        if let error = error {
            completionHandler(error)
            return
        }

        // 3. Create UDP connection to Intermediate Server
        self?.setupUDPConnection()

        // 4. Connect Agent to server
        self?.connectAgent()

        // 5. Start packet loops
        self?.startPacketLoop()
        self?.startTimeoutHandler()

        completionHandler(nil)
    }
}
```

### 3.3 UDP Connection Setup

```swift
private func setupUDPConnection() {
    let host = NWEndpoint.Host("127.0.0.1")  // TODO: Configure from options
    let port = NWEndpoint.Port(integerLiteral: 4433)

    udpConnection = NWConnection(host: host, port: port, using: .udp)
    udpConnection?.stateUpdateHandler = { [weak self] state in
        self?.handleConnectionState(state)
    }
    udpConnection?.start(queue: quicQueue)

    // Start receiving UDP packets
    receiveUDP()
}

private func receiveUDP() {
    udpConnection?.receiveMessage { [weak self] data, _, isComplete, error in
        if let data = data {
            self?.processIncomingUDP(data)
        }
        if !isComplete && error == nil {
            self?.receiveUDP()  // Continue receiving
        }
    }
}

private func processIncomingUDP(_ data: Data) {
    guard let agent = agent else { return }

    // Get source address (for now, hardcode server address)
    let fromIP: [UInt8] = [127, 0, 0, 1]  // TODO: Get from connection
    let fromPort: UInt16 = 4433

    do {
        try agent.recv(data: data, from: (fromIP, fromPort))
    } catch {
        os_log("Agent recv error: %{public}@", log: log, "\(error)")
    }

    // Poll for outgoing packets and send
    pollAndSend()
}
```

### 3.4 Packet Read Loop

```swift
private func startPacketLoop() {
    packetFlow.readPackets { [weak self] packets, protocols in
        guard let self = self else { return }

        for (i, packet) in packets.enumerated() {
            let protocolFamily = protocols[i].int32Value
            if protocolFamily == AF_INET {
                self.processOutboundPacket(packet)
            }
        }

        // Continue reading
        self.startPacketLoop()
    }
}

private func processOutboundPacket(_ data: Data) {
    guard let agent = agent else { return }

    do {
        try agent.sendDatagram(data)
    } catch {
        os_log("Send datagram error: %{public}@", log: log, "\(error)")
    }

    pollAndSend()
}

private func pollAndSend() {
    guard let agent = agent, let conn = udpConnection else { return }

    while let (data, _) = agent.poll() {
        conn.send(content: data, completion: .contentProcessed { error in
            if let error = error {
                os_log("UDP send error: %{public}@", log: self.log, "\(error)")
            }
        })
    }
}
```

### 3.5 Timeout Handler

```swift
private func startTimeoutHandler() {
    scheduleNextTimeout()
}

private func scheduleNextTimeout() {
    guard let agent = agent else { return }

    let ms = agent.timeoutMs
    guard ms > 0 else {
        // Check again in 100ms if no timeout pending
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
            self.scheduleNextTimeout()
        }
        return
    }

    timeoutTimer?.cancel()
    timeoutTimer = DispatchSource.makeTimerSource(queue: quicQueue)
    timeoutTimer?.schedule(deadline: .now() + .milliseconds(Int(ms)))
    timeoutTimer?.setEventHandler { [weak self] in
        self?.agent?.onTimeout()
        self?.pollAndSend()
        self?.scheduleNextTimeout()
    }
    timeoutTimer?.resume()
}
```

### 3.6 Agent Connection

```swift
private func connectAgent() {
    guard let agent = agent else { return }

    do {
        try agent.connect(host: "127.0.0.1", port: 4433)
        os_log("Agent connecting to server...", log: log)
    } catch {
        os_log("Agent connect error: %{public}@", log: log, "\(error)")
    }

    // Poll initial QUIC handshake packets
    pollAndSend()
}
```

### 3.7 Stop Tunnel

```swift
override func stopTunnel(with reason: NEProviderStopReason,
                         completionHandler: @escaping () -> Void) {
    os_log("Stopping tunnel: %{public}d", log: log, reason.rawValue)

    timeoutTimer?.cancel()
    udpConnection?.cancel()
    agent = nil

    completionHandler()
}
```

---

## Phase 4: Build Configuration

### 4.1 Build Rust Library

```bash
# Add macOS target if needed
rustup target add aarch64-apple-darwin

# Build release library
cd core/packet_processor
cargo build --release --target aarch64-apple-darwin

# Library output
ls target/aarch64-apple-darwin/release/libpacket_processor.a
```

### 4.2 Xcode Configuration

1. **Add Library:**
   - Drag `libpacket_processor.a` to Extension target
   - Or set Build Settings → Library Search Paths

2. **Linker Flags:**
   ```
   Other Linker Flags: -lpacket_processor
   ```

3. **Header Search Paths:**
   ```
   $(PROJECT_DIR)/../Shared
   ```

4. **Bridging Header:**
   ```
   $(PROJECT_DIR)/../Shared/PacketProcessor-Bridging-Header.h
   ```

### 4.3 Build Script (Optional)

Add Run Script phase to build Rust before Swift:
```bash
cd "${PROJECT_DIR}/../../core/packet_processor"
cargo build --release --target aarch64-apple-darwin
```

---

## Phase 5: Testing

### 5.1 Local Test Setup

```bash
# Terminal 1: Echo Server
cd tests/e2e/fixtures/echo-server
cargo run -- 9999

# Terminal 2: Intermediate Server
cd intermediate-server
cargo run -- --cert certs/cert.pem --key certs/key.pem

# Terminal 3: App Connector
cd app-connector
cargo run -- --server 127.0.0.1:4433 --service test-svc --forward 127.0.0.1:9999
```

### 5.2 macOS App Testing

1. Open `ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj` in Xcode
2. Build and run the app
3. Click "Start" button
4. Check Console.app for logs:
   - `Starting tunnel...`
   - `Agent connecting to server...`
   - Connection established messages

### 5.3 Verification Checklist

- [ ] App builds without errors
- [ ] Extension loads and starts
- [ ] Agent connects to Intermediate (logs show "Connected")
- [ ] QAD works (observed address logged)
- [ ] Outbound packets are tunneled
- [ ] Inbound packets are received
- [ ] Timeout handling works (connection stays alive)
- [ ] Stop button cleanly disconnects

---

## Future Work (Phase 2)

After basic relay works, add P2P support:

1. Start hole punching on demand
2. Process binding requests/responses
3. Switch to direct path when available
4. Keepalive for path health
5. Automatic fallback to relay

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| FFI memory safety | Use Swift's `withUnsafe*` APIs correctly |
| Thread safety | Serialize all FFI calls on single queue |
| UDP packet loss | QUIC handles retransmission |
| Build complexity | Document build steps clearly |
| Debug difficulty | Add extensive logging |
