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
config.set_application_protos(&[b"ztna"])?;
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

### Proposed Format

```
+--------+--------+--------+--------+--------+--------+--------+
| Type   | IP Version | IP Address (4 or 16 bytes) | Port    |
| (1)    | (1)        | (variable)                 | (2)     |
+--------+--------+--------+--------+--------+--------+--------+

Type: 0x01 = OBSERVED_ADDRESS
IP Version: 0x04 = IPv4, 0x06 = IPv6
IP Address: 4 bytes (IPv4) or 16 bytes (IPv6)
Port: 2 bytes, network byte order (big-endian)
```

### Example (IPv4)

Client observed at 203.0.113.5:54321:
```
01 04 CB 00 71 05 D4 31
│  │  └──────────┘ └────┘
│  │       │         │
│  │       │         └─ Port: 54321 (0xD431)
│  │       └─ IP: 203.0.113.5
│  └─ IPv4
└─ OBSERVED_ADDRESS type
```

### Delivery Method

Options:
1. **DATAGRAM frame** - unreliable but simple
2. **Stream 0** - reliable, requires stream setup
3. **Custom frame** - would require protocol negotiation

**Decision:** Use DATAGRAM for simplicity. Re-send periodically if needed.

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

## Async Runtime: tokio vs mio

### Option 1: mio (low-level)
- Used in quiche examples
- Manual event loop
- More control, more boilerplate

### Option 2: tokio (high-level)
- Higher-level async/await
- Built-in timers, channels
- Easier to extend later

**Decision:** Start with mio (closer to quiche examples), consider tokio migration later.

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
