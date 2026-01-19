# Research: E2E Relay Testing

**Task ID:** 004-e2e-relay-testing

---

## Purpose

Document research findings, test strategies, and tools for comprehensive end-to-end testing.

---

## Test Environment Options

### Option 1: Docker Compose

**Pros:**
- Isolated networking
- Reproducible
- Easy CI integration

**Cons:**
- macOS Agent requires host networking
- Network Extension complicates Docker

### Option 2: Local Processes

**Pros:**
- Simple, no containers
- Direct testing of real components
- Easier debugging

**Cons:**
- Less isolated
- Manual cleanup needed

### Decision

**MVP: Local processes with scripts**
- Agent runs on host (requires Network Extension)
- Intermediate and Connector as local processes
- Scripts to manage lifecycle

---

## Test Tools

### Networking

| Tool | Purpose |
|------|---------|
| `ping` | ICMP connectivity |
| `curl` | HTTP testing |
| `nc` (netcat) | TCP/UDP testing |
| `socat` | Advanced socket testing |
| `iperf3` | Throughput measurement |

### Monitoring

| Tool | Purpose |
|------|---------|
| `tcpdump` | Packet capture |
| `wireshark` | Packet analysis |
| `log stream` | macOS system logs |

---

## Test Scenarios

### Scenario 1: Basic Ping

```
Agent (macOS)
    │
    │ ping 10.0.0.100
    ▼
NEPacketTunnelProvider intercepts
    │
    │ QUIC DATAGRAM
    ▼
Intermediate (localhost:4433)
    │
    │ QUIC DATAGRAM relay
    ▼
Connector (localhost)
    │
    │ Forward to echo server
    ▼
Echo Server (10.0.0.100 simulated)
    │
    │ ICMP reply
    ▼
[reverse path]
```

### Scenario 2: HTTP Request

```
curl -x tunnel http://internal.app/api
    │
    ▼
Agent intercepts TCP SYN
    │
    ▼
[... relay through Intermediate ...]
    │
    ▼
Connector forwards to internal.app:80
    │
    ▼
HTTP server responds
    │
    ▼
[response returns through tunnel]
```

---

## Latency Measurement

### Methodology

1. **Baseline:** Direct connection to local service
2. **Tunneled:** Same request through ZTNA tunnel
3. **Overhead:** Tunneled - Baseline

### Expected Overhead

| Component | Estimated Latency |
|-----------|-------------------|
| Agent processing | 1-5ms |
| QUIC encryption | 1-2ms |
| Intermediate relay | 5-10ms |
| QUIC decryption | 1-2ms |
| Connector processing | 1-5ms |
| **Total overhead** | **10-25ms locally** |

---

## Error Scenarios

### Connection Failures

| Scenario | Expected Behavior |
|----------|-------------------|
| Intermediate down | Agent reconnects, traffic dropped |
| Connector down | Traffic dropped, error logged |
| Service down | TCP RST or timeout |

### Data Corruption

| Scenario | Expected Behavior |
|----------|-------------------|
| QUIC corruption | QUIC retransmits |
| Payload corruption | Detected at application layer |

---

## Test Data

### Echo Payloads

```
tests/fixtures/
├── small.txt      # 100 bytes
├── medium.txt     # 10 KB
├── large.txt      # 1 MB
└── huge.txt       # 100 MB
```

### Test IPs

| IP | Purpose |
|----|---------|
| 10.0.0.100 | Test service 1 |
| 10.0.0.101 | Test service 2 |
| 192.168.100.1 | Simulated internal network |

---

## CI Integration

### GitHub Actions Example

```yaml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Build components
        run: |
          cargo build --release -p intermediate-server
          cargo build --release -p app-connector

      - name: Run E2E tests
        run: ./tests/e2e/run-all.sh
```

---

## References

- [iperf3 documentation](https://iperf.fr/iperf-doc.php)
- [tcpdump tutorial](https://danielmiessler.com/study/tcpdump/)
- [Network testing best practices](https://www.phoronix.com/scan.php?page=article&item=network-testing-2023)
