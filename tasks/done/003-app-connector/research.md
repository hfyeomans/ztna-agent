# Research: App Connector

**Task ID:** 003-app-connector

---

## Purpose

Document research findings, design decisions, and reference materials for the App Connector implementation.

---

## Forwarding Strategies

### Option 1: User-space Socket Forwarding

**How it works:**
- Parse IP packet, extract TCP/UDP payload
- Create socket connection to local service
- Forward payload, receive response
- Re-encapsulate response

**Pros:**
- No special permissions needed
- Cross-platform
- Simple implementation

**Cons:**
- Loses original source IP (NAT-like)
- Connection state management for TCP

### Option 2: TUN Device Injection

**How it works:**
- Create TUN device
- Inject decapsulated IP packets directly
- Kernel handles routing/forwarding

**Pros:**
- Preserves original source IP
- Handles all protocols transparently

**Cons:**
- Requires root/admin
- Platform-specific TUN APIs
- More complex setup

### Decision

**MVP: User-space socket forwarding**
- Simpler, no permissions needed
- Sufficient for most use cases

**Future: TUN injection (optional)**
- For cases requiring original IP preservation

---

## IP Packet Decapsulation

### Parsing with etherparse

```rust
use etherparse::{SlicedPacket, InternetSlice, TransportSlice};

fn decapsulate(data: &[u8]) -> Result<ForwardTarget, Error> {
    let packet = SlicedPacket::from_ip(data)?;

    match packet.ip {
        Some(InternetSlice::Ipv4(ipv4, _)) => {
            let dst_ip = ipv4.destination_addr();
            let src_ip = ipv4.source_addr();

            match packet.transport {
                Some(TransportSlice::Tcp(tcp)) => {
                    ForwardTarget::Tcp {
                        src: (src_ip, tcp.source_port()),
                        dst: (dst_ip, tcp.destination_port()),
                        payload: packet.payload,
                    }
                }
                Some(TransportSlice::Udp(udp)) => {
                    ForwardTarget::Udp {
                        src: (src_ip, udp.source_port()),
                        dst: (dst_ip, udp.destination_port()),
                        payload: packet.payload,
                    }
                }
                _ => ForwardTarget::Other
            }
        }
        _ => ForwardTarget::Other
    }
}
```

---

## TCP Connection Handling

### Challenge: Stateful Protocol

TCP requires maintaining connection state:
- Connection establishment (SYN/SYN-ACK/ACK)
- Sequence numbers
- Window management
- Connection termination

### Approach 1: TCP Proxy

- Maintain local TCP connections per remote connection
- Map packets to connections by (src_ip, src_port, dst_ip, dst_port)
- Forward data bidirectionally

### Approach 2: Transparent Proxy

- Use TCP splice/splicing
- More efficient but platform-specific

### Decision

**MVP: Simple TCP proxy with connection table**

```rust
struct TcpConnectionTable {
    connections: HashMap<FlowKey, TcpStream>,
}

struct FlowKey {
    src_ip: IpAddr,
    src_port: u16,
    dst_ip: IpAddr,
    dst_port: u16,
}
```

---

## UDP Forwarding

### Simpler Case

UDP is stateless - just forward and await response.

```rust
async fn forward_udp(
    local_addr: SocketAddr,
    payload: &[u8],
) -> io::Result<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.send_to(payload, local_addr).await?;

    let mut buf = vec![0u8; 1500];
    let (len, _) = socket.recv_from(&mut buf).await?;
    buf.truncate(len);
    Ok(buf)
}
```

### Response Handling

- Need to track pending requests to match responses
- Timeout after reasonable period
- Clean up stale state

---

## Registration Protocol

### Message Format

```
+--------+--------+--------+--------+
| Type   | Len    | Service ID      |
| (1)    | (2)    | (variable)      |
+--------+--------+--------+--------+

Type: 0x02 = REGISTER_CONNECTOR
Len: Length of service ID (2 bytes, big-endian)
Service ID: UTF-8 string identifying the service
```

### Example

Register as "web-app" service:
```
02 00 07 77 65 62 2D 61 70 70
│  └────┘ └─────────────────┘
│    │           │
│    │           └─ "web-app" (7 bytes)
│    └─ Length: 7
└─ REGISTER_CONNECTOR type
```

---

## References

### Rust Libraries
- [etherparse](https://docs.rs/etherparse) - IP packet parsing
- [socket2](https://docs.rs/socket2) - Low-level socket control
- [tokio](https://docs.rs/tokio) - Async runtime

### quiche Client
- [quiche client example](https://github.com/cloudflare/quiche/blob/master/quiche/examples/client.rs)

### TCP Proxying
- [tokio-proxy patterns](https://github.com/tokio-rs/tokio/blob/master/examples/proxy.rs)

---

## Open Questions

1. **Port mapping:** How to handle port translation?
   - MVP: Forward to single configured port
   - Future: Dynamic port mapping

2. **Multiple services:** One Connector per service or multi-service?
   - MVP: Single service per Connector
   - Future: Multiple services via config

3. **Connection limits:** Max concurrent connections?
   - MVP: Unlimited (rely on OS limits)
   - Future: Configurable limits
