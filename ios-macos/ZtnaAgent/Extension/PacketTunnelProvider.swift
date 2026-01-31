import NetworkExtension
import Network
import os
import Darwin

private struct ServiceConfig {
    let id: String
    let virtualIp: String
}

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

    /// Timer for sending keepalive PINGs to prevent 30s idle timeout
    private var keepaliveTimer: DispatchSourceTimer?

    /// Keepalive interval in seconds (should be less than half of 30s idle timeout)
    private let keepaliveIntervalSeconds: Int = 10

    /// Server configuration (loaded from providerConfiguration at tunnel start)
    private var serverHost: String = "3.128.36.92"
    private var serverPort: UInt16 = 4433
    private var targetServiceId: String = "echo-service"

    /// IPv4 bytes derived from serverHost (single source of truth)
    private var serverIPBytes: [UInt8] = [3, 128, 36, 92]

    /// Service definitions for IP→service routing
    private var services: [ServiceConfig] = []

    /// Route table: destination IPv4 (as UInt32 in network byte order) → service ID
    private var routeTable: [UInt32: String] = [:]

    /// Track if we've already registered to avoid duplicate registrations
    private var hasRegistered = false

    /// Buffer for receiving UDP packets
    private var recvBuffer = [UInt8](repeating: 0, count: 1500)

    /// Buffer for sending UDP packets
    private var sendBuffer = [UInt8](repeating: 0, count: 1500)

    // MARK: - Reconnection State

    /// Network path monitor for detecting WiFi → Cellular transitions
    private var pathMonitor: NWPathMonitor?

    /// Reconnection timer (exponential backoff)
    private var reconnectTimer: DispatchSourceTimer?

    /// Current backoff delay in seconds (doubles each attempt)
    private var reconnectBackoff: TimeInterval = 1.0

    /// Maximum backoff delay
    private let maxReconnectBackoff: TimeInterval = 30.0

    /// Whether a reconnection attempt is in progress
    private var isReconnecting = false

    /// Track the previous QUIC state to detect transitions
    private var previousAgentState: AgentState = AgentStateDisconnected

    // MARK: - Tunnel Lifecycle

    override func startTunnel(options: [String: NSObject]? = nil) async throws {
        logger.info("Starting tunnel...")

        loadConfiguration()

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
        startPathMonitor()
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.info("Stopping tunnel (reason: \(reason.rawValue))")
        isRunning = false
        hasRegistered = false

        // Stop timers
        timeoutTimer?.cancel()
        timeoutTimer = nil
        keepaliveTimer?.cancel()
        keepaliveTimer = nil

        // Stop reconnect timer
        reconnectTimer?.cancel()
        reconnectTimer = nil

        // Stop path monitor
        pathMonitor?.cancel()
        pathMonitor = nil

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

    // MARK: - Configuration Loading

    private func loadConfiguration() {
        guard let tunnelProtocol = protocolConfiguration as? NETunnelProviderProtocol,
              let config = tunnelProtocol.providerConfiguration else {
            logger.warning("No provider configuration found, using defaults")
            serverIPBytes = parseIPv4(serverHost)
            return
        }

        if let host = config["serverHost"] as? String, !host.isEmpty {
            serverHost = host
        }
        if let port = config["serverPort"] as? Int, port > 0 {
            serverPort = UInt16(port)
        }
        if let service = config["serviceId"] as? String, !service.isEmpty {
            targetServiceId = service
        }

        // Parse services array for IP→service routing
        if let servicesArray = config["services"] as? [[String: Any]] {
            for entry in servicesArray {
                guard let id = entry["id"] as? String,
                      let virtualIp = entry["virtualIp"] as? String else {
                    continue
                }
                let svc = ServiceConfig(id: id, virtualIp: virtualIp)
                services.append(svc)

                // Build route table: IP → service_id
                if let ipKey = ipv4ToUInt32(virtualIp) {
                    routeTable[ipKey] = id
                    logger.info("Route: \(virtualIp) -> '\(id)'")
                }
            }
            logger.info("Loaded \(self.services.count) service routes")
        }

        serverIPBytes = parseIPv4(serverHost)
        logger.info("Configuration loaded: \(self.serverHost):\(self.serverPort), service=\(self.targetServiceId), routes=\(self.routeTable.count)")
    }

    private func parseIPv4(_ host: String) -> [UInt8] {
        let components = host.split(separator: ".").compactMap { UInt8($0) }
        guard components.count == 4 else {
            logger.error("Invalid IPv4 address format: \(host)")
            return [0, 0, 0, 0]
        }
        return components
    }

    private func ipv4ToUInt32(_ host: String) -> UInt32? {
        let bytes = parseIPv4(host)
        guard bytes != [0, 0, 0, 0] || host == "0.0.0.0" else { return nil }
        return UInt32(bytes[0]) << 24 | UInt32(bytes[1]) << 16 | UInt32(bytes[2]) << 8 | UInt32(bytes[3])
    }

    private func extractDestIPv4(_ packet: Data) -> UInt32? {
        guard packet.count >= 20 else { return nil }
        // Destination IP is at bytes 16-19 in IPv4 header
        return UInt32(packet[16]) << 24 | UInt32(packet[17]) << 16 | UInt32(packet[18]) << 8 | UInt32(packet[19])
    }

    // MARK: - Tunnel Configuration

    private func buildTunnelSettings() -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "192.0.2.1")

        let ipv4 = NEIPv4Settings(addresses: ["100.64.0.1"], subnetMasks: ["255.255.255.255"])
        // Route ZTNA service IPs through tunnel (10.100.0.0/24 = virtual service range)
        // 10.100.0.1 = echo-service (UDP 9999)
        ipv4.includedRoutes = [
            NEIPv4Route(destinationAddress: "10.100.0.0", subnetMask: "255.255.255.0")
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

        // Force IPv4 to avoid IPv6 preference on dual-stack networks
        if let ipOptions = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
            ipOptions.version = .v4
        }

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
                self.scheduleReconnect(reason: "UDP connection failed")

            case .waiting(let error):
                self.logger.info("UDP connection waiting: \(error.localizedDescription)")

            case .cancelled:
                self.logger.info("UDP connection cancelled")

            default:
                break
            }
        }

        connection.start(queue: networkQueue)
        udpConnection = connection
    }

    // MARK: - Connection Resilience

    /// Start monitoring network path changes (WiFi → Cellular, etc.)
    private func startPathMonitor() {
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self, self.isRunning else { return }

            if path.status == .satisfied {
                guard let agent = self.agent else { return }
                let state = agent_get_state(agent)
                if state == AgentStateClosed || state == AgentStateError
                   || state == AgentStateDisconnected {
                    self.logger.info("Network path changed (satisfied), scheduling reconnect")
                    self.scheduleReconnect(reason: "network path change")
                }
            } else {
                self.logger.info("Network path unsatisfied — waiting for connectivity")
            }
        }
        monitor.start(queue: networkQueue)
        pathMonitor = monitor
    }

    /// Schedule a reconnection attempt with exponential backoff.
    /// Safe to call multiple times — coalesces into a single timer.
    private func scheduleReconnect(reason: String) {
        guard isRunning, !isReconnecting else { return }

        reconnectTimer?.cancel()
        reconnectTimer = nil

        let delay = reconnectBackoff
        logger.info("Scheduling reconnect in \(delay)s (reason: \(reason))")

        let timer = DispatchSource.makeTimerSource(queue: networkQueue)
        timer.schedule(deadline: .now() + delay)
        timer.setEventHandler { [weak self] in
            self?.attemptReconnect()
        }
        timer.resume()
        reconnectTimer = timer

        // Exponential backoff: 1s → 2s → 4s → 8s → 16s → 30s (cap)
        reconnectBackoff = min(reconnectBackoff * 2, maxReconnectBackoff)
    }

    /// Tear down old connection and establish a new one.
    /// Reuses the existing Agent — just calls agent_connect() again.
    private func attemptReconnect() {
        guard agent != nil, isRunning else { return }
        isReconnecting = true

        logger.info("Attempting reconnect to \(self.serverHost):\(self.serverPort)")

        keepaliveTimer?.cancel()
        keepaliveTimer = nil

        udpConnection?.cancel()
        udpConnection = nil

        hasRegistered = false

        setupUdpConnection()

        isReconnecting = false
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

        var ipBytes = serverIPBytes

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
            // Drain any received IP packets from tunnel and inject into TUN
            drainIncomingDatagrams()
            // Process any outbound packets generated by QUIC
            pumpOutbound()
        } else {
            logger.warning("agent_recv error: \(result.rawValue)")
        }
    }

    /// Drain received IP packets from the QUIC tunnel and inject into TUN.
    ///
    /// Called after agent_recv() processes incoming UDP data. The Rust agent
    /// queues any received QUIC DATAGRAMs (IP packets from Connector responses).
    /// We poll until empty and write each packet to packetFlow for kernel delivery.
    private func drainIncomingDatagrams() {
        guard let agent, isRunning else { return }

        var packets: [Data] = []
        var protocols: [NSNumber] = []

        while true {
            var len = recvBuffer.count
            let result = agent_recv_datagram(agent, &recvBuffer, &len)

            if result == AgentResultNoData {
                break
            }

            if result == AgentResultOk, len > 0 {
                // Validate: must be IPv4 (version nibble == 4)
                if recvBuffer[0] >> 4 == 4 {
                    packets.append(Data(recvBuffer.prefix(len)))
                    protocols.append(NSNumber(value: AF_INET))
                }
            } else {
                break
            }
        }

        if !packets.isEmpty {
            packetFlow.writePackets(packets, withProtocols: protocols)
            logger.debug("Injected \(packets.count) return packet(s) into TUN")
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

    // MARK: - Keepalive Timer

    private func startKeepaliveTimer() {
        guard isRunning else { return }

        keepaliveTimer?.cancel()

        let timer = DispatchSource.makeTimerSource(queue: networkQueue)
        timer.schedule(
            deadline: .now() + .seconds(keepaliveIntervalSeconds),
            repeating: .seconds(keepaliveIntervalSeconds)
        )
        timer.setEventHandler { [weak self] in
            self?.sendKeepalive()
        }
        timer.resume()
        keepaliveTimer = timer
        logger.info("Keepalive timer started (interval: \(self.keepaliveIntervalSeconds)s)")
    }

    private func sendKeepalive() {
        guard let agent, isRunning else { return }

        let result = agent_send_intermediate_keepalive(agent)

        if result == AgentResultOk {
            logger.debug("Keepalive PING sent")
            // Pump outbound to actually send the PING frame
            pumpOutbound()
        } else if result == AgentResultNotConnected {
            logger.warning("Keepalive failed: not connected")
            keepaliveTimer?.cancel()
            keepaliveTimer = nil
            scheduleReconnect(reason: "keepalive detected disconnection")
        } else {
            logger.warning("Keepalive failed: \(result.rawValue)")
        }
    }

    // MARK: - Agent State Monitoring

    private func updateAgentState() {
        guard let agent else { return }

        let state = agent_get_state(agent)

        // Only act on state *transitions* (not repeated polls of same state)
        guard state != previousAgentState else { return }
        let oldState = previousAgentState
        previousAgentState = state

        switch state {
        case AgentStateConnected:
            logger.info("QUIC connection established")
            reconnectBackoff = 1.0
            checkObservedAddress()
            registerForService()

        case AgentStateDisconnected:
            logger.info("QUIC connection disconnected")
            if oldState == AgentStateConnected || oldState == AgentStateConnecting {
                scheduleReconnect(reason: "QUIC disconnected")
            }

        case AgentStateDraining:
            logger.info("QUIC connection draining")

        case AgentStateClosed:
            logger.info("QUIC connection closed")
            if oldState != AgentStateDisconnected {
                scheduleReconnect(reason: "QUIC connection closed")
            }

        case AgentStateError:
            logger.error("QUIC agent error")
            scheduleReconnect(reason: "QUIC agent error")

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

    private func registerForService() {
        let currentHasRegistered = self.hasRegistered
        let hasAgent = self.agent != nil
        logger.info("registerForService() called, hasRegistered=\(currentHasRegistered)")
        guard let agent, !hasRegistered else {
            logger.info("registerForService() guard failed: agent=\(hasAgent), hasRegistered=\(currentHasRegistered)")
            return
        }

        // Collect service IDs to register: use services array if configured, else single targetServiceId
        var serviceIds: [String] = services.map(\.id)
        if serviceIds.isEmpty {
            serviceIds = [targetServiceId]
        }

        var anySuccess = false
        for serviceId in serviceIds {
            logger.info("Calling agent_register for '\(serviceId)'")
            let result = serviceId.withCString { servicePtr in
                agent_register(agent, servicePtr)
            }
            logger.info("agent_register returned: \(result.rawValue)")

            if result == AgentResultOk {
                anySuccess = true
                logger.info("Registered for service '\(serviceId)'")
            } else {
                logger.warning("Failed to register for service '\(serviceId)': result=\(result.rawValue)")
            }
        }

        if anySuccess {
            hasRegistered = true
            // Pump outbound to send registration DATAGRAMs
            networkQueue.async { [weak self] in
                self?.pumpOutbound()
            }
            // Start keepalive timer to prevent 30s idle timeout
            startKeepaliveTimer()
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

        // If route table is populated, wrap packet with 0x2F service header
        if !routeTable.isEmpty, let destIp = extractDestIPv4(data), let serviceId = routeTable[destIp] {
            sendRoutedDatagram(agent: agent, serviceId: serviceId, packet: data)
        } else {
            // Legacy path: send raw IP packet (implicit single-service routing)
            sendRawDatagram(agent: agent, packet: data)
        }
    }

    private func sendRoutedDatagram(agent: OpaquePointer, serviceId: String, packet: Data) {
        // Build wrapped datagram: [0x2F, id_len, service_id_bytes..., ip_packet...]
        let idBytes = Array(serviceId.utf8)
        var wrapped = Data(capacity: 2 + idBytes.count + packet.count)
        wrapped.append(0x2F)
        wrapped.append(UInt8(idBytes.count))
        wrapped.append(contentsOf: idBytes)
        wrapped.append(packet)

        let result = wrapped.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_send_datagram(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                wrapped.count
            )
        }

        if result == AgentResultOk {
            logger.debug("Tunneled routed packet (\(packet.count) bytes) -> '\(serviceId)'")
            networkQueue.async { [weak self] in
                self?.pumpOutbound()
            }
        } else {
            logger.warning("Failed to tunnel routed packet: \(result.rawValue)")
        }
    }

    private func sendRawDatagram(agent: OpaquePointer, packet: Data) {
        let result = packet.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_send_datagram(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                packet.count
            )
        }

        if result == AgentResultOk {
            logger.debug("Tunneled packet (\(packet.count) bytes)")
            networkQueue.async { [weak self] in
                self?.pumpOutbound()
            }
        } else {
            logger.warning("Failed to tunnel packet: \(result.rawValue)")
        }
    }
}
