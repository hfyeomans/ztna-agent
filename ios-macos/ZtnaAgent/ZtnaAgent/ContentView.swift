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
        let manager = NEVPNManager.shared()
        
        manager.loadFromPreferences { error in
            if let error = error {
                print("Load error: \(error)")
                status = "Load Error"
                return
            }
            
            let protocolConfiguration = NETunnelProviderProtocol()
            // IMPORTANT: This Bundle ID must match your Network Extension target's Bundle ID
            protocolConfiguration.providerBundleIdentifier = "com.hankyeomans.ztna-agent.ZtnaAgent.Extension"
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
                
                do {
                    try manager.connection.startVPNTunnel()
                    status = "Starting..."
                } catch {
                    print("Start error: \(error)")
                    status = "Start Failed"
                }
            }
        }
    }
}
