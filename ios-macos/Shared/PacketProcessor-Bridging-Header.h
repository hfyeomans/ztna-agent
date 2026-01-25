#ifndef PacketProcessor_Bridging_Header_h
#define PacketProcessor_Bridging_Header_h

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

// ============================================================================
// Legacy Packet Processing (kept for compatibility)
// ============================================================================

// Define the enum to match Rust's Repr(C) enum
typedef enum {
    PacketActionDrop = 0,
    PacketActionForward = 1,
} PacketAction;

// Legacy packet filter function
PacketAction process_packet(const uint8_t *data, size_t len);

// ============================================================================
// QUIC Agent Types
// ============================================================================

// Opaque pointer to Rust Agent struct
typedef struct Agent Agent;

// Agent connection state
typedef enum {
    AgentStateDisconnected = 0,
    AgentStateConnecting = 1,
    AgentStateConnected = 2,
    AgentStateDraining = 3,
    AgentStateClosed = 4,
    AgentStateError = 5,
} AgentState;

// Result codes for agent operations
typedef enum {
    AgentResultOk = 0,
    AgentResultInvalidPointer = 1,
    AgentResultInvalidAddress = 2,
    AgentResultConnectionFailed = 3,
    AgentResultNotConnected = 4,
    AgentResultBufferTooSmall = 5,
    AgentResultNoData = 6,
    AgentResultQuicError = 7,
    AgentResultPanicCaught = 8,
} AgentResult;

// ============================================================================
// QUIC Agent Lifecycle
// ============================================================================

/// Create a new QUIC agent instance.
/// Returns: Pointer to agent, or NULL on failure.
/// Caller is responsible for calling agent_destroy when done.
Agent* agent_create(void);

/// Destroy an agent instance and free its resources.
/// @param agent Pointer created by agent_create (may be NULL).
void agent_destroy(Agent* agent);

/// Get the current agent connection state.
/// @param agent Agent pointer (NULL-safe, returns AgentStateError).
AgentState agent_get_state(const Agent* agent);

// ============================================================================
// QUIC Agent Connection Management
// ============================================================================

/// Initiate connection to a QUIC server.
/// @param agent Agent pointer.
/// @param host Server hostname or IP address (null-terminated C string).
/// @param port Server port number.
/// @return AgentResultOk on success, error code otherwise.
AgentResult agent_connect(Agent* agent, const char* host, uint16_t port);

/// Check if the agent is currently connected.
/// @param agent Agent pointer.
/// @return true if connected, false otherwise.
bool agent_is_connected(const Agent* agent);

/// Register the Agent for a target service.
/// This tells the Intermediate Server which service the Agent wants to reach.
/// Must be called after the connection is established (agent_is_connected returns true).
/// @param agent Agent pointer.
/// @param service_id Service ID to register for (null-terminated C string).
/// @return AgentResultOk on success, AgentResultNotConnected if not connected.
AgentResult agent_register(Agent* agent, const char* service_id);

// ============================================================================
// QUIC Agent Packet I/O
// ============================================================================

/// Feed a received UDP packet to the QUIC connection.
/// Call this when UDP data is received from the network.
/// @param agent Agent pointer.
/// @param data Pointer to received packet data.
/// @param len Length of received data.
/// @param from_ip Source IPv4 address as 4 bytes (network order).
/// @param from_port Source port (host byte order).
/// @return AgentResultOk on success, error code otherwise.
AgentResult agent_recv(Agent* agent, const uint8_t* data, size_t len,
                       const uint8_t* from_ip, uint16_t from_port);

/// Poll for outbound UDP packets that need to be sent.
/// Call this repeatedly until AgentResultNoData is returned.
/// @param agent Agent pointer.
/// @param out_data Buffer to write packet data into.
/// @param out_len On input: buffer capacity. On output: actual length written.
/// @param out_port On output: destination port for the packet.
/// @return AgentResultOk if packet was written, AgentResultNoData if empty, error otherwise.
AgentResult agent_poll(Agent* agent, uint8_t* out_data, size_t* out_len, uint16_t* out_port);

/// Send an IP packet through the QUIC tunnel as a DATAGRAM.
/// The packet will be encapsulated and sent to the server.
/// @param agent Agent pointer.
/// @param data IP packet data to send.
/// @param len Length of IP packet.
/// @return AgentResultOk on success, AgentResultNotConnected if not connected.
AgentResult agent_send_datagram(Agent* agent, const uint8_t* data, size_t len);

// ============================================================================
// QUIC Agent Timeout Handling
// ============================================================================

/// Handle a timeout event.
/// Call this when the timeout duration (from agent_timeout_ms) has elapsed.
/// @param agent Agent pointer.
void agent_on_timeout(Agent* agent);

/// Get milliseconds until the next timeout event.
/// @param agent Agent pointer.
/// @return Milliseconds until timeout, or 0 if no timeout pending.
uint64_t agent_timeout_ms(const Agent* agent);

// ============================================================================
// QUIC Address Discovery (QAD)
// ============================================================================

/// Get the observed public address discovered via QAD.
/// This is the agent's public IP:port as seen by the server.
/// @param agent Agent pointer.
/// @param out_ip Buffer for IPv4 address (4 bytes).
/// @param out_port On output: observed port number.
/// @return AgentResultOk if address available, AgentResultNoData if not yet discovered.
AgentResult agent_get_observed_address(const Agent* agent, uint8_t* out_ip, uint16_t* out_port);

#endif /* PacketProcessor_Bridging_Header_h */
