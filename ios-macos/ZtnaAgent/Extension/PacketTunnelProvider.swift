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

    // MARK: - P2P State

    /// Per-candidate NWConnections for sending binding requests during hole punch
    private var bindingConnections: [String: NWConnection] = [:]

    /// Direct P2P NWConnection to Connector (after successful hole punch)
    private var p2pConnection: NWConnection?

    /// Hole punch working address
    private var p2pHost: String?
    private var p2pPort: UInt16 = 0
    private var p2pIPBytes: [UInt8] = [0, 0, 0, 0]

    /// Hole punch poll timer (50ms interval during hole punching)
    private var holePunchTimer: DispatchSourceTimer?

    /// P2P keepalive timer (15s interval after P2P established)
    private var p2pKeepaliveTimer: DispatchSourceTimer?

    /// Whether hole punching has been initiated for this connection
    private var holePunchStarted = false

    /// Whether a P2P direct path is actively being used
    private var isP2PActive = false

    /// Buffer for P2P outbound packets
    private var p2pSendBuffer = [UInt8](repeating: 0, count: 1500)

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

        // Clean up P2P resources
        holePunchTimer?.cancel()
        holePunchTimer = nil
        p2pKeepaliveTimer?.cancel()
        p2pKeepaliveTimer = nil
        p2pConnection?.cancel()
        p2pConnection = nil
        for (_, conn) in bindingConnections {
            conn.cancel()
        }
        bindingConnections.removeAll()
        holePunchStarted = false
        isP2PActive = false

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
                // Report local endpoint to quiche for RecvInfo.to path validation
                if let agent = self.agent {
                    self.reportLocalAddress(connection: connection, agent: agent)
                }
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

        // Reset P2P state — hole punch restarts automatically via registerForService()
        holePunchTimer?.cancel()
        holePunchTimer = nil
        p2pKeepaliveTimer?.cancel()
        p2pKeepaliveTimer = nil
        p2pConnection?.cancel()
        p2pConnection = nil
        for (_, conn) in bindingConnections {
            conn.cancel()
        }
        bindingConnections.removeAll()
        holePunchStarted = false
        isP2PActive = false
        p2pHost = nil
        p2pPort = 0

        udpConnection?.cancel()
        udpConnection = nil

        hasRegistered = false

        setupUdpConnection()

        isReconnecting = false
    }

    /// Extract the local UDP endpoint from an NWConnection and pass it to the Rust agent
    /// so quiche uses it for RecvInfo.to path validation (instead of 0.0.0.0:0).
    private func reportLocalAddress(connection: NWConnection, agent: OpaquePointer) {
        if case .hostPort(let host, let port) = connection.currentPath?.localEndpoint {
            let portValue = port.rawValue
            let hostStr = "\(host)"
            let ipBytes = parseIPv4(hostStr)
            if ipBytes != [0, 0, 0, 0] {
                var ip = ipBytes
                let result = agent_set_local_addr(agent, &ip, portValue)
                logger.info("Set local address: \(hostStr):\(portValue) → \(result.rawValue)")
            } else {
                logger.warning("Could not parse local endpoint: \(hostStr)")
            }
        } else {
            logger.warning("No local endpoint available yet")
        }
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
        } else {
            logger.warning("agent_recv error: \(result.rawValue)")
        }
        // Always pump outbound — even on recv error, quiche may have
        // close/drain packets to send, and during handshake it needs
        // to send the next flight.
        pumpOutbound()
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
            // Initiate P2P hole punching after a short delay to allow
            // QAD observed address DATAGRAM to arrive and be processed
            networkQueue.asyncAfter(deadline: .now() + .milliseconds(500)) { [weak self] in
                self?.startHolePunching()
            }
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

        // Look up destination service from route table
        let destIp = extractDestIPv4(data)
        let serviceId = destIp.flatMap { routeTable[$0] }

        // Use P2P direct path only for the P2P-connected service
        if isP2PActive, agent_get_active_path(agent) == 0,
           serviceId == nil || serviceId == targetServiceId {
            sendP2PDatagram(agent: agent, packet: data, serviceId: serviceId)
            return
        }

        // Route via relay with service header
        if let serviceId {
            sendRoutedDatagram(agent: agent, serviceId: serviceId, packet: data)
        } else {
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

    /// Send an IP packet via P2P direct path.
    /// Falls back to relay on failure.
    private func sendP2PDatagram(agent: OpaquePointer, packet: Data, serviceId: String? = nil) {
        var destIp = p2pIPBytes
        let destPort = p2pPort

        let result = packet.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_send_datagram_p2p(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                packet.count,
                &destIp,
                destPort
            )
        }

        if result == AgentResultOk {
            logger.debug("P2P tunneled packet (\(packet.count) bytes)")
            networkQueue.async { [weak self] in
                self?.pumpP2POutbound()
            }
        } else {
            // Fall back to relay with proper service routing
            logger.warning("P2P send failed (\(result.rawValue)), falling back to relay")
            if let serviceId {
                sendRoutedDatagram(agent: agent, serviceId: serviceId, packet: packet)
            } else {
                sendRawDatagram(agent: agent, packet: packet)
            }
        }
    }

    // MARK: - P2P Hole Punching

    /// Initiate hole punching after service registration.
    /// Sends CandidateOffer via signaling stream through the Intermediate.
    private func startHolePunching() {
        guard let agent, !holePunchStarted, isRunning else { return }

        let serviceId = targetServiceId
        let result = serviceId.withCString { servicePtr in
            agent_start_hole_punch(agent, servicePtr)
        }

        if result == AgentResultOk {
            holePunchStarted = true
            logger.info("Hole punch initiated for service '\(serviceId)'")

            // Pump outbound to send CandidateOffer through Intermediate
            networkQueue.async { [weak self] in
                self?.pumpOutbound()
            }

            // Start polling for hole punch progress (50ms interval)
            startHolePunchPollTimer()
        } else {
            logger.warning("Failed to start hole punch: \(result.rawValue)")
        }
    }

    private func startHolePunchPollTimer() {
        holePunchTimer?.cancel()

        let timer = DispatchSource.makeTimerSource(queue: networkQueue)
        timer.schedule(deadline: .now() + .milliseconds(50), repeating: .milliseconds(50))
        timer.setEventHandler { [weak self] in
            self?.pollHolePunch()
        }
        timer.resume()
        holePunchTimer = timer
    }

    /// Called every 50ms during hole punching.
    /// Sends binding requests, processes responses, and checks for completion.
    private func pollHolePunch() {
        guard let agent, isRunning, holePunchStarted else { return }

        // 1. Send any pending binding requests to candidate addresses
        sendPendingBindingRequests()

        // 2. Check hole punch completion
        var ipBytes: [UInt8] = [0, 0, 0, 0]
        var port: UInt16 = 0
        var complete: UInt8 = 0

        let result = agent_poll_hole_punch(agent, &ipBytes, &port, &complete)

        if complete == 1 {
            // Hole punching finished
            holePunchTimer?.cancel()
            holePunchTimer = nil

            if result == AgentResultOk {
                let ip = "\(ipBytes[0]).\(ipBytes[1]).\(ipBytes[2]).\(ipBytes[3])"
                logger.info("Hole punch SUCCESS: direct path to \(ip, privacy: .public):\(port, privacy: .public)")
                setupP2PConnection(host: ip, port: port, ipBytes: ipBytes)
            } else {
                logger.info("Hole punch FAILED: continuing with relay path")
                cleanupBindingConnections()
            }
        }
    }

    // MARK: - Binding Request Pump

    /// Poll for binding requests from Rust and send them via per-candidate NWConnections.
    private func sendPendingBindingRequests() {
        guard let agent else { return }

        var bindingBuffer = [UInt8](repeating: 0, count: 1500)

        while true {
            var len = bindingBuffer.count
            var ipBytes: [UInt8] = [0, 0, 0, 0]
            var port: UInt16 = 0

            let result = agent_poll_binding_request(agent, &bindingBuffer, &len, &ipBytes, &port)

            guard result == AgentResultOk, len > 0 else { break }

            let host = "\(ipBytes[0]).\(ipBytes[1]).\(ipBytes[2]).\(ipBytes[3])"
            let key = "\(host):\(port)"
            let data = Data(bindingBuffer.prefix(len))

            logger.info("Binding request: \(len, privacy: .public) bytes -> \(key, privacy: .public)")

            let connection = getOrCreateBindingConnection(host: host, port: port, key: key)

            if connection.state == .ready {
                logger.info("Sending binding request to \(key, privacy: .public) (\(len, privacy: .public) bytes)")
                connection.send(content: data, completion: .contentProcessed { [weak self] error in
                    if let error {
                        self?.logger.warning("Binding send error to \(key, privacy: .public): \(error.localizedDescription, privacy: .public)")
                    }
                })
            } else {
                logger.info("Binding connection to \(key, privacy: .public) not ready (state: \(String(describing: connection.state), privacy: .public))")
            }
        }
    }

    /// Get or create a per-candidate NWConnection for binding request delivery.
    private func getOrCreateBindingConnection(host: String, port: UInt16, key: String) -> NWConnection {
        if let existing = bindingConnections[key] {
            return existing
        }

        let endpoint = NWEndpoint.Host(host)
        let nwPort = NWEndpoint.Port(rawValue: port)!

        let params = NWParameters.udp
        params.allowLocalEndpointReuse = true
        if let ipOptions = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
            ipOptions.version = .v4
        }

        let connection = NWConnection(host: endpoint, port: nwPort, using: params)

        connection.stateUpdateHandler = { [weak self] state in
            guard let self else { return }
            switch state {
            case .ready:
                self.logger.info("Binding connection ready to \(key, privacy: .public)")
                self.startBindingReceiveLoop(connection: connection, key: key)
            case .failed(let error):
                self.logger.warning("Binding connection failed to \(key, privacy: .public): \(error.localizedDescription, privacy: .public)")
                self.bindingConnections.removeValue(forKey: key)
            default:
                break
            }
        }

        connection.start(queue: networkQueue)
        bindingConnections[key] = connection
        return connection
    }

    /// Receive binding responses on a per-candidate connection and feed back to Rust.
    private func startBindingReceiveLoop(connection: NWConnection, key: String) {
        connection.receiveMessage { [weak self] data, _, _, error in
            guard let self, self.isRunning, self.holePunchStarted else { return }

            if let data, !data.isEmpty, let agent = self.agent {
                // Parse key to get source IP/port
                let parts = key.split(separator: ":")
                if parts.count == 2 {
                    var fromIp = self.parseIPv4(String(parts[0]))
                    let fromPort = UInt16(parts[1]) ?? 0

                    data.withUnsafeBytes { buffer in
                        guard let baseAddress = buffer.baseAddress else { return }
                        let _ = agent_process_binding_response(
                            agent,
                            baseAddress.assumingMemoryBound(to: UInt8.self),
                            data.count,
                            &fromIp,
                            fromPort
                        )
                    }
                    self.logger.info("Processed binding response from \(key, privacy: .public) (\(data.count, privacy: .public) bytes)")
                }
            }

            if let error {
                self.logger.warning("Binding receive error from \(key, privacy: .public): \(error.localizedDescription, privacy: .public)")
            }

            // Only continue receiving if the connection is still viable
            guard connection.state == .ready else { return }
            self.startBindingReceiveLoop(connection: connection, key: key)
        }
    }

    private func cleanupBindingConnections() {
        for (_, conn) in bindingConnections {
            conn.cancel()
        }
        bindingConnections.removeAll()
    }

    // MARK: - P2P QUIC Connection

    /// Set up a direct P2P NWConnection after hole punch succeeds.
    private func setupP2PConnection(host: String, port: UInt16, ipBytes: [UInt8]) {
        cleanupBindingConnections()

        p2pHost = host
        p2pPort = port
        p2pIPBytes = ipBytes

        let endpoint = NWEndpoint.Host(host)
        let nwPort = NWEndpoint.Port(rawValue: port)!

        let params = NWParameters.udp
        params.allowLocalEndpointReuse = true
        if let ipOptions = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
            ipOptions.version = .v4
        }

        let connection = NWConnection(host: endpoint, port: nwPort, using: params)

        connection.stateUpdateHandler = { [weak self] state in
            guard let self else { return }
            switch state {
            case .ready:
                self.logger.info("P2P UDP connection ready to \(host):\(port)")
                self.initiateP2PQuicConnection()
                self.startP2PReceiveLoop()
            case .failed(let error):
                self.logger.error("P2P connection failed: \(error.localizedDescription)")
                self.isP2PActive = false
            default:
                break
            }
        }

        connection.start(queue: networkQueue)
        p2pConnection = connection
    }

    /// Initiate P2P QUIC handshake over the direct connection.
    private func initiateP2PQuicConnection() {
        guard let agent, let host = p2pHost else { return }
        let port = p2pPort

        let result = host.withCString { hostPtr in
            agent_connect_p2p(agent, hostPtr, port)
        }

        if result == AgentResultOk {
            logger.info("P2P QUIC connection initiated to \(host):\(port)")
            pumpP2POutbound()
        } else {
            logger.warning("Failed to initiate P2P QUIC: \(result.rawValue)")
        }
    }

    /// Receive loop for the P2P direct connection.
    private func startP2PReceiveLoop() {
        guard isRunning else { return }

        p2pConnection?.receiveMessage { [weak self] data, _, _, error in
            guard let self, self.isRunning else { return }

            if let data, !data.isEmpty {
                self.handleP2PReceivedPacket(data)
            }

            if let error {
                self.logger.warning("P2P receive error: \(error.localizedDescription)")
            }

            self.startP2PReceiveLoop()
        }
    }

    /// Feed received P2P UDP data to the Rust agent.
    private func handleP2PReceivedPacket(_ data: Data) {
        guard let agent else { return }

        var ipBytes = p2pIPBytes

        let result = data.withUnsafeBytes { buffer -> AgentResult in
            guard let baseAddress = buffer.baseAddress else { return AgentResultInvalidPointer }
            return agent_recv(
                agent,
                baseAddress.assumingMemoryBound(to: UInt8.self),
                data.count,
                &ipBytes,
                p2pPort
            )
        }

        if result == AgentResultOk {
            drainIncomingDatagrams()
            pumpP2POutbound()

            // Check if P2P QUIC handshake has completed
            if !isP2PActive, let host = p2pHost {
                let connected = host.withCString { hostPtr in
                    agent_is_p2p_connected(agent, hostPtr, p2pPort)
                }
                if connected {
                    isP2PActive = true
                    logger.info("P2P QUIC connection ESTABLISHED - switching to direct path")
                    startP2PKeepaliveTimer()
                }
            }
        }
    }

    // MARK: - P2P Packet Pump

    /// Poll for outbound P2P QUIC packets and send via p2pConnection.
    private func pumpP2POutbound() {
        guard let agent, let connection = p2pConnection, isRunning else { return }

        while true {
            var len = p2pSendBuffer.count
            var ipBytes: [UInt8] = [0, 0, 0, 0]
            var port: UInt16 = 0

            let result = agent_poll_p2p(agent, &p2pSendBuffer, &len, &ipBytes, &port)

            if result == AgentResultOk {
                let data = Data(p2pSendBuffer.prefix(len))
                connection.send(content: data, completion: .contentProcessed { [weak self] error in
                    if let error {
                        self?.logger.warning("P2P send error: \(error.localizedDescription)")
                    }
                })
            } else {
                break
            }
        }
    }

    // MARK: - P2P Keepalive & Path Monitoring

    /// Start P2P keepalive timer (15s interval) after P2P QUIC is established.
    private func startP2PKeepaliveTimer() {
        p2pKeepaliveTimer?.cancel()

        let timer = DispatchSource.makeTimerSource(queue: networkQueue)
        timer.schedule(deadline: .now() + .seconds(15), repeating: .seconds(15))
        timer.setEventHandler { [weak self] in
            self?.sendP2PKeepalive()
        }
        timer.resume()
        p2pKeepaliveTimer = timer
        logger.info("P2P keepalive timer started (15s interval)")
    }

    private func sendP2PKeepalive() {
        guard let agent, let connection = p2pConnection, isRunning, isP2PActive else { return }

        var ipBytes: [UInt8] = [0, 0, 0, 0]
        var port: UInt16 = 0
        var keepaliveData = [UInt8](repeating: 0, count: 5)

        let result = agent_poll_keepalive(agent, &ipBytes, &port, &keepaliveData)

        if result == AgentResultOk {
            let data = Data(keepaliveData)
            connection.send(content: data, completion: .contentProcessed { [weak self] error in
                if let error {
                    self?.logger.warning("P2P keepalive send error: \(error.localizedDescription)")
                }
            })
        }

        logPathState()

        // Check if we've fallen back to relay
        if agent_is_in_fallback(agent) {
            logger.warning("P2P path failed — fallen back to relay")
            isP2PActive = false
            p2pKeepaliveTimer?.cancel()
            p2pKeepaliveTimer = nil
        }
    }

    private func logPathState() {
        guard let agent else { return }

        let activePath = agent_get_active_path(agent)
        var missedKeepalives: UInt32 = 0
        var rttMs: UInt64 = 0
        var inFallback: UInt8 = 0

        let _ = agent_get_path_stats(agent, &missedKeepalives, &rttMs, &inFallback)

        let pathName: String
        switch activePath {
        case 0: pathName = "DIRECT"
        case 1: pathName = "RELAY"
        default: pathName = "NONE"
        }

        logger.info("Path: \(pathName), RTT: \(rttMs)ms, missed keepalives: \(missedKeepalives), fallback: \(inFallback == 1 ? "YES" : "NO")")
    }
}
