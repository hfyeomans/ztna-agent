import Testing
import Foundation
@testable import ZtnaAgent

// MARK: - IPv4 Parsing

@Suite("IPv4 Parsing")
struct IPv4ParsingTests {

    @Test("Valid dotted-decimal addresses", arguments: [
        ("192.168.1.1", [192, 168, 1, 1] as [UInt8]),
        ("10.0.0.1", [10, 0, 0, 1] as [UInt8]),
        ("255.255.255.255", [255, 255, 255, 255] as [UInt8]),
        ("0.0.0.0", [0, 0, 0, 0] as [UInt8]),
        ("127.0.0.1", [127, 0, 0, 1] as [UInt8])
    ])
    func parseIPv4Valid(input: String, expected: [UInt8]) {
        #expect(parseIPv4(input) == expected)
    }

    @Test("Invalid addresses return zeroes", arguments: [
        "not-an-ip",
        "192.168.1",
        "192.168.1.1.1",
        "",
        "256.1.1.1",
        "abc.def.ghi.jkl"
    ])
    func parseIPv4Invalid(input: String) {
        #expect(parseIPv4(input) == [0, 0, 0, 0])
    }
}

// MARK: - IPv4 to UInt32

@Suite("IPv4 to UInt32 Conversion")
struct IPv4ToUInt32Tests {

    @Test("Known address conversions", arguments: [
        ("10.100.0.1", UInt32(0x0A640001)),
        ("192.168.1.1", UInt32(0xC0A80101)),
        ("255.255.255.255", UInt32(0xFFFFFFFF)),
        ("0.0.0.0", UInt32(0x00000000)),
        ("1.2.3.4", UInt32(0x01020304))
    ])
    func ipv4ToUInt32Valid(input: String, expected: UInt32) throws {
        let result = try #require(ipv4ToUInt32(input))
        #expect(result == expected)
    }

    @Test("Invalid addresses return nil", arguments: [
        "not-an-ip",
        "192.168.1",
        ""
    ])
    func ipv4ToUInt32Invalid(input: String) {
        #expect(ipv4ToUInt32(input) == nil)
    }
}

// MARK: - Destination IP Extraction

@Suite("Packet Header Parsing")
struct PacketHeaderTests {

    @Test("Extract destination from minimal 20-byte IPv4 header")
    func extractDestFromMinimalPacket() throws {
        // Minimal IPv4 header: 20 bytes, destination IP at bytes 16-19
        var packet = Data(repeating: 0, count: 20)
        packet[16] = 10    // 10.100.0.1
        packet[17] = 100
        packet[18] = 0
        packet[19] = 1
        let result = try #require(extractDestIPv4(packet))
        #expect(result == 0x0A640001)
    }

    @Test("Extract destination from larger packet")
    func extractDestFromLargerPacket() throws {
        var packet = Data(repeating: 0xFF, count: 60)
        packet[16] = 192   // 192.168.1.100
        packet[17] = 168
        packet[18] = 1
        packet[19] = 100
        let result = try #require(extractDestIPv4(packet))
        #expect(result == 0xC0A80164)
    }

    @Test("Short packets return nil", arguments: [0, 1, 10, 19])
    func shortPacketReturnsNil(length: Int) {
        let packet = Data(repeating: 0, count: length)
        #expect(extractDestIPv4(packet) == nil)
    }

    @Test("Exactly 20 bytes is the minimum valid length")
    func exactMinimumLength() {
        let packet = Data(repeating: 0, count: 20)
        #expect(extractDestIPv4(packet) != nil)
    }
}

// MARK: - Routed Datagram Wire Format

@Suite("Routed Datagram")
struct RoutedDatagramTests {

    @Test("Wire format: header + service ID + payload")
    func basicWireFormat() {
        let payload = Data([0x45, 0x00, 0x00, 0x3C]) // Fake IP header start
        let result = buildRoutedDatagram(serviceId: "echo-service", packet: payload)

        // Header byte
        #expect(result[0] == 0x2F)
        // Service ID length
        #expect(result[1] == UInt8("echo-service".utf8.count))
        // Service ID bytes
        let idRange = result[2..<(2 + Int(result[1]))]
        #expect(Array(idRange) == Array("echo-service".utf8))
        // Payload follows immediately after service ID
        let payloadStart = 2 + Int(result[1])
        #expect(Array(result[payloadStart...]) == Array(payload))
    }

    @Test("Total length is correct")
    func totalLength() {
        let payload = Data(repeating: 0xAB, count: 100)
        let serviceId = "web-app"
        let result = buildRoutedDatagram(serviceId: serviceId, packet: payload)
        // 1 (header) + 1 (length) + serviceId.count + payload.count
        #expect(result.count == 2 + serviceId.utf8.count + payload.count)
    }

    @Test("Empty payload produces header-only datagram")
    func emptyPayload() {
        let result = buildRoutedDatagram(serviceId: "svc", packet: Data())
        #expect(result.count == 2 + 3) // header + len + "svc"
        #expect(result[0] == 0x2F)
        #expect(result[1] == 3)
    }

    @Test("Single-char service ID")
    func singleCharServiceId() {
        let result = buildRoutedDatagram(serviceId: "x", packet: Data([0xFF]))
        #expect(result == Data([0x2F, 0x01, 0x78, 0xFF])) // 0x78 = 'x'
    }
}
