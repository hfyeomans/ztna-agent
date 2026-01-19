# Research: P2P Hole Punching

**Task ID:** 005-p2p-hole-punching

---

## Purpose

Document research findings on NAT traversal, hole punching techniques, and QUIC connection migration for implementing direct P2P connectivity.

---

## NAT Types and Behavior

### Full Cone NAT

**Behavior:**
- Once internal host sends to external, ANY external can send back
- Easiest for hole punching

**Detection:**
- QAD returns same external IP:port regardless of destination

```
Internal: 192.168.1.10:50000
External: 203.0.113.50:40000  (fixed mapping)

Any external host can send to 203.0.113.50:40000
```

### Restricted Cone NAT

**Behavior:**
- External host must have received packet first
- IP-based restriction

**Detection:**
- Different destination IPs see same external port

```
To Server A: 203.0.113.50:40000
To Server B: 203.0.113.50:40000  (same port)

Only hosts that received packets can respond
```

### Port Restricted Cone NAT

**Behavior:**
- External host:port must have received packet first
- IP:port-based restriction

**Detection:**
- Same external port, but port-specific filtering

```
Sent to 1.2.3.4:5000 → can receive from 1.2.3.4:5000 only
Sent to 1.2.3.4:6000 → different filter entry
```

### Symmetric NAT

**Behavior:**
- Different external port for each destination
- Hardest for hole punching

**Detection:**
- QAD to different servers shows different external ports

```
To Server A: 203.0.113.50:40000
To Server B: 203.0.113.50:40001  (different port!)
```

---

## Hole Punching Algorithm

### UDP Hole Punching Steps

```
1. Both peers connect to Intermediate (relay server)
2. Intermediate tells each peer the other's reflexive address
3. Both peers send UDP packets to each other simultaneously
4. NAT creates outbound mapping on first packet
5. When peer's packet arrives, mapping already exists
6. Bidirectional communication established
```

### Timing Considerations

```
             Agent                    Connector
               │                          │
    T=0        │──── UDP to Connector ───►│ (packet blocked by Connector's NAT)
               │                          │
    T=0        │◄─── UDP to Agent ────────│ (packet blocked by Agent's NAT)
               │                          │
               │  NAT mappings now exist  │
               │                          │
    T=50ms     │──── UDP retransmit ─────►│ (NOW GETS THROUGH!)
               │                          │
    T=50ms     │◄─── UDP retransmit ──────│ (NOW GETS THROUGH!)
               │                          │
```

### Coordination Protocol

```rust
// Intermediate coordinates timing
struct StartPunchingCommand {
    target_time: Instant,  // Both start at same time
    peer_candidates: Vec<Candidate>,
    session_id: u64,
}
```

---

## ICE (Interactive Connectivity Establishment)

### Candidate Types (RFC 8445)

| Type | Description | Priority |
|------|-------------|----------|
| host | Local IP address | Highest |
| srflx | Server reflexive (STUN) | Medium |
| prflx | Peer reflexive (discovered) | Medium |
| relay | TURN server | Lowest |

### Priority Formula

```rust
fn calculate_priority(type_pref: u32, local_pref: u32, component: u32) -> u32 {
    (type_pref << 24) + (local_pref << 8) + (256 - component)
}

// Type preferences (higher = better)
const HOST_TYPE_PREF: u32 = 126;
const SRFLX_TYPE_PREF: u32 = 100;
const PRFLX_TYPE_PREF: u32 = 110;
const RELAY_TYPE_PREF: u32 = 0;
```

### Candidate Pair States

```
Frozen → Waiting → In-Progress → Succeeded/Failed
```

---

## QUIC Connection Migration

### How It Works

QUIC supports path migration natively (RFC 9000 Section 9):

1. **Path Validation:** Send PATH_CHALLENGE on new path
2. **Receive PATH_RESPONSE:** New path validated
3. **Switch Paths:** Update preferred path

### quiche API

```rust
// Check if migration is possible
if conn.is_path_validated(new_path) {
    // Migrate to new path
    conn.migrate(new_path);
}

// Or let quiche handle it automatically
conn.on_timeout();  // Handles path probing
```

### Migration Triggers

```rust
enum MigrationTrigger {
    BetterPath,      // Direct path discovered
    PathFailure,     // Current path failed
    NetworkChange,   // Interface changed (WiFi→Cell)
}
```

---

## Implementation Patterns

### Candidate Gathering (Rust)

```rust
use std::net::{IpAddr, SocketAddr, UdpSocket};

fn gather_host_candidates() -> Vec<Candidate> {
    let mut candidates = Vec::new();

    // Get all interfaces
    for iface in pnet::datalink::interfaces() {
        for ip in iface.ips {
            if ip.is_ipv4() && !ip.ip().is_loopback() {
                candidates.push(Candidate {
                    candidate_type: CandidateType::Host,
                    address: SocketAddr::new(ip.ip(), 0),
                    priority: calculate_priority(HOST_TYPE_PREF, 65535, 1),
                    foundation: format!("host-{}", iface.name),
                });
            }
        }
    }

    candidates
}

fn gather_reflexive_candidate(qad_response: &QadResponse) -> Candidate {
    Candidate {
        candidate_type: CandidateType::ServerReflexive,
        address: qad_response.observed_address,
        priority: calculate_priority(SRFLX_TYPE_PREF, 65535, 1),
        foundation: "srflx-intermediate".to_string(),
    }
}
```

### Connectivity Check

```rust
async fn check_connectivity(
    socket: &UdpSocket,
    candidate_pair: &CandidatePair,
) -> Result<Duration, CheckError> {
    let request = BindingRequest::new();
    let start = Instant::now();

    // Send binding request
    socket.send_to(&request.serialize(), candidate_pair.remote.address)?;

    // Wait for response with timeout
    let mut buf = [0u8; 1500];
    match tokio::time::timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
        Ok(Ok((len, from))) => {
            let response = BindingResponse::parse(&buf[..len])?;
            if response.transaction_id == request.transaction_id {
                Ok(start.elapsed())  // RTT
            } else {
                Err(CheckError::TransactionMismatch)
            }
        }
        _ => Err(CheckError::Timeout),
    }
}
```

### Hole Punching Coordinator

```rust
struct HolePunchCoordinator {
    candidates: Vec<CandidatePair>,
    socket: UdpSocket,
}

impl HolePunchCoordinator {
    async fn start_punching(&self, start_time: Instant) {
        // Wait until coordinated start time
        tokio::time::sleep_until(start_time.into()).await;

        // Send to all candidate pairs simultaneously
        for pair in &self.candidates {
            let request = BindingRequest::new();
            self.socket.send_to(&request.serialize(), pair.remote.address).ok();
        }

        // Retransmit with exponential backoff
        for attempt in 0..5 {
            tokio::time::sleep(Duration::from_millis(50 * (1 << attempt))).await;
            for pair in &self.candidates {
                if !pair.succeeded.load(Ordering::Relaxed) {
                    let request = BindingRequest::new();
                    self.socket.send_to(&request.serialize(), pair.remote.address).ok();
                }
            }
        }
    }
}
```

---

## NAT Detection

### Algorithm

```rust
async fn detect_nat_type(intermediate1: SocketAddr, intermediate2: SocketAddr) -> NatType {
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    // Query first server
    let addr1 = qad_query(&socket, intermediate1).await?;

    // Query second server
    let addr2 = qad_query(&socket, intermediate2).await?;

    if addr1.port() == addr2.port() {
        // Same port = cone NAT
        // Further tests needed to distinguish full/restricted/port-restricted
        NatType::Cone
    } else {
        // Different port = symmetric NAT
        NatType::Symmetric
    }
}
```

---

## Keepalive Strategy

### Timing

```rust
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const PATH_TIMEOUT: Duration = Duration::from_secs(60);
const MISSED_KEEPALIVES_THRESHOLD: u32 = 3;
```

### Implementation

```rust
async fn keepalive_loop(conn: &mut Connection, path: &Path) {
    let mut missed = 0;

    loop {
        tokio::time::sleep(KEEPALIVE_INTERVAL).await;

        if let Err(_) = send_keepalive(conn, path).await {
            missed += 1;
            if missed >= MISSED_KEEPALIVES_THRESHOLD {
                // Path failed, trigger fallback
                trigger_path_failure(conn).await;
                break;
            }
        } else {
            missed = 0;
        }
    }
}
```

---

## References

- [RFC 8445 - ICE](https://tools.ietf.org/html/rfc8445)
- [RFC 5389 - STUN](https://tools.ietf.org/html/rfc5389)
- [RFC 5766 - TURN](https://tools.ietf.org/html/rfc5766)
- [RFC 9000 - QUIC Connection Migration](https://www.rfc-editor.org/rfc/rfc9000#section-9)
- [NAT Behavior for UDP (RFC 4787)](https://tools.ietf.org/html/rfc4787)
- [Tailscale's NAT Traversal](https://tailscale.com/blog/how-nat-traversal-works/)
- [libp2p Hole Punching](https://docs.libp2p.io/concepts/nat/hole-punching/)
