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
- [ICE RFC 8445](https://datatracker.ietf.org/doc/html/rfc8445) - NAT traversal / candidate priority
- [quiche Library](https://github.com/cloudflare/quiche)
- [Apple NetworkExtension](https://developer.apple.com/documentation/networkextension)
- [Zero Trust Architecture (NIST SP 800-207)](https://csrc.nist.gov/publications/detail/sp/800-207/final)

### Internal Documentation

- `tasks/005-p2p-hole-punching/plan.md` - Detailed P2P implementation plan
- `tasks/005-p2p-hole-punching/state.md` - P2P task status and progress
- `tasks/_context/components.md` - Component status overview
