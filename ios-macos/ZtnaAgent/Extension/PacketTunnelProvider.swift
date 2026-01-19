import NetworkExtension
import os
import Darwin

final class PacketTunnelProvider: NEPacketTunnelProvider {

    private let logger = Logger(subsystem: "com.hankyeomans.ztna-agent", category: "Tunnel")

    /// Thread-safe running state using OSAllocatedUnfairLock
    private let runningLock = OSAllocatedUnfairLock(initialState: false)

    private var isRunning: Bool {
        get { runningLock.withLock { $0 } }
        set { runningLock.withLock { $0 = newValue } }
    }

    // MARK: - Tunnel Lifecycle

    override func startTunnel(options: [String: NSObject]? = nil) async throws {
        logger.info("Starting tunnel...")

        let settings = buildTunnelSettings()
        try await setTunnelNetworkSettings(settings)

        logger.info("Tunnel settings applied successfully")

        isRunning = true
        startPacketLoop()
    }

    override func stopTunnel(with reason: NEProviderStopReason) async {
        logger.info("Stopping tunnel (reason: \(reason.rawValue))")
        isRunning = false
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

        return settings
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
        let action = data.withUnsafeBytes { buffer -> PacketAction in
            guard let baseAddress = buffer.baseAddress else { return PacketActionForward }
            return process_packet(baseAddress.assumingMemoryBound(to: UInt8.self), data.count)
        }

        if action == PacketActionDrop {
            logger.debug("Dropping packet (\(data.count) bytes, IPv\(isIPv6 ? "6" : "4")) - Rust decision")
        } else {
            logger.debug("Forwarding packet (\(data.count) bytes, IPv\(isIPv6 ? "6" : "4"))")
        }
    }
}
