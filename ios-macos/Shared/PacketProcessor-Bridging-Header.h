#ifndef PacketProcessor_Bridging_Header_h
#define PacketProcessor_Bridging_Header_h

#include <stdint.h>
#include <stddef.h>

// Define the enum to match Rust's Repr(C) enum
typedef enum {
    PacketActionDrop = 0,
    PacketActionForward = 1,
} PacketAction;

// Expose the Rust function
// pub extern "C" fn process_packet(data: *const u8, len: libc::size_t) -> PacketAction;
PacketAction process_packet(const uint8_t *data, size_t len);

#endif /* PacketProcessor_Bridging_Header_h */
