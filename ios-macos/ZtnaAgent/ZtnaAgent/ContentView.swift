import SwiftUI
import NetworkExtension
import Observation

@main
struct ZtnaAgentApp: App {
    @State private var vpnManager = VPNManager()

    /// Check for --auto-start command line argument for testing automation
    private var shouldAutoStart: Bool {
        CommandLine.arguments.contains("--auto-start")
    }

    /// Check for --auto-stop <seconds> to automatically stop after duration
    /// Usage: --auto-stop 30 (stops after 30 seconds)
    private var autoStopDuration: TimeInterval? {
        guard let idx = CommandLine.arguments.firstIndex(of: "--auto-stop"),
              idx + 1 < CommandLine.arguments.count,
              let seconds = TimeInterval(CommandLine.arguments[idx + 1]) else {
            return nil
        }
        return seconds
    }

    /// Check for --exit-after-stop to quit app after VPN stops
    private var shouldExitAfterStop: Bool {
        CommandLine.arguments.contains("--exit-after-stop")
    }

    var body: some Scene {
        WindowGroup {
            ContentView(vpnManager: vpnManager)
                .task {
                    if shouldAutoStart {
                        // Small delay to ensure app is fully ready
                        try? await Task.sleep(for: .milliseconds(500))
                        await vpnManager.start()

                        // If auto-stop is configured, wait for connection then schedule stop
                        if let duration = autoStopDuration {
                            // Wait for connection to establish
                            while vpnManager.status != .connected && vpnManager.status != .startError {
                                try? await Task.sleep(for: .milliseconds(100))
                            }

                            if vpnManager.status == .connected {
                                print("[TEST] Connected. Will auto-stop in \(Int(duration)) seconds...")
                                try? await Task.sleep(for: .seconds(duration))
                                print("[TEST] Auto-stopping VPN...")
                                vpnManager.stop()

                                // Wait for disconnect
                                try? await Task.sleep(for: .milliseconds(500))

                                if shouldExitAfterStop {
                                    print("[TEST] Exiting app...")
                                    try? await Task.sleep(for: .milliseconds(200))
                                    exit(0)
                                }
                            }
                        }
                    }
                }
        }
    }
}

@Observable
@MainActor
final class VPNManager {
    private(set) var status: VPNStatus = .unknown
    private var manager: NETunnelProviderManager?
    nonisolated(unsafe) private var statusTask: Task<Void, Never>?

    // MARK: - Tunnel Configuration

    var serverHost: String {
        didSet { UserDefaults.standard.set(serverHost, forKey: "ztnaServerHost") }
    }

    var serverPort: UInt16 {
        didSet { UserDefaults.standard.set(Int(serverPort), forKey: "ztnaServerPort") }
    }

    var serviceId: String {
        didSet { UserDefaults.standard.set(serviceId, forKey: "ztnaServiceId") }
    }

    enum VPNStatus: String, Sendable {
        case unknown = "Unknown"
        case invalid = "Invalid"
        case disconnected = "Disconnected"
        case connecting = "Connecting..."
        case connected = "Connected ✓"
        case reasserting = "Reasserting..."
        case disconnecting = "Disconnecting..."
        case loadError = "Load Error"
        case startError = "Start Error"
    }

    init() {
        serverHost = UserDefaults.standard.string(forKey: "ztnaServerHost") ?? "3.128.36.92"
        let savedPort = UserDefaults.standard.integer(forKey: "ztnaServerPort")
        serverPort = savedPort > 0 ? UInt16(savedPort) : 4433
        serviceId = UserDefaults.standard.string(forKey: "ztnaServiceId") ?? "echo-service"
        startStatusObserver()
    }
    
    deinit {
        statusTask?.cancel()
    }
    
    private func startStatusObserver() {
        statusTask = Task { [weak self] in
            let notifications = NotificationCenter.default.notifications(named: .NEVPNStatusDidChange)
            for await notification in notifications {
                guard let self,
                      let connection = notification.object as? NEVPNConnection else { continue }
                self.updateStatus(from: connection.status)
            }
        }
    }
    
    private func updateStatus(from neStatus: NEVPNStatus) {
        status = switch neStatus {
        case .invalid: .invalid
        case .disconnected: .disconnected
        case .connecting: .connecting
        case .connected: .connected
        case .reasserting: .reasserting
        case .disconnecting: .disconnecting
        @unknown default: .unknown
        }
    }
    
    func start() async {
        do {
            let managers = try await NETunnelProviderManager.loadAllFromPreferences()
            let mgr = managers.first ?? NETunnelProviderManager()
            manager = mgr

            let config = NETunnelProviderProtocol()
            config.providerBundleIdentifier = "com.hankyeomans.ztna-agent.ZtnaAgent.Extension"
            config.serverAddress = serverHost
            config.providerConfiguration = [
                "serverHost": serverHost,
                "serverPort": Int(serverPort),
                "serviceId": serviceId,
                "services": [
                    ["id": serviceId, "virtualIp": "10.100.0.1"]
                ] as [[String: Any]]
            ]

            mgr.protocolConfiguration = config
            mgr.isEnabled = true
            mgr.localizedDescription = "ZTNA Agent"

            try await mgr.saveToPreferences()
            try await mgr.loadFromPreferences()

            // Retry logic for first-time configuration
            var lastError: Error?
            for attempt in 1...3 {
                do {
                    try mgr.connection.startVPNTunnel()
                    status = .connecting
                    return
                } catch {
                    lastError = error
                    print("VPN start attempt \(attempt) failed: \(error)")
                    if attempt < 3 {
                        // Wait and reload before retrying
                        try? await Task.sleep(for: .milliseconds(500))
                        try? await mgr.loadFromPreferences()
                    }
                }
            }

            if let error = lastError {
                throw error
            }
        } catch {
            print("VPN start error: \(error)")
            status = .startError
        }
    }
    
    func stop() {
        manager?.connection.stopVPNTunnel()
    }
    
    var isConnected: Bool { status == .connected }
    var isTransitioning: Bool { status == .connecting || status == .disconnecting || status == .reasserting }
}

struct ContentView: View {
    @Bindable var vpnManager: VPNManager
    @State private var portText: String = ""

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: vpnManager.isConnected ? "lock.shield.fill" : "lock.shield")
                .font(.system(size: 48))
                .foregroundStyle(vpnManager.isConnected ? .green : .blue)
                .symbolEffect(.pulse, isActive: vpnManager.isTransitioning)

            Text("ZTNA Agent")
                .font(.title.bold())

            Text(vpnManager.status.rawValue)
                .font(.headline)
                .foregroundStyle(.secondary)

            HStack(spacing: 16) {
                Button("Start") {
                    Task { await vpnManager.start() }
                }
                .disabled(vpnManager.isConnected || vpnManager.isTransitioning)

                Button("Stop") {
                    vpnManager.stop()
                }
                .disabled(!vpnManager.isConnected)
            }
            .buttonStyle(.borderedProminent)

            Divider()

            configSection
        }
        .padding(24)
        .frame(width: 420, height: 440)
        .onAppear {
            portText = String(vpnManager.serverPort)
        }
    }

    private var configSection: some View {
        GroupBox("Server Configuration") {
            VStack(alignment: .leading, spacing: 8) {
                LabeledContent("Host") {
                    TextField("Server host", text: $vpnManager.serverHost)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 200)
                }
                LabeledContent("Port") {
                    TextField("Port", text: $portText)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 80)
                        .onChange(of: portText) {
                            if let port = UInt16(portText), port > 0 {
                                vpnManager.serverPort = port
                            }
                        }
                }
                LabeledContent("Service") {
                    TextField("Service ID", text: $vpnManager.serviceId)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 200)
                }
            }
            .padding(.vertical, 4)
        }
        .disabled(vpnManager.isConnected || vpnManager.isTransitioning)
    }
}
