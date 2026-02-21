# ZTNA Demo Runbook

**Purpose:** Self-contained 5-terminal demo for blog posts, articles, and presentations.
**Last Updated:** 2026-02-21
**Prerequisites:** AWS EC2 running all services, macOS Agent built, VPN configuration set.

---

## Prerequisites Checklist

Before starting the demo, verify:

- [ ] SSH access to AWS EC2: `ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126`
- [ ] macOS Agent app built (Xcode or `xcodebuild`)
- [ ] Agent configured: Host=3.128.36.92, Port=4433, Services=echo-service,web-app
- [ ] AWS services running (see Verify Services below)

### Verify AWS Services

```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126 \
    'systemctl is-active ztna-intermediate ztna-connector ztna-connector-web http-server echo-server'
```

Expected output: 5 lines of `active`. If any show `inactive`, restart:

```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126 \
    'sudo systemctl restart ztna-intermediate ztna-connector ztna-connector-web http-server echo-server'
```

---

## Terminal Layout

Open 5 terminal windows, arranged so all are visible:

| Terminal | Purpose | What to Watch |
|----------|---------|---------------|
| T1 | AWS Intermediate logs | Connection registration, relay traffic |
| T2 | AWS Connector logs | P2P activity, ICMP/TCP/UDP handling |
| T3 | macOS Agent logs | QUIC connection, service registration, hole punch |
| T4 | Test traffic | ping, curl, nc commands |
| T5 | Failover testing | SSH to AWS for iptables commands |

---

## Act 1: Connect

Establish the ZTNA tunnel from macOS to AWS.

**T1 — AWS Intermediate Server logs:**
```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126 \
    'sudo journalctl -u ztna-intermediate -f'
```

**T2 — AWS Connector logs:**
```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126 \
    'sudo journalctl -u ztna-connector -f'
```

**T3 — macOS Agent logs:**
```bash
log stream --predicate 'subsystem CONTAINS "ztna"' --info
```

**T4 — Launch the macOS Agent app:**
```bash
# Build if needed (one-time):
xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
    -scheme ZtnaAgent -configuration Debug \
    -derivedDataPath /tmp/ZtnaAgent-build build

# Launch:
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app
```

Click **Start** in the app UI.

**What you'll see across terminals:**

- **T3:** `"QUIC connection established"`, `"Registered for service 'echo-service'"`, `"Registered for service 'web-app'"`
- **T1:** `"New connection from..."`, `"Registration: Agent for service 'echo-service'"`, `"Registration: Agent for service 'web-app'"`
- **T2:** `"Registered as Connector for 'echo-service'"`, `"QAD: Observed address is ..."`
- **T3:** `"Hole punch initiated for service 'echo-service'"` ... `"Hole punch SUCCESS"` ... `"P2P QUIC connection ESTABLISHED"`

**Talking point:** The Agent connects to the Intermediate Server via QUIC, registers for two services, then automatically hole-punches a direct P2P path to the Connector — all in under 2 seconds.

---

## Act 2: ICMP Ping via P2P

Demonstrate direct P2P path with sub-35ms latency.

**T4:**
```bash
ping -c 10 10.100.0.1
```

**Expected output:**
```
64 bytes from 10.100.0.1: icmp_seq=0 ttl=64 time=31.5 ms
64 bytes from 10.100.0.1: icmp_seq=1 ttl=64 time=32.8 ms
...
10 packets transmitted, 10 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 31.1/32.6/34.5/0.8 ms
```

**What you'll see:**

- **T2:** Connector processes ICMP Echo Requests and generates Echo Replies
- **T3:** `"Tunneled routed packet"` and `"Injected return packet(s) into TUN"`
- **T1:** No relay activity (traffic goes direct P2P, bypassing Intermediate)

**Talking point:** Traffic flows directly between the Agent and Connector via P2P hole-punched QUIC — the Intermediate Server is not involved. Latency is ~32ms (macOS home NAT to AWS us-east-2).

---

## Act 3: HTTP via Relay

Demonstrate TCP HTTP through the relay path.

**T4:**
```bash
curl -v http://10.100.0.2:8080/
```

**Expected output:**
```
*   Trying 10.100.0.2:8080...
* Connected to 10.100.0.2 (10.100.0.2) port 8080
> GET / HTTP/1.1
> Host: 10.100.0.2:8080
...
< HTTP/1.0 200 OK
...
<html><body><h1>ZTNA Test Page</h1>...</body></html>
```

**What you'll see:**

- **T1:** `"Service-routed datagram"` and `"Relayed N bytes for 'web-app'"` (both directions)
- **T3:** `"Tunneled routed packet to 'web-app'"` for outbound, return packets injected to TUN
- **T4:** HTTP 200 response with HTML content

Run multiple requests to show consistency:
```bash
for i in $(seq 1 5); do
    curl -s -o /dev/null -w "Request $i: HTTP %{http_code} in %{time_total}s\n" http://10.100.0.2:8080/
done
```

**Talking point:** The web-app Connector is relay-only — TCP traffic flows through the Intermediate Server. The userspace TCP proxy in the Connector handles SYN/ACK/FIN, and HTTP works transparently. Average latency is ~76ms via relay (vs ~32ms for P2P direct).

---

## Act 4: Failover — Block P2P, Continue via Relay

Demonstrate seamless per-packet failover from P2P to relay.

**T5 — SSH to AWS (separate session):**
```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126
```

**T4 — Start a sustained ping:**
```bash
ping -c 60 10.100.0.1
```

While ping is running, switch to T5:

**T5 — Block P2P traffic on the external interface:**
```bash
sudo iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP
```

**Important:** Use `-i ens5` (interface-specific) — blocking globally kills both P2P and relay since the Connector uses a shared socket.

**What you'll see:**

- **T4:** Ping continues with **zero packet loss** — no interruption
- **T1:** Relay activates: `"Relayed 84 bytes for 'echo-service'"` entries appear
- **T3:** P2P send failures trigger automatic relay fallback (per-packet, no timeout needed)

**Talking point:** The Agent tries P2P first for every packet. When P2P fails, it immediately falls back to the relay path — per-packet, not per-connection. There's no reconnection delay, no keepalive timeout to wait for. The traffic seamlessly moves through the Intermediate Server.

---

## Act 5: Recovery — Unblock P2P

**T5 — Remove the iptables rule:**
```bash
sudo iptables -F INPUT
```

**T4:** Let ping continue for 30-60 more seconds.

**What you'll see:**

- **T3:** P2P keepalive resumes, path switches back to DIRECT
- **T1:** Relay entries stop (traffic goes direct again)
- **T4:** Ping continues with 0% loss throughout the entire test

**Final ping summary (expected):**
```
60 packets transmitted, 60 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 30.2/31.8/35.3/0.7 ms
```

**Talking point:** P2P recovery is automatic. Once the direct path is available again, the Agent switches back. The entire failover and recovery happened with zero packet loss — the user never notices.

---

## Key Talking Points for Blog/Article

1. **Split tunnel architecture** — only ZTNA traffic (10.100.0.0/24) goes through the tunnel. Everything else routes normally. No VPN-style "all traffic" overhead.

2. **P2P by default, relay as fallback** — the Agent automatically hole-punches a direct path. If that fails, traffic seamlessly falls back to the relay through the Intermediate Server. Per-packet failover means zero downtime.

3. **Multi-protocol support** — ICMP (ping), UDP (echo), and TCP (HTTP) all work through the same QUIC DATAGRAM tunnel. The Connector handles protocol-specific processing.

4. **Multi-service routing** — the 0x2F service-routed datagram protocol lets a single Agent access multiple backend services. Each service gets its own virtual IP (10.100.0.1, 10.100.0.2, etc.).

5. **Connection resilience** — the Agent auto-recovers from server restarts, WiFi toggles, and network changes. Exponential backoff (1s → 30s cap) prevents thundering herd on outages.

6. **Performance** — P2P direct: 32.6ms avg. Relay: 76ms avg. 10-minute sustained test: 600/600 packets, 0% loss. P2P is 2.3x faster than relay.

---

## Cleanup

After the demo:

**T5 — Ensure iptables are clean:**
```bash
sudo iptables -F INPUT
sudo iptables -L INPUT   # Should show empty chain
```

**T4 — Stop the macOS Agent:**
Click **Stop** in the app UI, or:
```bash
# Force stop if needed
networksetup -disconnectpppoeservice "ZTNA Agent"
```

---

## Common Mistakes

| Mistake | What Happens | Fix |
|---------|-------------|-----|
| `iptables -A INPUT -p udp --dport 4434 -j DROP` (no `-i ens5`) | Kills both P2P AND relay — Connector can't talk to Intermediate either | Always use `-i ens5` for interface-specific blocking |
| Forgetting to `iptables -F INPUT` after demo | Future SSH or services may be affected | Always clean up iptables rules |
| Starting ping before hole punch completes | First few pings go via relay (~76ms) before P2P kicks in (~32ms) | Wait for "P2P QUIC connection ESTABLISHED" in T3 before testing P2P latency |
| Agent not configured for both services | Only one service routes correctly | Ensure providerConfiguration has both echo-service and web-app |

---

## Quick Reference: AWS Commands

```bash
# SSH to AWS
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126

# Check all services
systemctl status ztna-intermediate ztna-connector ztna-connector-web http-server echo-server

# View Intermediate logs
sudo journalctl -u ztna-intermediate -f

# View echo-service Connector logs
sudo journalctl -u ztna-connector -f

# View web-app Connector logs
sudo journalctl -u ztna-connector-web -f

# Block P2P (failover test)
sudo iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP

# Unblock P2P (recovery)
sudo iptables -F INPUT

# Restart all services
sudo systemctl restart ztna-intermediate ztna-connector ztna-connector-web http-server echo-server
```

---

**Cross-references:**
- Full testing guide: `tasks/_context/testing-guide.md`
- Architecture details: `tasks/_context/components.md`
- Task 006 state: `tasks/006-cloud-deployment/state.md`
