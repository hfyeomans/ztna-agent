import SwiftUI
import NetworkExtension

@main
struct ZtnaAgentApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    @State private var status = "Unknown"
    
    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "lock.shield")
                .font(.system(size: 60))
                .foregroundColor(.blue)
            
            Text("ZTNA Agent MVP")
                .font(.title)
            
            Text("Status: \(status)")
                .font(.headline)
                .foregroundColor(.gray)
            
            Button("Install & Start VPN") {
                setupAndStartVPN()
            }
            .padding()
            .background(Color.blue)
            .foregroundColor(.white)
            .cornerRadius(10)
        }
        .padding()
        .frame(width: 400, height: 300)
    }
    
    func setupAndStartVPN() {
        // Use NETunnelProviderManager for Custom VPNs (Packet Tunnel), NOT NEVPNManager (Personal VPN)
        NETunnelProviderManager.loadAllFromPreferences { managers, error in
            if let error = error {
                print("Load error: \(error)")
                status = "Load Error"
                return
            }
            
            // Re-use existing manager/preference if found, or create a new one
            let manager = managers?.first ?? NETunnelProviderManager()
            
            let protocolConfiguration = NETunnelProviderProtocol()
            // IMPORTANT: This Bundle ID must match your Network Extension target's Bundle ID
            protocolConfiguration.providerBundleIdentifier = "com.hankyeomans.ztna-agent.Extension"
            protocolConfiguration.serverAddress = "127.0.0.1" // Virtual
            
            manager.protocolConfiguration = protocolConfiguration
            manager.isEnabled = true
            manager.localizedDescription = "ZTNA Agent"
            
            manager.saveToPreferences { error in
                if let error = error {
                    print("Save error: \(error)")
                    status = "Save Error"
                    return
                }
                
                // Reload to ensure we have the valid reference before starting
                manager.loadFromPreferences { _ in
                    do {
                        try manager.connection.startVPNTunnel()
                        self.status = "Starting..."
                    } catch {
                        print("Start error: \(error)")
                        self.status = "Start Failed"
                    }
                }
            }
        }
    }
}
