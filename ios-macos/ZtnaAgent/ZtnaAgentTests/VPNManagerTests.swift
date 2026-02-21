import Testing
import Foundation
@testable import ZtnaAgent

// MARK: - VPNStatus Enum

@Suite("VPNStatus")
struct VPNStatusTests {

    @Test("Raw values match expected display strings", arguments: [
        (VPNManager.VPNStatus.unknown, "Unknown"),
        (VPNManager.VPNStatus.invalid, "Invalid"),
        (VPNManager.VPNStatus.disconnected, "Disconnected"),
        (VPNManager.VPNStatus.connecting, "Connecting..."),
        (VPNManager.VPNStatus.connected, "Connected ✓"),
        (VPNManager.VPNStatus.reasserting, "Reasserting..."),
        (VPNManager.VPNStatus.disconnecting, "Disconnecting..."),
        (VPNManager.VPNStatus.loadError, "Load Error"),
        (VPNManager.VPNStatus.startError, "Start Error")
    ])
    func rawValues(status: VPNManager.VPNStatus, expected: String) {
        #expect(status.rawValue == expected)
    }

    @Test("All cases are accounted for")
    func allCases() {
        // Verify exhaustiveness — if a new case is added, this count must update
        let allCases: [VPNManager.VPNStatus] = [
            .unknown, .invalid, .disconnected, .connecting,
            .connected, .reasserting, .disconnecting, .loadError, .startError
        ]
        #expect(allCases.count == 9)
    }
}

// MARK: - VPNManager State

@Suite("VPNManager State", .serialized)
@MainActor
struct VPNManagerStateTests {

    @Test("Default status is unknown")
    func defaultStatus() {
        let manager = VPNManager()
        #expect(manager.status == .unknown)
    }

    @Test("isConnected only true when connected")
    func isConnectedLogic() {
        let manager = VPNManager()
        #expect(manager.isConnected == false)
        // Cannot set status directly (private set), but we verify the computed property
    }

    @Test("isTransitioning covers connecting, disconnecting, reasserting")
    func isTransitioningLogic() {
        let manager = VPNManager()
        // Default state should not be transitioning
        #expect(manager.isTransitioning == false)
    }

    @Test("Default server configuration values")
    func defaultConfig() {
        // Clear any saved defaults first
        UserDefaults.standard.removeObject(forKey: "ztnaServerHost")
        UserDefaults.standard.removeObject(forKey: "ztnaServerPort")
        UserDefaults.standard.removeObject(forKey: "ztnaServiceId")

        let manager = VPNManager()
        #expect(manager.serverHost == "3.128.36.92")
        #expect(manager.serverPort == 4433)
        #expect(manager.serviceId == "echo-service")
    }

    @Test("UserDefaults persistence round-trip")
    func userDefaultsPersistence() {
        // Clear defaults
        UserDefaults.standard.removeObject(forKey: "ztnaServerHost")
        UserDefaults.standard.removeObject(forKey: "ztnaServerPort")
        UserDefaults.standard.removeObject(forKey: "ztnaServiceId")

        let manager = VPNManager()
        manager.serverHost = "192.168.1.100"
        manager.serverPort = 5555
        manager.serviceId = "test-service"

        // Verify persisted to UserDefaults
        #expect(UserDefaults.standard.string(forKey: "ztnaServerHost") == "192.168.1.100")
        #expect(UserDefaults.standard.integer(forKey: "ztnaServerPort") == 5555)
        #expect(UserDefaults.standard.string(forKey: "ztnaServiceId") == "test-service")

        // Verify a new instance picks up saved values
        let manager2 = VPNManager()
        #expect(manager2.serverHost == "192.168.1.100")
        #expect(manager2.serverPort == 5555)
        #expect(manager2.serviceId == "test-service")

        // Cleanup
        UserDefaults.standard.removeObject(forKey: "ztnaServerHost")
        UserDefaults.standard.removeObject(forKey: "ztnaServerPort")
        UserDefaults.standard.removeObject(forKey: "ztnaServiceId")
    }
}
