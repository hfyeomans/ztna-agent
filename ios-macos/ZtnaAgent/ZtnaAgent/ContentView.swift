import SwiftUI
import NetworkExtension
import Observation

@main
struct ZtnaAgentApp: App {
    @State private var vpnManager = VPNManager()
    
    var body: some Scene {
        WindowGroup {
            ContentView(vpnManager: vpnManager)
        }
    }
}

@Observable
@MainActor
final class VPNManager {
    private(set) var status: VPNStatus = .unknown
    private var manager: NETunnelProviderManager?
    nonisolated(unsafe) private var statusTask: Task<Void, Never>?
    
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
            config.serverAddress = "192.0.2.1"
            
            mgr.protocolConfiguration = config
            mgr.isEnabled = true
            mgr.localizedDescription = "ZTNA Agent"
            
            try await mgr.saveToPreferences()
            try await mgr.loadFromPreferences()
            try mgr.connection.startVPNTunnel()
            
            status = .connecting
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
    let vpnManager: VPNManager
    
    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: vpnManager.isConnected ? "lock.shield.fill" : "lock.shield")
                .font(.system(size: 56))
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
        }
        .padding(40)
        .frame(width: 400, height: 300)
    }
}
