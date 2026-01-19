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
│  1. Agent connects to Intermediate System                                    │
│  2. App Connector connects to Intermediate System                            │
│  3. Both learn their public IP:Port via QAD                                  │
│  4. Intermediate facilitates initial routing (relay mode)                    │
│  5. Data flows: Agent ↔ Intermediate ↔ Connector                            │
│                                                                              │
│  PHASE 2: HOLE PUNCH ATTEMPT (parallel to data flow)                         │
│  ───────────────────────────────────────────────────                         │
│  1. Intermediate shares peer addresses with both sides                       │
│  2. Agent sends QUIC packets directly to Connector's public IP               │
│  3. Connector sends QUIC packets directly to Agent's public IP               │
│  4. If NAT bindings align → direct path opens                                │
│                                                                              │
│  PHASE 3: CONNECTION MIGRATION (on successful hole punch)                    │
│  ────────────────────────────────────────────────────────                    │
│  1. QUIC connection migrates from relay path to direct path                  │
│  2. Intermediate drops out of data path (signaling only)                     │
│  3. Data flows: Agent ↔ Connector (direct, optimal latency)                  │
│                                                                              │
│  FALLBACK: CONTINUE RELAY (if hole punch fails)                              │
│  ──────────────────────────────────────────────                              │
│  • Strict NAT/firewall prevents direct connection                            │
│  • Continue using relay path indefinitely                                    │
│  • Periodically retry hole punch on network changes                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Implementation Phases

| Phase | Milestone | Connection Mode |
|-------|-----------|-----------------|
| **Phase 2** ✅ | Agent UDP integration | N/A (no server yet) |
| **Phase 3** | Intermediate System relay | Relay only |
| **Phase 4** | App Connector | Relay only |
| **Phase 5** | End-to-end testing | Relay only |
| **Phase 6** | **Hole punching + migration** | **Direct preferred, relay fallback** |

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
- Establish QUIC connection to Intermediate System
- Encapsulate intercepted packets in QUIC DATAGRAM frames
- Learn public IP:Port via QAD (no STUN required)
- Handle connection migration on network changes

**Technology Stack:**
- Swift 6.2 / SwiftUI (host app)
- Rust (packet processing, QUIC via `quiche`)
- NetworkExtension framework

**Key Files:**
```
ios-macos/ZtnaAgent/
├── ZtnaAgent/ContentView.swift      # Host app UI
├── Extension/PacketTunnelProvider.swift  # Packet interception
core/packet_processor/
└── src/lib.rs                       # Rust FFI for packet processing
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
- Register as endpoint for specific services/applications
- Receive encapsulated IP packets via DATAGRAM frames
- Decapsulate and forward to local application (TCP/UDP)
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

## Data Flow

### Outbound Traffic (User → Application)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Outbound Packet Flow                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. User App        2. Network Extension    3. Rust Processor               │
│  ┌─────────┐        ┌──────────────────┐    ┌─────────────────┐             │
│  │ Browser │──TCP──►│ NEPacketTunnel   │───►│ process_packet()│             │
│  │ curl    │        │ Provider         │    │ (FFI)           │             │
│  └─────────┘        │                  │    └────────┬────────┘             │
│                     │  packetFlow      │             │                       │
│                     │  .readPackets()  │             │ Forward/Drop         │
│                     └──────────────────┘             ▼                       │
│                                                                              │
│  4. QUIC Encapsulation              5. Intermediate System                  │
│  ┌─────────────────────┐            ┌─────────────────────┐                 │
│  │ IP Packet           │            │                     │                 │
│  │ ┌─────────────────┐ │   QUIC     │  Route DATAGRAM     │                 │
│  │ │ TCP/UDP Payload │ │──DATAGRAM─►│  to App Connector   │                 │
│  │ └─────────────────┘ │            │                     │                 │
│  └─────────────────────┘            └──────────┬──────────┘                 │
│                                                 │                            │
│  6. App Connector                    7. Private Application                 │
│  ┌─────────────────────┐            ┌─────────────────────┐                 │
│  │ Decapsulate         │            │                     │                 │
│  │ IP Packet           │───TCP/UDP─►│  Web Server         │                 │
│  │                     │            │  Database           │                 │
│  │ Forward to local    │            │  API Service        │                 │
│  └─────────────────────┘            └─────────────────────┘                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Inbound Traffic (Application → User)

The reverse path follows the same tunnel, with responses encapsulated by the App Connector and delivered back to the Endpoint Agent, which injects them into the local network stack.

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

## P2P Optimization (Future)

When network conditions allow, Agents and Connectors can establish direct P2P connections, bypassing the Intermediate System for data transfer:

```
┌─────────────────────────────────────────────────────────────────┐
│                    P2P Hole Punching                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Agent connects to Intermediate, learns public address (QAD) │
│  2. Connector connects to Intermediate, learns public address   │
│  3. Intermediate exchanges addresses between Agent & Connector  │
│  4. Both send UDP packets to each other's public address        │
│  5. NAT bindings created, direct path established               │
│  6. QUIC connection migrated to direct path                     │
│                                                                  │
│     Agent ◄──────────── Direct QUIC ────────────► Connector     │
│              (Intermediate only used for signaling)             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Deployment Architecture

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
| Intermediate Server | Rust + `tokio` | Async I/O, performance |
| App Connector | Rust | Lightweight, containerizable |
| Container Runtime | Docker / Kubernetes | Standard deployment |

---

## References

- [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000)
- [QUIC Datagram RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221)
- [quiche Library](https://github.com/cloudflare/quiche)
- [Apple NetworkExtension](https://developer.apple.com/documentation/networkextension)
- [Zero Trust Architecture (NIST SP 800-207)](https://csrc.nist.gov/publications/detail/sp/800-207/final)
