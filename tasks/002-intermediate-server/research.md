# Research: Intermediate Server

**Task ID:** 002-intermediate-server

---

## Purpose

Document research findings, design decisions, and reference materials for the Intermediate Server implementation.

---

## quiche Server Implementation

### Reference: quiche Examples

The quiche repository has server examples that demonstrate:
- UDP socket setup with mio
- Connection management with HashMap
- Packet routing by connection ID

**Key file:** `quiche/examples/server.rs`

### quiche Server Configuration

```rust
let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

// TLS setup
config.load_cert_chain_from_pem_file("cert.pem")?;
config.load_priv_key_from_pem_file("key.pem")?;

// QUIC settings
// CRITICAL: Must match Agent ALPN at core/packet_processor/src/lib.rs:28
config.set_application_protos(&[b"ztna-v1"])?;
config.set_max_idle_timeout(30_000); // 30 seconds
config.set_max_recv_udp_payload_size(1350);
config.set_max_send_udp_payload_size(1350);
config.set_initial_max_data(10_000_000);
config.set_initial_max_stream_data_bidi_local(1_000_000);
config.set_initial_max_stream_data_bidi_remote(1_000_000);
config.set_initial_max_streams_bidi(100);

// Enable DATAGRAM
config.enable_dgram(true, 1000, 1000);
```

---

## QAD Message Format

### Format (Must Match Agent Parser)

**CRITICAL:** The Agent at `core/packet_processor/src/lib.rs:255-262` parses QAD messages
without an IP version byte. The server MUST use this exact format:

```
+--------+--------+--------+--------+--------+--------+--------+
| Type   | IPv4 Address (4 bytes)            | Port (2 bytes)  |
| (1)    |                                   | (big-endian)    |
+--------+--------+--------+--------+--------+--------+--------+

Type: 0x01 = OBSERVED_ADDRESS
IP Address: 4 bytes (IPv4 only for now)
Port: 2 bytes, network byte order (big-endian)

Total: 7 bytes
```

### Example (IPv4)

Client observed at 203.0.113.5:54321:
```
01 CB 00 71 05 D4 31
│  └──────────┘ └────┘
│       │         │
│       │         └─ Port: 54321 (0xD431)
│       └─ IP: 203.0.113.5
└─ OBSERVED_ADDRESS type (0x01)
```

### Future: IPv6 Support

IPv6 support would require updating the Agent parser to accept a version byte.
This is deferred to a future task.

### Delivery Method

**Decision:** Use DATAGRAM (required by Agent implementation).

The Agent at `core/packet_processor/src/lib.rs:251` only processes QAD messages
received via `conn.dgram_recv()`. Stream-based delivery would not be parsed.

Re-send on NAT rebinding (address change detection).

---

## Client Registry Design

### Data Structures

```rust
struct Client {
    conn: quiche::Connection,
    client_type: ClientType,
    observed_addr: SocketAddr,
    destination_id: Option<String>,  // For Agents: which Connector to reach
    last_activity: Instant,
}

enum ClientType {
    Agent,
    Connector { service_id: String },
}

struct Registry {
    clients: HashMap<ConnectionId, Client>,
    connectors: HashMap<String, ConnectionId>,  // service_id → conn_id
}
```

### Routing Logic

1. Agent connects, specifies `destination_id` (service it wants to reach)
2. Connector connects, registers `service_id`
3. When Agent sends DATAGRAM:
   - Look up Connector by `destination_id`
   - Forward DATAGRAM to Connector
4. When Connector sends DATAGRAM:
   - Reverse lookup Agent
   - Forward DATAGRAM to Agent

---

## Async Runtime: mio

**Decision:** Use mio (matches quiche examples and sans-IO model).

### Rationale
- quiche examples use mio directly
- Sans-IO model means quiche doesn't own the socket
- mio provides the event loop without async/await complexity
- Easier to understand connection ID routing
- Can migrate to tokio later if needed for async features

### Key mio Concepts
```rust
use mio::{Events, Poll, Token};
use mio::net::UdpSocket;

let mut poll = Poll::new()?;
let mut socket = UdpSocket::bind(addr)?;
poll.registry().register(&mut socket, Token(0), Interest::READABLE)?;

let mut events = Events::with_capacity(1024);
loop {
    poll.poll(&mut events, timeout)?;
    for event in events.iter() {
        // Handle socket readable/writable
    }
}
```

---

## Certificate Generation

### Development Self-Signed Cert

```bash
# Generate private key
openssl genrsa -out key.pem 2048

# Generate self-signed certificate
openssl req -new -x509 -key key.pem -out cert.pem -days 365 \
  -subj "/CN=localhost"
```

### For Production

- Use Let's Encrypt or proper CA
- Certificate must match server hostname
- Consider mTLS for client authentication

---

## References

### quiche Documentation
- [quiche GitHub](https://github.com/cloudflare/quiche)
- [quiche docs.rs](https://docs.rs/quiche)

### QUIC RFCs
- [RFC 9000 - QUIC Transport](https://datatracker.ietf.org/doc/html/rfc9000)
- [RFC 9001 - QUIC TLS](https://datatracker.ietf.org/doc/html/rfc9001)
- [RFC 9221 - QUIC DATAGRAM](https://datatracker.ietf.org/doc/html/rfc9221)

### Similar Projects
- [quinn](https://github.com/quinn-rs/quinn) - Alternative Rust QUIC library
- [s2n-quic](https://github.com/aws/s2n-quic) - AWS QUIC library

---

## Open Questions

1. **Authentication:** How do Agents/Connectors prove identity?
   - MVP: None (localhost only)
   - Future: Token-based or mTLS

2. **Multiple Connectors:** How to handle multiple Connectors for same service?
   - MVP: Single Connector per service
   - Future: Load balancing

3. **Connection Migration:** Support QUIC connection migration?
   - MVP: No
   - Future: Yes (needed for P2P)
