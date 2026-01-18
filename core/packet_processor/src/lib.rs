use std::slice;

#[repr(C)]
pub enum PacketAction {
    Drop = 0,
    Forward = 1,
}

#[no_mangle]
pub extern "C" fn process_packet(data: *const u8, len: libc::size_t) -> PacketAction {
    if data.is_null() || len == 0 {
        return PacketAction::Forward;
    }

    let slice = unsafe { slice::from_raw_parts(data, len) };

    match etherparse::SlicedPacket::from_ip(slice) {
        Err(value) => {
            // Not a valid IP packet, or something we can't parse easily
            // For MVP, just forward it
            // In real app, might want to log this
             PacketAction::Forward
        },
        Ok(value) => {
             // For MVP: Log the packet destination
             if let Some(ip) = value.ip {
                 match ip {
                     etherparse::InternetSlice::Ipv4(header, _ext) => {
                         println!("[Rust] IPv4 Packet: {:?} -> {:?}", header.source_addr(), header.destination_addr());
                     }
                     etherparse::InternetSlice::Ipv6(header, _ext) => {
                         println!("[Rust] IPv6 Packet: {:?} -> {:?}", header.source_addr(), header.destination_addr());
                     }
                 }
             }
             PacketAction::Forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_packet() {
        // Simple test to ensure it runs
        let data = [0u8; 20]; // Mock empty packet
        let action = process_packet(data.as_ptr(), data.len());
        // Since it's garbage data, from_ip should Err or fail parsing, defaulting to Forward
        match action {
            PacketAction::Forward => assert!(true),
            PacketAction::Drop => assert!(false),
        }
    }
}
