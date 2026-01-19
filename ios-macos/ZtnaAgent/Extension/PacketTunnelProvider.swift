import NetworkExtension
import Network
import os
import Darwin

final class PacketTunnelProvider: NEPacketTunnelProvider {

    private let logger = Logger(subsystem: "com.hankyeomans.ztna-agent", category: "Tunnel")

    // MARK: - Thread-Safe State

    /// Thread-safe running state using OSAllocatedUnfairLock
    private let runningLock = OSAllocatedUnfairLock(initialState: false)

    private var isRunning: Bool {
        get { runningLock.withLock { $0 } }
        set { runningLock.withLock { $0 = newValue } }
    }

    // MARK: - QUIC Agent State

    /// Rust QUIC agent (opaque pointer)
    private var agent: OpaquePointer?

    /// UDP connection for QUIC transport
    private var udpConnection: NWConnection?

    /// Dispatch queue for network operations
    private let networkQueue = DispatchQueue(label: "com.hankyeomans.ztna-agent.network")

    /// Timer for QUIC timeout handling
    private var timeoutTimer: DispatchSourceTimer?

    /// Server configuration (hardcoded for MVP)
    private let serverHost = "127.0.0.1"
    private let serverPort: UInt16 = 4433

    /// Buffer for receiving UDP packets
    private var recvBuffer = [UInt8](repeating: 0, count: 1500)

    /// Buffer for sending UDP packets
    private var sendBuffer = [UInt8](repeating: 0, count: 1500)

    // MARK: - Tunnel Lifecycle

    override func startTunnel(options: [String: NSObject]? = nil) async throws {
        logger.info("Starting tunnel...")

        // Apply tunnel network settings
        let settings = buildTunnelSettings()
        try await setTunnelNetworkSettings(settings)
        logger.info("Tunnel settings applied successfully")

        // Create QUIC agent
        guard let newAgent = agent_create() else {
            logger.error("Failed to create QUIC agent")
            throw NSError(domain: "ZtnaAgent", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to create QUIC agent"])
        }
        agent = newAgent
        logger.info("QUIC agent created")

        // Create UDP connection to server
        setupUdpConnection()

        isRunning = true
        startPacketLoop()
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.info("Stopping tunnel (reason: \(reason.rawValue))")
        isRunning = false

        // Stop timeout timer
        timeoutTimer?.cancel()
        timeoutTimer = nil

        // Cancel UDP connection
        udpConnection?.cancel()
        udpConnection = nil

        // Destroy QUIC agent
        if let agent = agent {
            agent_destroy(agent)
            self.agent = nil
            logger.info("QUIC agent destroyed")
        }
    }

    override func handleAppMessage(_ messageData: Data) async -> Data? {
        nil
    }

    // MARK: - Tunnel Configuration

    private func buildTunnelSettings() -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "192.0.2.1")

        let ipv4 = NEIPv4Settings(addresses: ["100.64.0.1"], subnetMasks: ["255.255.255.255"])
        ipv4.includedRoutes = [
            NEIPv4Route(destinationAddress: "1.1.1.1", subnetMask: "255.255.255.255")
        ]
        settings.ipv4Settings = ipv4
        settings.dnsSettings = NEDNSSettings(servers: ["8.8.8.8"])
        settings.mtu = NSNumber(value: 1280)

        return settings
    }

    // MARK: - UDP Connection Setup

    private func setupUdpConnection() {
        let host = NWEndpoint.Host(serverHost)
        let port = NWEndpoint.Port(rawValue: serverPort)!

        let params = NWParameters.udp
        params.allowLocalEndpointReuse = true

        let connection = NWConnection(host: host, port: port, using: params)

        connection.stateUpdateHandler = { [weak self] state in
            guard let self else { return }

            switch state {
            case .ready:
                self.logger.info("UDP connection ready to \(self.serverHost):\(self.serverPort)")
                self.initiateQuicConnection()
                self.startReceiveLoop()
                self.scheduleTimeout()

            case .failed(let error):
                self.logger.error("UDP connection failed: \(error.localizedDescription)")

            case .cancelled:
                self.logger.info("UDP connection cancelled")

            default:
                break
            }
        }

        connection.start(queue: networkQueue)
        udpConnection = connection
    }

    // MARK: - QUIC Connection

    private func initiateQuicConnection() {
        guard let agent else { return }

        let host = serverHost
        let port = serverPort

        let result = host.withCString { hostPtr in
            agent_connect(agent, hostPtr, port)
        }

        if result == AgentResultOk {
            logger.info("QUIC connection initiated to \(host):\(port)")
            // Pump outbound packets to start handshake
            pumpOutbound()
        } else {
            logger.error("Failed to initiate QUIC connection: \(result.rawValue)")
        }
    }

    // MARK: - Send Loop (Agent → Network)

    private func pumpOutbound() {
        guard let agent, let connection = udpConnection, isRunning else { return }

        var len = sendBuffer.count
        var port: UInt16 = 0

        while true {
            len = sendBuffer.count
            let result = agent_poll(agent, &sendBuffer, &len, &port)

            if result == AgentResultOk {
                let data = Data(sendBuffer.prefix(len))
                connection.send(content: data, completion: .contentProcessed { [weak self] error in
                    if let error {
                        self?.logger.warning("UDP send error: \(error.localizedDescription)")
                    }
                })
            } else if result == AgentResultNoData {
                // No more packets to send
                break
            } else {
                logger.warning("agent_poll error: \(result.rawValue)")
                break
            }
        }

        // Update agent state and reschedule timeout
        updateAgentState()
        scheduleTimeout()
    }

    // MARK: - Receive Loop (Network → Agent)

    private func startReceiveLoop() {
        guard isRunning else { return }

        udpConnection?.receiveMessage { [weak self] data, _, _, error in
            guard let self, self.isRunning else { return }

            if let error {
                self.logger.warning("UDP receive error: \(error.localizedDescription)")
            }

            if let data, !data.isEmpty {
                self.handleReceivedPacket(data)
            }

            // Continue receiving
            self.startReceiveLoop()
        }
    }

    private func handleReceivedPacket(_ data: Data) {
        guard let agent else { return }

        // Parse server address into IP bytes
        // For MVP, hardcode server IP (127.0.0.1)
        var ipBytes: [UInt8] = [127, 0, 0, 1]

        let result = data.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_recv(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                data.count,
                &ipBytes,
                serverPort
            )
        }

        if result == AgentResultOk {
            // Process any outbound packets generated by QUIC
            pumpOutbound()
        } else {
            logger.warning("agent_recv error: \(result.rawValue)")
        }
    }

    // MARK: - Timeout Handling

    private func scheduleTimeout() {
        guard let agent, isRunning else { return }

        timeoutTimer?.cancel()

        let timeoutMs = agent_timeout_ms(agent)
        guard timeoutMs > 0 else { return }

        let timer = DispatchSource.makeTimerSource(queue: networkQueue)
        timer.schedule(deadline: .now() + .milliseconds(Int(timeoutMs)))
        timer.setEventHandler { [weak self] in
            self?.handleTimeout()
        }
        timer.resume()
        timeoutTimer = timer
    }

    private func handleTimeout() {
        guard let agent, isRunning else { return }

        agent_on_timeout(agent)
        pumpOutbound()
    }

    // MARK: - Agent State Monitoring

    private func updateAgentState() {
        guard let agent else { return }

        let state = agent_get_state(agent)

        switch state {
        case AgentStateConnected:
            logger.info("QUIC connection established")
            checkObservedAddress()

        case AgentStateDisconnected:
            logger.info("QUIC connection disconnected")

        case AgentStateDraining:
            logger.info("QUIC connection draining")

        case AgentStateClosed:
            logger.info("QUIC connection closed")

        case AgentStateError:
            logger.error("QUIC agent error")

        default:
            break
        }
    }

    private func checkObservedAddress() {
        guard let agent else { return }

        var ipBytes: [UInt8] = [0, 0, 0, 0]
        var port: UInt16 = 0

        let result = agent_get_observed_address(agent, &ipBytes, &port)
        if result == AgentResultOk {
            let ip = "\(ipBytes[0]).\(ipBytes[1]).\(ipBytes[2]).\(ipBytes[3])"
            logger.info("QAD observed address: \(ip):\(port)")
        }
    }

    // MARK: - Packet Processing

    private func startPacketLoop() {
        readPackets()
    }

    private func readPackets() {
        guard isRunning else { return }

        packetFlow.readPackets { [weak self] packets, protocols in
            guard let self, self.isRunning else { return }

            for (index, packetData) in packets.enumerated() {
                let protocolFamily = protocols[index].int32Value
                if protocolFamily == AF_INET || protocolFamily == AF_INET6 {
                    self.processPacket(packetData, isIPv6: protocolFamily == AF_INET6)
                }
            }

            self.readPackets()
        }
    }

    private func processPacket(_ data: Data, isIPv6: Bool) {
        // Skip IPv6 for now (Rust FFI doesn't support it yet)
        guard !isIPv6 else {
            logger.debug("Skipping IPv6 packet (\(data.count) bytes)")
            return
        }

        guard let agent else {
            logger.debug("No agent, dropping packet")
            return
        }

        // Check if agent is connected before tunneling
        guard agent_is_connected(agent) else {
            logger.debug("Agent not connected, dropping packet (\(data.count) bytes)")
            return
        }

        // Send packet through QUIC tunnel as DATAGRAM
        let result = data.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_send_datagram(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                data.count
            )
        }

        if result == AgentResultOk {
            logger.debug("Tunneled packet (\(data.count) bytes)")
            // Pump outbound to send the DATAGRAM
            networkQueue.async { [weak self] in
                self?.pumpOutbound()
            }
        } else {
            logger.warning("Failed to tunnel packet: \(result.rawValue)")
        }
    }
}
