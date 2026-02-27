#ifndef PacketProcessor_Bridging_Header_h
#define PacketProcessor_Bridging_Header_h

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

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
    // Specific QUIC error codes for debugging (10+)
    AgentResultQuicDone = 10,
    AgentResultQuicBufferTooShort = 11,
    AgentResultQuicUnknownVersion = 12,
    AgentResultQuicInvalidFrame = 13,
    AgentResultQuicInvalidPacket = 14,
    AgentResultQuicInvalidState = 15,
    AgentResultQuicInvalidStreamState = 16,
    AgentResultQuicInvalidTransportParam = 17,
    AgentResultQuicCryptoFail = 18,
    AgentResultQuicTlsFail = 19,
    AgentResultQuicFlowControl = 20,
    AgentResultQuicStreamLimit = 21,
    AgentResultQuicStreamStopped = 22,
    AgentResultQuicStreamReset = 23,
    AgentResultQuicFinalSize = 24,
    AgentResultQuicCongestionControl = 25,
    AgentResultQuicIdLimit = 26,
    AgentResultQuicOutOfIdentifiers = 27,
    AgentResultQuicKeyUpdate = 28,
} AgentResult;

// ============================================================================
// QUIC Agent Lifecycle
// ============================================================================

/// Create a new QUIC agent instance with TLS configuration.
/// @param ca_cert_path Path to CA certificate PEM file for server verification.
///   Pass NULL to use the system CA store.
/// @param verify_peer Whether to verify the server's TLS certificate.
///   Should be true in production. Pass false for dev with self-signed certs.
/// Returns: Pointer to agent, or NULL on failure.
/// Caller is responsible for calling agent_destroy when done.
Agent* agent_create(const char* ca_cert_path, bool verify_peer);

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

/// Set the local UDP address (used as RecvInfo.to for quiche path validation).
/// Call this after the NWConnection reports its local endpoint.
/// @param agent Agent pointer.
/// @param ip Local IPv4 address as bytes.
/// @param ip_len Length of the IP address buffer (must be >= 4).
/// @param port Local port (host byte order).
/// @return AgentResultOk on success, AgentResultInvalidPointer if ip_len < 4.
AgentResult agent_set_local_addr(Agent* agent, const uint8_t* ip, size_t ip_len, uint16_t port);

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

/// Send a keepalive PING on the Intermediate connection.
/// Call this periodically (e.g., every 10 seconds) to prevent the QUIC
/// connection from timing out due to inactivity (30 second idle timeout).
/// @param agent Agent pointer.
/// @return AgentResultOk if keepalive was sent, AgentResultNotConnected if not connected.
AgentResult agent_send_intermediate_keepalive(Agent* agent);

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

/// Poll for received IP packets from the QUIC tunnel.
/// Call this repeatedly after agent_recv() until AgentResultNoData is returned.
/// Each call returns one IP packet received via QUIC DATAGRAM (response from Connector).
/// @param agent Agent pointer.
/// @param out_data Buffer to write IP packet data into.
/// @param out_len On input: buffer capacity. On output: actual length written.
/// @return AgentResultOk if packet was written, AgentResultNoData if empty.
AgentResult agent_recv_datagram(Agent* agent, uint8_t* out_data, size_t* out_len);

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

// ============================================================================
// P2P Connections
// ============================================================================

/// Initiate a P2P QUIC connection to a Connector at the given address.
/// Call after hole punching discovers a working address.
/// @param agent Agent pointer.
/// @param host Connector IP address (null-terminated C string).
/// @param port Connector port.
/// @return AgentResultOk on success, error code otherwise.
AgentResult agent_connect_p2p(Agent* agent, const char* host, uint16_t port);

/// Check if a P2P connection is established to the given address.
/// @param agent Agent pointer.
/// @param host Connector IP address (null-terminated C string).
/// @param port Connector port.
/// @return true if P2P connected, false otherwise.
bool agent_is_p2p_connected(const Agent* agent, const char* host, uint16_t port);

/// Poll for outbound UDP packets from P2P connections.
/// Call repeatedly until AgentResultNoData is returned.
/// @param agent Agent pointer.
/// @param out_data Buffer to write packet data.
/// @param out_len On input: buffer capacity. On output: actual length written.
/// @param out_ip Buffer for destination IPv4 address (4 bytes).
/// @param out_port On output: destination port.
/// @return AgentResultOk if packet written, AgentResultNoData if empty.
AgentResult agent_poll_p2p(Agent* agent, uint8_t* out_data, size_t* out_len,
                           uint8_t* out_ip, uint16_t* out_port);

/// Send an IP packet through a P2P connection as a DATAGRAM.
/// @param agent Agent pointer.
/// @param data IP packet data.
/// @param len Length of IP packet.
/// @param dest_ip Destination Connector IPv4 address (4 bytes).
/// @param dest_port Destination Connector port.
/// @return AgentResultOk on success, AgentResultNotConnected if no P2P connection.
AgentResult agent_send_datagram_p2p(Agent* agent, const uint8_t* data, size_t len,
                                     const uint8_t* dest_ip, uint16_t dest_port);

// ============================================================================
// Hole Punching
// ============================================================================

/// Start hole punching for a service.
/// Initiates P2P negotiation to establish a direct connection to the Connector.
/// Sends CandidateOffer via signaling stream through the Intermediate.
/// @param agent Agent pointer.
/// @param service_id Service to connect to (null-terminated C string).
/// @return AgentResultOk on success, error code otherwise.
AgentResult agent_start_hole_punch(Agent* agent, const char* service_id);

/// Poll hole punching progress.
/// @param agent Agent pointer.
/// @param out_ip Buffer for working IPv4 address (4 bytes).
/// @param out_port On output: working port.
/// @param out_complete Set to 1 if hole punching is complete, 0 otherwise.
/// @return AgentResultOk if working address available, AgentResultNoData otherwise.
AgentResult agent_poll_hole_punch(Agent* agent, uint8_t* out_ip, uint16_t* out_port,
                                   uint8_t* out_complete);

/// Get binding requests to send for hole punching.
/// Returns STUN-like binding requests that must be sent to candidate addresses.
/// @param agent Agent pointer.
/// @param out_data Buffer for binding request data.
/// @param out_len On input: buffer capacity. On output: data length.
/// @param out_ip Buffer for destination IPv4 address (4 bytes).
/// @param out_port On output: destination port.
/// @return AgentResultOk if request available, AgentResultNoData otherwise.
AgentResult agent_poll_binding_request(Agent* agent, uint8_t* out_data, size_t* out_len,
                                        uint8_t* out_ip, uint16_t* out_port);

/// Process a received binding response from a candidate.
/// Feed responses back to the hole punch state machine.
/// @param agent Agent pointer.
/// @param data Binding response data.
/// @param len Data length.
/// @param from_ip Source IPv4 address (4 bytes).
/// @param from_port Source port.
/// @return AgentResultOk on success, error code otherwise.
AgentResult agent_process_binding_response(Agent* agent, const uint8_t* data, size_t len,
                                            const uint8_t* from_ip, uint16_t from_port);

// ============================================================================
// Path Resilience
// ============================================================================

/// Poll for a keepalive message to send on the P2P path.
/// @param agent Agent pointer.
/// @param out_ip Buffer for destination IPv4 address (4 bytes).
/// @param out_port On output: destination port.
/// @param out_data Buffer for keepalive message (6 bytes minimum).
/// @return AgentResultOk if keepalive should be sent, AgentResultNoData otherwise.
AgentResult agent_poll_keepalive(Agent* agent, uint8_t* out_ip, uint16_t* out_port,
                                  uint8_t* out_data);

/// Get the current active path type.
/// @param agent Agent pointer.
/// @return 0 = Direct, 1 = Relay, 2 = None.
uint8_t agent_get_active_path(const Agent* agent);

/// Check if the agent is in fallback mode (relay after direct path failure).
/// @param agent Agent pointer.
/// @return true if in fallback, false otherwise.
bool agent_is_in_fallback(const Agent* agent);

/// Get path statistics for monitoring.
/// @param agent Agent pointer.
/// @param out_missed_keepalives Output: count of missed keepalives.
/// @param out_rtt_ms Output: RTT in milliseconds (0 if not measured).
/// @param out_in_fallback Output: 1 if in fallback, 0 if not.
/// @return AgentResultOk on success.
AgentResult agent_get_path_stats(const Agent* agent, uint32_t* out_missed_keepalives,
                                  uint64_t* out_rtt_ms, uint8_t* out_in_fallback);

#endif /* PacketProcessor_Bridging_Header_h */
