# ZTNA Agent Architecture

## Overview

The Zero Trust Network Access (ZTNA) Agent provides secure, identity-aware access to private applications without exposing them to the public internet. Traffic is tunneled through QUIC, with NAT traversal handled natively via **QUIC Address Discovery (QAD)** — eliminating the need for traditional STUN/TURN servers.

---

## Connection Strategy: Direct P2P First

**The primary architectural goal is direct peer-to-peer connectivity.** The Intermediate System serves as bootstrap and fallback, not the intended steady-state data path.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONNECTION PRIORITY                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRIORITY 1: DIRECT P2P (preferred, lowest latency)                         │
│  ════════════════════════════════════════════════                           │
│     Agent ◄────────────── QUIC Direct ──────────────► App Connector         │
│                                                                              │
│     • Best performance (no relay hop)                                        │
│     • No load on Intermediate System                                         │
│     • Requires successful NAT hole punching                                  │
│     • Uses QUIC connection migration from relay to direct                    │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRIORITY 2: RELAYED (fallback, always works)                               │
│  ════════════════════════════════════════════                               │
│     Agent ◄───► Intermediate System ◄───► App Connector                     │
│                                                                              │
│     • Guaranteed connectivity (works behind strict NAT/firewall)             │
│     • Higher latency (extra hop through relay)                               │
│     • Intermediate bears relay bandwidth cost                                │
│     • Used when hole punching fails or during initial connection             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Connection Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONNECTION LIFECYCLE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PHASE 1: BOOTSTRAP (via Intermediate)                                       │
│  ─────────────────────────────────────                                       │
│  1. Agent connects to Intermediate System (QUIC handshake)                  │
│  2. Agent receives QAD observed address                                      │
│  3. Agent registers: DATAGRAM [0x10, len, "echo-service"]                   │
│  4. App Connector connects and registers: DATAGRAM [0x11, len, "echo-service"]
│  5. Intermediate routes data: Agent ↔ Intermediate ↔ Connector             │
│                                                                              │
│  PHASE 2: HOLE PUNCH ATTEMPT (parallel to data flow)                         │
│  ───────────────────────────────────────────────────                         │
│  1. Agent initiates hole punch: CandidateOffer message                      │
│  2. Intermediate relays candidates to Connector                             │
│  3. Both sides send QUIC packets directly to peer's public IP               │
│  4. If NAT bindings align → direct path opens                                │
│                                                                              │
│  PHASE 3: DIRECT PATH (on successful hole punch)                             │
│  ───────────────────────────────────────────────                             │
│  1. New QUIC connection established directly Agent ↔ Connector             │
│  2. Data flows on direct path (lower latency)                               │
│  3. Intermediate remains for signaling only                                  │
│                                                                              │
│  FALLBACK: CONTINUE RELAY (if hole punch fails)                              │
│  ──────────────────────────────────────────────                              │
│  • Strict NAT/firewall prevents direct connection                            │
│  • Continue using relay path indefinitely                                    │
│  • Automatic retry after 30 second cooldown                                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Implementation Phases

| Phase | Task | Status | Connection Mode |
|-------|------|--------|-----------------|
| **001** | Agent QUIC Client | ✅ Complete | Agent + FFI ready |
| **002** | Intermediate Server | ✅ Complete | Relay + QAD |
| **003** | App Connector | ✅ Complete | Relay + Registration |
| **004** | E2E Relay Testing | ✅ Complete | 61+ E2E tests, relay verified |
| **005** | P2P Hole Punching | ✅ Complete | 81 unit tests, protocol ready |
| **005a** | Swift Agent Integration | ✅ Complete | macOS VPN + QUIC |
| **006** | Cloud Deployment | 🔄 In Progress | Config, TCP/ICMP, 0x2F routing done |

### Why This Matters

| Metric | Relay Mode | Direct P2P |
|--------|------------|------------|
| Latency | +20-100ms (relay hop) | Optimal |
| Bandwidth Cost | Intermediate pays | Peers only |
| Scalability | Limited by relay capacity | Unlimited |
| Privacy | Intermediate sees metadata | End-to-end only |

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              ZTNA Architecture                                   │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│   ┌─────────────┐         ┌─────────────────────┐         ┌─────────────────┐   │
│   │  Endpoint   │  QUIC   │   Intermediate      │  QUIC   │  App Connector  │   │
│   │   Agent     │◄───────►│      System         │◄───────►│                 │   │
│   │  (macOS)    │ Tunnel  │   (Relay + QAD)     │ Tunnel  │  (k8s/Docker)   │   │
│   └─────────────┘         └─────────────────────┘         └─────────────────┘   │
│         │                          │                              │             │
│         │                          │                              │             │
│   ┌─────▼─────┐            ┌───────▼───────┐              ┌───────▼───────┐     │
│   │ User Apps │            │ Public IP:Port│              │ Private Apps  │     │
│   │ (Browser) │            │   Discovery   │              │ (HTTP, SSH,   │     │
│   └───────────┘            └───────────────┘              │  Databases)   │     │
│                                                           └───────────────┘     │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Components

### 1. Endpoint Agent (macOS/iOS)

The agent runs on user devices and intercepts network traffic destined for protected applications.

**Responsibilities:**
- Intercept IP packets via `NEPacketTunnelProvider` (Network Extension)
- **Split-tunnel routing:** Only route configured virtual IPs (10.100.0.0/24) through tunnel
- Establish QUIC connection to Intermediate System
- **Service registration:** Register for all configured services via 0x10 DATAGRAM
- **0x2F service-routed wrapping:** Lookup destination IP → service ID, wrap packet with 0x2F header
- Learn public IP:Port via QAD (no STUN required)
- **Keepalive:** 10-second PING interval prevents 30s QUIC idle timeout
- Handle connection migration on network changes

**Configuration:**
- Server host, port, service ID configurable via SwiftUI UI + UserDefaults
- Service definitions with virtual IPs passed to extension via `NETunnelProviderProtocol.providerConfiguration`
- Route table built from services array: `{virtualIp → serviceId}`

**Technology Stack:**
- Swift 6.2 / SwiftUI (host app)
- Rust (packet processing, QUIC via `quiche`)
- NetworkExtension framework

**Key Files:**
```
ios-macos/ZtnaAgent/
├── ZtnaAgent/ContentView.swift      # Host app UI + VPNManager (configurable)
├── Extension/PacketTunnelProvider.swift  # Packet interception + 0x2F routing
core/packet_processor/
└── src/lib.rs                       # Rust FFI for packet processing
deploy/config/
└── agent.json                       # Reference config (services + virtualIps)
```

---

### 2. Intermediate System (Relay + QAD Server)

The Intermediate System is the rendezvous point between Agents and App Connectors. It handles NAT traversal and relays traffic when direct P2P connections aren't possible.

**Responsibilities:**
- Accept QUIC connections from Agents and Connectors
- Perform **QUIC Address Discovery (QAD)** — report observed public IP:Port to clients
- Route DATAGRAM frames between matched Agent/Connector pairs
- Facilitate P2P hole punching (future optimization)
- Authenticate and authorize connections

**Deployment:**
- Cloud VM with public IP (AWS, GCP, Azure, etc.)
- Rust binary using `quiche` as QUIC server
- Stateless design for horizontal scaling

---

### 3. App Connector

The App Connector runs alongside private applications and provides the "last mile" connection from the ZTNA tunnel to the actual service.

**Responsibilities:**
- Establish persistent QUIC connection to Intermediate System
- Register as endpoint for specific services/applications (0x11)
- Receive encapsulated IP packets via DATAGRAM frames
- **Multi-protocol support:** UDP forwarding, TCP proxy, ICMP Echo Reply
- **UDP:** Extract payload → forward to backend → encapsulate return IP/UDP packet
- **TCP:** Userspace proxy with session tracking (SYN→connect, data→stream, FIN→close)
- **ICMP:** Generate Echo Reply at Connector (swap src/dst, no backend needed)
- **JSON config:** `--config` flag for service definitions, backend addresses, P2P certs
- **Keepalive:** 10-second QUIC PING prevents idle timeout
- Handle response traffic back through the tunnel

**Deployment Options:**

#### Kubernetes Sidecar
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-app
spec:
  containers:
  - name: app
    image: my-app:latest
    ports:
    - containerPort: 8080
  - name: ztna-connector
    image: ztna-connector:latest
    env:
    - name: INTERMEDIATE_SERVER
      value: "relay.example.com:4433"
    - name: SERVICE_PORT
      value: "8080"
```

#### Docker Compose
```yaml
version: '3.8'
services:
  app:
    image: my-app:latest
    ports:
      - "8080:8080"
    networks:
      - ztna-network

  ztna-connector:
    image: ztna-connector:latest
    environment:
      INTERMEDIATE_SERVER: relay.example.com:4433
      SERVICE_HOST: app
      SERVICE_PORT: 8080
    networks:
      - ztna-network

networks:
  ztna-network:
    driver: bridge
```

---

## QUIC Address Discovery (QAD)

### The Problem with STUN

Traditional NAT traversal uses STUN (Session Traversal Utilities for NAT) servers to discover public IP addresses:

```
┌────────────────────────────────────────────────────────────────┐
│                    Traditional STUN Flow                        │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Client ──── STUN Request ────► STUN Server                   │
│          ◄─── STUN Response ────                               │
│               (Your IP: 203.0.113.5:54321)                     │
│                                                                 │
│   Client ──── Signaling ────────► Peer                         │
│          ◄─── Signaling ────────                               │
│               (Exchange discovered addresses)                   │
│                                                                 │
│   Client ◄────── Direct P2P ──────► Peer                       │
│                                                                 │
└────────────────────────────────────────────────────────────────┘

Problems:
- Requires separate STUN infrastructure
- Multiple round trips before connection
- STUN binding can expire/change
- Requires out-of-band signaling channel
```

### QAD: Native Address Discovery in QUIC

QUIC Address Discovery eliminates the need for separate STUN servers by embedding address discovery directly into the QUIC handshake:

```
┌────────────────────────────────────────────────────────────────┐
│                    QAD Flow (Simplified)                        │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Agent ──── QUIC Initial ─────► Intermediate System           │
│                                        │                        │
│                                        ▼                        │
│                              Observe source IP:Port             │
│                              from UDP packet header             │
│                                        │                        │
│         ◄─── OBSERVED_ADDRESS ─────────┘                       │
│              (Your public IP: 203.0.113.5:54321)               │
│                                                                 │
│   Agent now knows its public address!                          │
│   (Discovered during normal QUIC handshake)                    │
│                                                                 │
└────────────────────────────────────────────────────────────────┘

Benefits:
- No separate STUN infrastructure needed
- Address discovered during connection setup (zero extra RTT)
- Always fresh (observed on every packet)
- Same connection used for data and signaling
```

### QAD Implementation Details

The Intermediate System implements QAD by:

1. **Observing Source Address:** When a UDP packet arrives, the server reads the source IP:Port from the UDP header — this is the client's public address after NAT translation.

2. **Reporting via QUIC Frame:** The observed address is sent back to the client using either:
   - A custom application-layer message on Stream 0
   - A QUIC DATAGRAM frame with a special type identifier
   - (Future) Native QUIC PATH_CHALLENGE/PATH_RESPONSE extensions

3. **Continuous Updates:** If the client's address changes (mobile network switch, NAT rebinding), the server observes the new address and notifies the client.

```rust
// Pseudocode: Server-side QAD
fn handle_incoming_packet(udp_packet: UdpPacket) {
    let observed_addr = udp_packet.source_address(); // Client's public IP:Port
    let quic_conn = get_or_create_connection(udp_packet);

    // Send OBSERVED_ADDRESS to client
    let qad_message = QadMessage::ObservedAddress {
        ip: observed_addr.ip(),
        port: observed_addr.port(),
    };
    quic_conn.send_datagram(qad_message.encode());
}
```

```swift
// Pseudocode: Client-side QAD handling
func handleQadMessage(_ message: QadMessage) {
    switch message {
    case .observedAddress(let ip, let port):
        self.publicAddress = "\(ip):\(port)"
        logger.info("Discovered public address: \(self.publicAddress)")
    }
}
```

---

## Service Registration Protocol

The Intermediate Server uses **service-based routing** to relay traffic between Agents and Connectors. Both must register to enable data flow.

### Why Registration is Required

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      WITHOUT REGISTRATION                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Agent sends data ──► Intermediate ──► "No destination for relay" ──► DROP  │
│                                                                              │
│  Problem: Intermediate doesn't know which Connector should receive           │
│           the Agent's traffic.                                               │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                      WITH REGISTRATION                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Agent registers: "I want to reach 'echo-service'"                       │
│  2. Connector registers: "I provide 'echo-service'"                         │
│  3. Agent sends data ──► Intermediate ──► Connector ──► Echo Server         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Registration Message Format

Registration is sent as a QUIC DATAGRAM immediately after connection establishment:

```
┌────────────────┬──────────────────┬─────────────────────┐
│ Type (1 byte)  │ Length (1 byte)  │ Service ID (N bytes)│
└────────────────┴──────────────────┴─────────────────────┘
```

| Type Byte | Client Type | Meaning |
|-----------|-------------|---------|
| `0x10` | Agent | "I want to reach service X" |
| `0x11` | Connector | "I provide service X" |

**Example: Register for "echo-service" (12 bytes)**
```
Agent:     [0x10] [0x0c] [echo-service]
Connector: [0x11] [0x0c] [echo-service]
```

### Registration Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COMPLETE REGISTRATION FLOW                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CONNECTOR STARTUP:                                                          │
│  ─────────────────                                                          │
│  1. Connector connects to Intermediate (QUIC handshake)                     │
│  2. Connector receives QAD observed address (0x01 message)                  │
│  3. Connector sends: DATAGRAM [0x11, 12, "echo-service"]                    │
│  4. Intermediate logs: "Registered Connector for 'echo-service'"            │
│                                                                              │
│  AGENT STARTUP:                                                              │
│  ─────────────                                                               │
│  1. Agent connects to Intermediate (QUIC handshake)                         │
│  2. Agent receives QAD observed address                                      │
│  3. Agent sends: DATAGRAM [0x10, 12, "echo-service"]                        │
│  4. Intermediate logs: "Registered Agent targeting 'echo-service'"          │
│                                                                              │
│  DATA FLOW (after registration):                                             │
│  ────────────────────────────────                                            │
│  5. Agent sends IP packet as DATAGRAM                                        │
│  6. Intermediate finds: Agent → target "echo-service" → Connector           │
│  7. Intermediate relays DATAGRAM to Connector                               │
│  8. Connector decapsulates, forwards to local echo server                   │
│  9. Echo server responds, Connector encapsulates response                   │
│  10. Intermediate relays response back to Agent                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Intermediate Server Registry

The Intermediate Server maintains two maps for routing:

```rust
struct Registry {
    // Connector registration: service_id → connection_id
    connectors: HashMap<String, ConnectionId>,

    // Agent registration: connection_id → target_service_id
    agent_targets: HashMap<ConnectionId, String>,
}

fn find_destination(from: ConnectionId) -> Option<ConnectionId> {
    // If sender is an Agent, find the Connector for their target service
    if let Some(service) = agent_targets.get(&from) {
        return connectors.get(service);
    }
    // If sender is a Connector, find Agents targeting their service
    // (reverse lookup for response traffic)
    ...
}
```

### FFI Implementation

**Rust (`core/packet_processor/src/lib.rs`):**
```rust
const REG_TYPE_AGENT: u8 = 0x10;

#[no_mangle]
pub extern "C" fn agent_register(
    agent: *mut Agent,
    service_id: *const c_char,
) -> AgentResult {
    // Send registration DATAGRAM: [0x10, len, service_id_bytes]
}
```

**Swift (`PacketTunnelProvider.swift`):**
```swift
private let targetServiceId = "echo-service"

private func registerForService() {
    let result = targetServiceId.withCString { servicePtr in
        agent_register(agent, servicePtr)
    }
    if result == AgentResultOk {
        logger.info("Registered for service '\(targetServiceId)'")
    }
}
```

### 0x2F Service-Routed Datagram Protocol

When an Agent is registered for multiple services, per-packet routing is needed. The Agent wraps each outgoing IP packet with a 0x2F header that identifies the target service:

```
┌────────────┬──────────────────┬─────────────────────┬─────────────────┐
│ 0x2F       │ ID Length (1B)   │ Service ID (N bytes)│ IP Packet       │
│ (1 byte)   │                  │                     │ (remaining)     │
└────────────┴──────────────────┴─────────────────────┴─────────────────┘
```

**Flow:**
1. Agent intercepts packet to 10.100.0.1
2. Route table lookup: 10.100.0.1 → "echo-service"
3. Agent wraps: `[0x2F, 12, "echo-service", ip_packet_bytes...]`
4. Intermediate reads 0x2F, finds Connector for "echo-service"
5. Intermediate strips 0x2F wrapper, forwards raw IP packet to Connector
6. Connector processes IP packet (UDP/TCP/ICMP)

**Backward Compatibility:** Non-0x2F datagrams still use implicit single-service routing.

### Registration Notes

1. **Service ID must match exactly** — Agent's target must match Connector's registered service
2. **No acknowledgment** — Registration is fire-and-forget; success assumed
3. **Connection-scoped** — Registration lost on disconnect; re-register on reconnect
4. **Multi-service** — Agent can register for multiple services per connection (0x2F routing)

---

## Data Flow

### Complete End-to-End Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COMPLETE END-TO-END DATA FLOW                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  macOS Agent                                                         │    │
│  │                                                                      │    │
│  │  1. User App (ping, curl, browser)                                   │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  2. NetworkExtension intercepts packet                               │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  3. Rust FFI: agent_send_datagram(ip_packet)                         │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  4. QUIC DATAGRAM → UDP socket                                       │    │
│  │                                                                      │    │
│  └──────────────────────────┬──────────────────────────────────────────┘    │
│                              │                                               │
│                              │ UDP over Internet/LAN                        │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Intermediate Server (k8s or Cloud)                                  │    │
│  │                                                                      │    │
│  │  5. Receive QUIC packet on UDP 4433                                  │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  6. Registry lookup: Agent conn_id → target "echo-service"          │    │
│  │         │                            → Connector conn_id             │    │
│  │         ▼                                                            │    │
│  │  7. Relay DATAGRAM to Connector's QUIC connection                   │    │
│  │                                                                      │    │
│  └──────────────────────────┬──────────────────────────────────────────┘    │
│                              │                                               │
│                              │ QUIC (internal network)                       │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  App Connector                                                       │    │
│  │                                                                      │    │
│  │  8. Receive DATAGRAM with IP packet                                  │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  9. Decapsulate: extract UDP payload from IP packet                  │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  10. Forward to local service (echo-server:9999)                    │    │
│  │                                                                      │    │
│  └──────────────────────────┬──────────────────────────────────────────┘    │
│                              │                                               │
│                              │ UDP to localhost                              │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Backend Service (Echo Server)                                       │    │
│  │                                                                      │    │
│  │  11. Process request, generate response                              │    │
│  │         │                                                            │    │
│  │         ▼                                                            │    │
│  │  12. Send UDP response → Connector                                   │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  RESPONSE PATH (12 → 1 in reverse):                                         │
│  Connector encapsulates → Intermediate relays → Agent injects to tun       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Protocol Messages

| Step | Message Type | Format |
|------|-------------|--------|
| Connection | QUIC handshake | TLS 1.3, ALPN "ztna-v1" |
| QAD | OBSERVED_ADDRESS | `[0x01, ip0, ip1, ip2, ip3, port_hi, port_lo]` |
| Registration | Agent | `[0x10, len, service_id...]` (can send multiple) |
| Registration | Connector | `[0x11, len, service_id...]` |
| Data (routed) | 0x2F service datagram | `[0x2F, len, service_id..., ip_packet...]` |
| Data (legacy) | Raw IP packet | QUIC DATAGRAM containing full IP packet |

### Supported Protocols at Connector

| IP Protocol | Proto # | Connector Behavior |
|------------|---------|-------------------|
| **UDP** | 17 | Extract payload → forward to backend → construct return IP/UDP |
| **TCP** | 6 | Userspace proxy: SYN→connect, ACK+data→write, FIN→close, RST→reset |
| **ICMP** | 1 | Echo Reply generated locally (swap src/dst IP, type 8→0) |
| Other | * | Dropped with trace log |

### Inbound Traffic (Application → User)

The reverse path follows the same tunnel, with responses encapsulated by the App Connector and delivered back to the Endpoint Agent, which injects them into the local network stack via `packetFlow.writePackets()`.

---

## Split-Tunnel Routing

The ZTNA Agent uses a **split-tunnel** model: only traffic destined for configured virtual service IPs flows through the QUIC tunnel. All other traffic uses the normal default gateway.

### How Split-Tunnel Works

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       SPLIT-TUNNEL ROUTING                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  macOS Kernel Routing Table (after VPN connect):                            │
│  ──────────────────────────────────────────────                             │
│  10.100.0.0/24  →  utun6 (ZTNA tunnel)     ← Only these go through QUIC   │
│  0.0.0.0/0      →  en0 (default gateway)    ← Everything else: normal      │
│                                                                              │
│  What gets tunneled:                    What does NOT get tunneled:          │
│  • ping 10.100.0.1 (echo-service)      • ping 8.8.8.8 (Google DNS)         │
│  • curl 10.100.0.2:8080 (web-app)      • curl example.com (web browsing)   │
│  • ssh 10.100.0.3 (future service)     • DNS queries to 8.8.8.8            │
│                                          • All other internet traffic        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Configuration-Driven Service Definition

Services are defined in JSON configuration files. The configuration flows through the system:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    CONFIGURATION → REGISTRATION → ROUTING                     │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  1. CONFIGURATION (JSON files define what gets tunneled)                      │
│                                                                               │
│  agent.json:                    connector.json:                               │
│  ┌───────────────────────┐     ┌───────────────────────────┐                 │
│  │ services:              │     │ services:                  │                 │
│  │ - id: echo-service     │     │ - id: echo-service         │                 │
│  │   virtualIp: 10.100.0.1│     │   backend: 127.0.0.1:9999 │                 │
│  │ - id: web-app          │     │   protocol: udp            │                 │
│  │   virtualIp: 10.100.0.2│     │ - id: web-app              │                 │
│  └───────────────────────┘     │   backend: 127.0.0.1:8080  │                 │
│                                 │   protocol: tcp             │                 │
│                                 └───────────────────────────┘                 │
│                                                                               │
│  2. REGISTRATION (tell Intermediate who provides/consumes what)               │
│                                                                               │
│  Agent → Intermediate:   [0x10, 12, "echo-service"]                          │
│  Agent → Intermediate:   [0x10, 7, "web-app"]                                │
│  Connector → Intermediate: [0x11, 12, "echo-service"]                        │
│                                                                               │
│  Intermediate registry:                                                       │
│    agent_targets: { agent_conn → {"echo-service", "web-app"} }               │
│    connectors:    { "echo-service" → connector_conn }                        │
│                                                                               │
│  3. ROUTING (per-packet service-routed datagrams)                             │
│                                                                               │
│  User runs: ping 10.100.0.1                                                  │
│    → macOS routes to utun6 (matches 10.100.0.0/24)                           │
│    → PacketTunnelProvider captures ICMP packet                                │
│    → Route table lookup: 10.100.0.1 → "echo-service"                        │
│    → Wrap: [0x2F, 12, "echo-service", ip_packet...]                          │
│    → QUIC DATAGRAM to Intermediate                                            │
│    → Intermediate: read 0x2F → find Connector for "echo-service"             │
│    → Strip wrapper → forward raw IP to Connector                             │
│    → Connector: parse IP → protocol 1 (ICMP) → build Echo Reply             │
│    → Send reply back through tunnel                                           │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Configuration Files

| Component | Config Path | Key Fields |
|-----------|------------|------------|
| Agent (macOS) | UI → providerConfiguration | `serverHost`, `serverPort`, `serviceId`, `services[]` |
| App Connector | `--config` or `/etc/ztna/connector.json` | `intermediate_server`, `services[]`, `p2p` |
| Intermediate | `--config` or `/etc/ztna/intermediate.json` | `port`, `bind_addr`, `external_ip`, certs |

Reference configs: `deploy/config/{agent,connector,intermediate}.json`

---

## Security Model

### Zero Trust Principles

1. **Never Trust, Always Verify:** Every connection is authenticated, regardless of network location.

2. **Least Privilege Access:** Users only access specific applications they're authorized for, not entire networks.

3. **Assume Breach:** Traffic is encrypted end-to-end; the Intermediate System cannot read payload contents.

### Authentication Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Authentication Flow                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Agent authenticates user (SSO, MFA, device cert)            │
│                        │                                         │
│                        ▼                                         │
│  2. Agent obtains short-lived token from Identity Provider       │
│                        │                                         │
│                        ▼                                         │
│  3. Token presented during QUIC handshake (ALPN or early data)  │
│                        │                                         │
│                        ▼                                         │
│  4. Intermediate System validates token, authorizes access       │
│                        │                                         │
│                        ▼                                         │
│  5. Connection established with authorized application set       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Encryption Layers

| Layer | Protection |
|-------|------------|
| QUIC TLS 1.3 | Agent ↔ Intermediate System |
| QUIC TLS 1.3 | Intermediate System ↔ App Connector |
| (Optional) mTLS | End-to-end application layer |

---

## P2P Hole Punching ✅ IMPLEMENTED

Direct P2P connectivity is now implemented via NAT hole punching. This allows Agents and Connectors to establish direct QUIC connections, bypassing the Intermediate System for data transfer.

### P2P Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         P2P HOLE PUNCHING FLOW                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PHASE 1: CANDIDATE GATHERING                                               │
│  ─────────────────────────────                                               │
│  • Host candidates: Local interface addresses (127.0.0.1, 192.168.1.x)     │
│  • Reflexive candidates: Public address from QAD (203.0.113.5:54321)       │
│  • Relay candidates: Intermediate server address (fallback)                 │
│                                                                              │
│  PHASE 2: SIGNALING (via Intermediate)                                       │
│  ─────────────────────────────────────                                       │
│  • Agent sends CandidateOffer with gathered candidates                      │
│  • Intermediate relays to Connector                                          │
│  • Connector responds with CandidateAnswer                                  │
│  • Intermediate sends StartPunching to both                                 │
│                                                                              │
│  PHASE 3: CONNECTIVITY CHECKS                                                │
│  ───────────────────────────                                                 │
│  • Both sides form candidate pairs (local × remote)                         │
│  • Pairs sorted by priority (RFC 8445)                                      │
│  • BindingRequest/Response exchange validates paths                         │
│  • First successful pair is nominated                                       │
│                                                                              │
│  PHASE 4: DIRECT CONNECTION                                                  │
│  ─────────────────────────                                                   │
│  • New QUIC connection established on direct path                           │
│  • Data flows: Agent ◄───────────► Connector (direct)                       │
│  • Intermediate drops out of data path (signaling only)                     │
│                                                                              │
│  FALLBACK: RELAY MODE                                                        │
│  ────────────────────                                                        │
│  • If all connectivity checks fail within 5 seconds                         │
│  • Continue using relay path through Intermediate                           │
│  • Automatic retry after 30 second cooldown                                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### P2P Implementation Details

**Location:** `core/packet_processor/src/p2p/`

| Module | Purpose | Tests |
|--------|---------|-------|
| `candidate.rs` | ICE candidate types, gathering, RFC 8445 priority | 11 |
| `signaling.rs` | CandidateOffer/Answer, StartPunching messages | 13 |
| `connectivity.rs` | BindingRequest/Response, CandidatePair, CheckList | 17 |
| `hole_punch.rs` | HolePunchCoordinator state machine, path selection | 17 |
| `resilience.rs` | PathManager, keepalive, automatic fallback | 12 |

**Key Design Decisions:**

1. **P2P = New QUIC Connection** (not path migration)
   - Direct P2P creates a separate QUIC connection to Connector
   - Agent manages multiple connections (Intermediate + P2P)
   - Path migration is a different concept (same connection, different route)

2. **Single Socket Reuse**
   - Agent: Swift NetworkExtension manages single socket
   - Connector: Dual-mode QUIC (client to Intermediate + server for Agents)
   - Same local port ensures NAT mapping consistency

3. **RFC 8445 Compliant Priority**
   - Priority formula: `(type_pref << 24) | (local_pref << 8) | (256 - component_id)`
   - Pair priority: `2^32*MIN(G,D) + 2*MAX(G,D) + (G>D?1:0)`

4. **Resilience**
   - Keepalive: 15 second interval, 3 missed = path failure
   - Automatic fallback to relay on path failure
   - 30 second cooldown before retry

### Connector Dual-Mode Architecture

The App Connector operates in dual-mode QUIC:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CONNECTOR DUAL-MODE QUIC                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  SINGLE UDP SOCKET (quic_socket)                                            │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  ┌──────────────────────┐       ┌──────────────────────┐          │    │
│  │  │ QUIC CLIENT          │       │ QUIC SERVER          │          │    │
│  │  │                      │       │                      │          │    │
│  │  │ • Connect to         │       │ • Accept connections │          │    │
│  │  │   Intermediate       │       │   from Agents        │          │    │
│  │  │ • Signaling          │       │ • Direct data path   │          │    │
│  │  │ • Relay fallback     │       │ • TLS cert required  │          │    │
│  │  └──────────┬───────────┘       └──────────┬───────────┘          │    │
│  │             │                              │                       │    │
│  │             └──────────┬───────────────────┘                       │    │
│  │                        │                                           │    │
│  │  Packet Routing:       ▼                                           │    │
│  │  • Check source address                                            │    │
│  │  • If from Intermediate → client connection                        │    │
│  │  • If QUIC Initial packet → accept new P2P connection              │    │
│  │  • If known P2P client → route to that connection                  │    │
│  │                                                                     │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**CLI Options for P2P:**
```bash
./app-connector \
  --server intermediate.example.com:4433 \
  --service my-service \
  --forward 127.0.0.1:8080 \
  --p2p-cert certs/connector-cert.pem \
  --p2p-key certs/connector-key.pem
```

### Local Testing Limitations

P2P hole punching is designed for NAT traversal, which requires real network address translation. Local testing can verify:

| Feature | Testable Locally? | Notes |
|---------|-------------------|-------|
| Host candidate gathering | ✅ Yes | Enumerates local interfaces |
| Signaling protocol | ✅ Yes | Message encode/decode via unit tests |
| Binding request/response | ✅ Yes | Protocol verification |
| Connectivity checks | ✅ Yes | Localhost connections |
| Keepalive mechanism | ✅ Yes | Timer-based verification |
| Fallback logic | ✅ Yes | Simulated path failure |
| **NAT hole punching** | ❌ No | Requires real NAT (cloud deployment) |
| **Reflexive address accuracy** | ❌ No | QAD returns 127.0.0.1 locally |
| **Symmetric NAT handling** | ❌ No | Requires real NAT scenarios |

**Full NAT testing requires cloud deployment (Task 006).**

### NAT Compatibility

P2P hole punching success depends on NAT type:

| NAT Type | Direct P2P | Notes |
|----------|------------|-------|
| **Full Cone** | ✅ Works | Any external host can send to mapped port |
| **Address-Restricted Cone** | ✅ Works | Same IP must be contacted first |
| **Port-Restricted Cone** | ✅ Works | Same IP:port must be contacted first |
| **Symmetric NAT** | ⚠️ Limited | Different mapping per destination - relay recommended |

**Symmetric NAT Handling:**
- Port prediction is unreliable
- Implementation automatically falls back to relay
- No performance penalty (relay is always available)

### P2P Troubleshooting

| Symptom | Possible Cause | Solution |
|---------|----------------|----------|
| All connectivity checks fail | Symmetric NAT | Use relay (automatic fallback) |
| Reflexive address = 127.0.0.1 | Testing locally | Deploy to cloud for NAT testing |
| No candidates gathered | No network interfaces | Check network connectivity |
| Signaling timeout | Intermediate unreachable | Check Intermediate connection |
| Keepalive failures | Path became invalid | Automatic fallback to relay |
| Frequent path switches | Unstable network | Check network quality |

**Logging:**
```bash
# Enable P2P debug logging
RUST_LOG=ztna_agent::p2p=debug ./app-connector ...

# Key log messages to look for:
# - "P2P server mode enabled" - Connector accepting connections
# - "Gathered X candidates" - Candidate gathering success
# - "Direct path established" - P2P success
# - "Falling back to relay" - P2P failed, using relay
```

---

## Deployment Architecture

### Current Development Setup (Pi k8s + macOS)

The current working deployment uses a Raspberry Pi Kubernetes cluster with Cilium L2 LoadBalancer:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CURRENT DEPLOYMENT (Home Lab)                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   macOS Workstation (10.0.150.x)                                            │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │  ZtnaAgent.app                                                   │       │
│   │  ├── ContentView.swift (UI)                                      │       │
│   │  └── Extension/PacketTunnelProvider.swift (VPN)                  │       │
│   │       └── Rust FFI (libpacket_processor.a)                       │       │
│   └──────────────────────────┬──────────────────────────────────────┘       │
│                               │                                              │
│                               │ QUIC/UDP                                     │
│                               ▼                                              │
│   Pi Kubernetes Cluster (10.0.150.101-108)                                  │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │  Cilium L2 LoadBalancer: 10.0.150.205:4433/UDP                  │       │
│   │                               │                                  │       │
│   │  ┌────────────────────────────┼────────────────────────────┐    │       │
│   │  │ ztna namespace             │                             │    │       │
│   │  │                            ▼                             │    │       │
│   │  │  ┌───────────────────────────────────────────────────┐  │    │       │
│   │  │  │  intermediate-server (Deployment)                  │  │    │       │
│   │  │  │  - hyeomans/ztna-intermediate-server:latest       │  │    │       │
│   │  │  │  - QUIC server on 4433                             │  │    │       │
│   │  │  │  - QAD + DATAGRAM relay                            │  │    │       │
│   │  │  └───────────────────────────────────────────────────┘  │    │       │
│   │  │                            │                             │    │       │
│   │  │                            │ ClusterIP                   │    │       │
│   │  │                            ▼                             │    │       │
│   │  │  ┌───────────────────────────────────────────────────┐  │    │       │
│   │  │  │  app-connector (Deployment)                        │  │    │       │
│   │  │  │  - hyeomans/ztna-app-connector:latest             │  │    │       │
│   │  │  │  - --service echo-service                          │  │    │       │
│   │  │  │  - --forward echo-server:9999                      │  │    │       │
│   │  │  └───────────────────────────────────────────────────┘  │    │       │
│   │  │                            │                             │    │       │
│   │  │                            │ ClusterIP                   │    │       │
│   │  │                            ▼                             │    │       │
│   │  │  ┌───────────────────────────────────────────────────┐  │    │       │
│   │  │  │  echo-server (Deployment)                          │  │    │       │
│   │  │  │  - hyeomans/ztna-echo-server:latest               │  │    │       │
│   │  │  │  - UDP echo on port 9999                           │  │    │       │
│   │  │  └───────────────────────────────────────────────────┘  │    │       │
│   │  │                                                          │    │       │
│   │  └──────────────────────────────────────────────────────────┘    │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
│  Key Configuration:                                                          │
│  - LoadBalancer: externalTrafficPolicy: Cluster (required for Cilium L2)    │
│  - TLS: Self-signed certs mounted via k8s Secret                            │
│  - Images: Multi-arch (arm64) on Docker Hub                                 │
│  - SNAT: macOS appears as k8s node IP to intermediate (externalTrafficPolicy: Cluster)  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Deployment Commands:**
```bash
# Apply manifests
kubectl --context k8s1 apply -k deploy/k8s/overlays/pi-home

# Check status
kubectl --context k8s1 get pods -n ztna

# View logs
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f
```

### Small Scale (Single Region)

```
                    ┌─────────────────────┐
                    │  Intermediate       │
                    │  System             │
                    │  (1 instance)       │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
              ▼                ▼                ▼
        ┌──────────┐    ┌──────────┐    ┌──────────┐
        │  Agent   │    │  Agent   │    │ Connector│
        │ (macOS)  │    │ (macOS)  │    │  (k8s)   │
        └──────────┘    └──────────┘    └──────────┘
```

### Large Scale (Multi-Region)

```
                         ┌─────────────────┐
                         │  Global Load    │
                         │  Balancer       │
                         └────────┬────────┘
                                  │
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
           ▼                      ▼                      ▼
    ┌─────────────┐        ┌─────────────┐        ┌─────────────┐
    │ Intermediate│        │ Intermediate│        │ Intermediate│
    │ US-East     │        │ EU-West     │        │ AP-South    │
    └──────┬──────┘        └──────┬──────┘        └──────┬──────┘
           │                      │                      │
           │         ┌────────────┴────────────┐         │
           │         │    Redis/etcd Cluster   │         │
           │         │    (Session State)      │         │
           │         └─────────────────────────┘         │
           │                                             │
    ┌──────┴──────┐                              ┌───────┴──────┐
    │ Connectors  │                              │  Connectors  │
    │ US Region   │                              │  AP Region   │
    └─────────────┘                              └──────────────┘
```

---

## Technology Choices

| Component | Technology | Rationale |
|-----------|------------|-----------|
| QUIC Library | `quiche` (Cloudflare) | Sans-IO design, Rust, battle-tested |
| Packet Processing | Rust + `etherparse` | Performance, memory safety, FFI |
| macOS Agent | Swift 6.2 + NetworkExtension | Native platform integration |
| Intermediate Server | Rust + `mio` | Matches quiche's sans-IO model |
| App Connector | Rust + `mio` | Lightweight, matches server |
| Container Runtime | Docker / Kubernetes | Standard deployment |
| Build System | Cargo + Kustomize | Rust builds, k8s overlays |

---

## References

- [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000)
- [QUIC Datagram RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221)
- [ICE RFC 8445](https://datatracker.ietf.org/doc/html/rfc8445) - NAT traversal / candidate priority
- [quiche Library](https://github.com/cloudflare/quiche)
- [Apple NetworkExtension](https://developer.apple.com/documentation/networkextension)
- [Zero Trust Architecture (NIST SP 800-207)](https://csrc.nist.gov/publications/detail/sp/800-207/final)

### Internal Documentation

- `tasks/_context/README.md` - Project overview and session resume instructions
- `tasks/_context/components.md` - Component status overview with Service Registration Protocol
- `tasks/_context/testing-guide.md` - Testing commands and E2E verification
- `tasks/006-cloud-deployment/` - Current task: cloud deployment and NAT testing
- `deploy/k8s/k8s-deploy-skill.md` - Kubernetes deployment guide
- `tests/e2e/README.md` - E2E test framework architecture
