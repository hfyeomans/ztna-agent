import Foundation

// MARK: - IPv4 Parsing

/// Parse an IPv4 dotted-decimal string into 4 bytes.
/// Returns `[0, 0, 0, 0]` if the format is invalid.
func parseIPv4(_ host: String) -> [UInt8] {
    let components = host.split(separator: ".").compactMap { UInt8($0) }
    guard components.count == 4 else { return [0, 0, 0, 0] }
    return components
}

/// Convert an IPv4 dotted-decimal string to a network-order UInt32.
/// Returns nil for invalid addresses (except literal "0.0.0.0").
func ipv4ToUInt32(_ host: String) -> UInt32? {
    let bytes = parseIPv4(host)
    guard bytes != [0, 0, 0, 0] || host == "0.0.0.0" else { return nil }
    return UInt32(bytes[0]) << 24 | UInt32(bytes[1]) << 16 | UInt32(bytes[2]) << 8 | UInt32(bytes[3])
}

/// Extract destination IPv4 address (bytes 16-19) from an IP packet.
/// Returns nil if the packet is shorter than a minimal IPv4 header (20 bytes).
func extractDestIPv4(_ packet: Data) -> UInt32? {
    guard packet.count >= 20 else { return nil }
    return UInt32(packet[16]) << 24 | UInt32(packet[17]) << 16 | UInt32(packet[18]) << 8 | UInt32(packet[19])
}

// MARK: - Routed Datagram

/// Build a routed datagram with wire format: `[0x2F, id_len, service_id_bytes..., ip_packet...]`
func buildRoutedDatagram(serviceId: String, packet: Data) -> Data {
    let idBytes = Array(serviceId.utf8)
    var wrapped = Data(capacity: 2 + idBytes.count + packet.count)
    wrapped.append(0x2F)
    wrapped.append(UInt8(idBytes.count))
    wrapped.append(contentsOf: idBytes)
    wrapped.append(packet)
    return wrapped
}
