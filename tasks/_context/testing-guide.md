# ZTNA Testing & Demo Guide

**Last Updated:** 2026-02-27
**Status:** Task 015 Complete (PR #10 merged). Oracle quick fixes: IPv6 QAD, CSPRNG IDs, dead FFI removal, UDP length sanity. 146 unit tests.

---

## Linting & Code Quality

### Pre-commit Hooks (Local)

```bash
# Install (one-time)
brew install pre-commit shellcheck
pre-commit install

# Run all hooks on staged files (automatic on git commit)
pre-commit run

# Run all hooks on entire codebase
pre-commit run --all-files
```

12 hooks: 5 rustfmt (per crate), 5 clippy (per crate), 1 ShellCheck, 1 SwiftLint.

### CI Lint (GitHub Actions)

Runs automatically on push/PR to master. Three parallel jobs:
- **Rust**: clippy + fmt across 5 crates (matrix strategy)
- **SwiftLint**: Strict mode with github-actions-logging reporter
- **ShellCheck**: Bash scripts only (zsh informational)

### Manual Lint Commands

```bash
# Rust (per crate)
cargo fmt --manifest-path app-connector/Cargo.toml --all -- --check
cargo clippy --manifest-path app-connector/Cargo.toml --all-targets -- -D warnings

# Swift
swiftlint lint --strict

# Shell (bash only)
grep -rl '#!/bin/bash' --include='*.sh' . | grep -v target | xargs shellcheck --severity=warning
```

---

## Configuration

Set these shell variables before running any remote commands below. Values match `docs/demo-runbook.md`.

```bash
export ZTNA_SSH_KEY="~/.ssh/hfymba.aws.pem"
export ZTNA_SSH_HOST="ubuntu@10.0.2.126"
export ZTNA_SSH="ssh -i $ZTNA_SSH_KEY $ZTNA_SSH_HOST"
export ZTNA_PUBLIC_IP="3.128.36.92"
export ZTNA_QUIC_PORT="4433"
export K8S_LB_HOST="10.0.150.205"
export K8S_LB_PORT="4433"
export ZTNA_ECHO_VIRTUAL_IP="10.100.0.1"
export ZTNA_WEB_VIRTUAL_IP="10.100.0.2"
export ZTNA_WEB_PORT="8080"
export ZTNA_INTERMEDIATE_BIND="10.0.2.126"
export ZTNA_INTERMEDIATE_METRICS_PORT="9090"
export ZTNA_CONNECTOR_METRICS_PORT="9091"
```

---

## AWS Cloud Comprehensive Demo (Task 006 Current) - RECOMMENDED

This is the primary demo showing the complete ZTNA stack with all current capabilities.

### Network Topology

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  AWS Cloud Deployment (Comprehensive Demo)                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────┐                    ┌──────────────────────────────────┐│
│  │  macOS Agent    │                    │  AWS EC2 (3.128.36.92)          ││
│  │  (Home NAT)    │                    │                                  ││
│  │                 │     QUIC/UDP       │  ┌────────────────────────────┐ ││
│  │  Config:        │◄──── :4433 ───────►│  │ Intermediate Server       │ ││
│  │  echo-service   │  0x2F routed       │  │ (systemd: ztna-           │ ││
│  │  → 10.100.0.1  │                    │  │  intermediate.service)    │ ││
│  │  web-app        │                    │  └────────────────────────────┘ ││
│  │  → 10.100.0.2  │                    │               │                  ││
│  │                 │                    │               ▼                  ││
│  │  Split tunnel:  │                    │  ┌────────────────────────────┐ ││
│  │  10.100.0.0/24  │                    │  │ App Connector              │ ││
│  │    → utun       │                    │  │ (systemd: ztna-            │ ││
│  │  everything     │                    │  │  connector.service)        │ ││
│  │  else: normal   │                    │  │ UDP + TCP + ICMP           │ ││
│  └─────────────────┘                    │  └───────────┬────────────────┘ ││
│                                          │              │                  ││
│                                          │              ▼                  ││
│                                          │  ┌────────────────────────────┐ ││
│                                          │  │ Echo Server (UDP :9999)    │ ││
│                                          │  │ (systemd: echo-            │ ││
│                                          │  │  server.service)           │ ││
│                                          │  └────────────────────────────┘ ││
│                                          └──────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────────────┘
```

### Multi-Terminal Demo Setup (5 Terminals)

Open 5 terminal windows to run the complete demo:

**Terminal 1 - AWS Server Logs (Intermediate):**
```bash
# SSH to AWS instance and watch intermediate-server logs
$ZTNA_SSH
sudo journalctl -u ztna-intermediate -f
```
*Watch for: "New connection", "Registration", "Relayed X bytes", "0x2F service datagram"*

**Terminal 2 - AWS Server Logs (Connector):**
```bash
$ZTNA_SSH
sudo journalctl -u ztna-connector -f
```
*Watch for: "Registered as Connector", "Forward to local", "TCP session", "ICMP Echo Reply"*

**Terminal 3 - macOS Agent Logs:**
```bash
log stream --predicate 'subsystem CONTAINS "ztna"' --info
```
*Watch for: "QUIC connection established", "Registered for service", "Tunneled routed packet"*

**Terminal 4 - Launch macOS Agent App:**
```bash
# Build if needed
xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
    -scheme ZtnaAgent -configuration Debug \
    -derivedDataPath /tmp/ZtnaAgent-build build

# Launch (configure UI: Host=$ZTNA_PUBLIC_IP, Port=$ZTNA_QUIC_PORT, Service=echo-service)
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app

# Or with auto-start for testing
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app \
    --args --auto-start --auto-stop 120 --exit-after-stop
```

**Terminal 5 - Test Traffic:**
```bash
# Test 1: ICMP ping through tunnel (full E2E with return-path)
ping -c 3 10.100.0.1
# Expected: 3 packets transmitted, 3 received, RTT ~80-150ms

# Test 2: UDP echo through tunnel
echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999

# Test 3: Verify split tunnel - this should NOT go through QUIC
ping -c 1 8.8.8.8  # Should work via normal routing, not tunnel

# Verify VPN interface and routes
ifconfig utun6
netstat -rn | grep utun
```

### Expected Demo Flow

1. **Terminal 4:** Launch app → Click "Start" (or use `--auto-start`)
2. **Terminal 3:** See "QUIC connection established", "Registered for service 'echo-service'"
3. **Terminal 1:** See "New connection from...", "Registration: Agent for service 'echo-service'"
4. **Terminal 2:** See "Registered as Connector for 'echo-service'"
5. **Terminal 5:** Run `ping -c 3 10.100.0.1`
6. **Terminal 1:** See "Service-routed datagram: 84 bytes" and "Relayed 84 bytes" (both directions)
7. **Terminal 2:** See Connector processing ICMP Echo Requests
8. **Terminal 3:** See "Injected 1 return packet(s) into TUN"
9. **Terminal 5:** See `64 bytes from 10.100.0.1: icmp_seq=1 ttl=64` replies

### What Currently Works vs What Needs Deployment

| Test | Status | Notes |
|------|--------|-------|
| UDP echo (`nc -u 10.100.0.1 9999`) | ✅ Works | Full E2E verified on AWS |
| ICMP ping (`ping 10.100.0.1`) | ✅ Works | Full E2E: Agent→Intermediate→Connector (Echo Reply)→Agent→TUN |
| TCP connect (`curl 10.100.0.2:8080`) | ✅ Works | HTTP through tunnel via web-app connector (relay-only) |
| Split tunnel (normal traffic untouched) | ✅ Works | Only 10.100.0.0/24 routes through utun |

---

## Pi k8s Cluster Demo (Task 006 Phase 1)

This section demonstrates ZTNA deployed to a **real Kubernetes cluster** with Cilium L2 LoadBalancer.

### Network Topology

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Pi k8s Cluster Deployment                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐                ┌─────────────────────────────────────────┐ │
│  │   macOS     │                │           Pi k8s Cluster                 │ │
│  │  (Home LAN) │                │           (10.0.150.101-108)            │ │
│  │             │                │                                          │ │
│  │  App or     │◄──── QUIC ────►│  ┌─────────────────────────────────────┐ │ │
│  │  Test CLI   │   UDP:4433     │  │    Intermediate Server              │ │ │
│  │             │                │  │    LoadBalancer: 10.0.150.205:4433  │ │ │
│  └─────────────┘                │  │    (Cilium L2 announcement)         │ │ │
│                                  │  └─────────────────────────────────────┘ │ │
│                                  │                    │                      │ │
│                                  │                    │ ClusterIP            │ │
│                                  │                    ▼                      │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    App Connector                    │ │ │
│                                  │  │    Registers for 'echo-service'     │ │ │
│                                  │  └───────────────┬─────────────────────┘ │ │
│                                  │                   │ ClusterIP             │ │
│                                  │                   ▼                       │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    Echo Server (test service)       │ │ │
│                                  │  │    UDP :9999                        │ │ │
│                                  │  └─────────────────────────────────────┘ │ │
│                                  └──────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Prerequisites

- Pi k8s cluster with Cilium (kubectl context: `k8s1`)
- Docker Hub images pushed (public repos)
- TLS secrets created in cluster

### Quick Start

```bash
# 1. Verify cluster access
kubectl --context k8s1 get nodes

# 2. Apply Cilium L2 configuration (cluster-scoped)
kubectl apply -f deploy/k8s/overlays/pi-home/cilium-l2.yaml

# 3. Deploy ZTNA components
kubectl apply -k deploy/k8s/overlays/pi-home

# 4. Check status
kubectl --context k8s1 get pods -n ztna
kubectl --context k8s1 get svc -n ztna
```

### Test Connection from macOS

```bash
# Build local app-connector
(cd app-connector && cargo build --release)

# Connect to k8s intermediate-server
./app-connector/target/release/app-connector \
  --server $K8S_LB_HOST:$K8S_LB_PORT \
  --service test-from-mac \
  --no-verify-peer
```

**Expected output:**
```
[INFO] ZTNA App Connector starting...
[INFO]   Server:  10.0.150.205:4433
[INFO]   Service: test-from-mac
[INFO] Connecting to Intermediate at 10.0.150.205:4433
[INFO] Registered as Connector for service 'test-from-mac'
[INFO] QAD: Observed address is 10.0.0.22:XXXXX
```

### Multi-Terminal Live Monitoring

**Terminal 1 - Intermediate Server Logs:**
```bash
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f
```
*Watch for: "New connection", "Registration", "Sent QAD"*

**Terminal 2 - App Connector Logs:**
```bash
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=app-connector -f
```
*Watch for: "Registered as Connector", "QAD: Observed address is"*

**Terminal 3 - Pod Status:**
```bash
watch -n2 'kubectl --context k8s1 get pods -n ztna -o wide'
```
*Watch for: Running status, restart counts*

**Terminal 4 - Run Test:**
```bash
./app-connector/target/release/app-connector \
  --server $K8S_LB_HOST:$K8S_LB_PORT \
  --service test-external \
  --no-verify-peer
```

### Quick Copy Commands

```bash
# Check LoadBalancer IP
kubectl --context k8s1 get svc -n ztna intermediate-server

# Check Cilium L2 lease holder
kubectl --context k8s1 get leases -n kube-system | grep l2announce

# Check all ZTNA pods
kubectl --context k8s1 get pods -n ztna -o wide

# Intermediate server logs
kubectl --context k8s1 logs -n ztna deployment/intermediate-server --tail=50

# App connector logs (may restart due to 30s idle timeout)
kubectl --context k8s1 logs -n ztna deployment/app-connector --tail=50

# Test UDP connectivity from Mac
nc -u -v -z $K8S_LB_HOST $K8S_LB_PORT
```

### Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| "Destination Host Unreachable" | `externalTrafficPolicy: Local` with L2 lease on different node | Change to `Cluster` policy |
| App Connector CrashLoopBackOff | Normal - 30s idle timeout | Expected behavior, will restart |
| LoadBalancer `<pending>` | Cilium L2 not enabled | Run helm upgrade to enable L2 |
| ErrImagePull | Docker Hub repos private | Make repos public on hub.docker.com |

### Expected App Connector Behavior

The app-connector in k8s registers for 'echo-service' and has **expected CrashLoopBackOff**:

1. Connects to intermediate-server ✅
2. Registers as Connector for 'echo-service' ✅
3. Receives QAD observed address ✅
4. Waits for traffic... (idle)
5. **30 second idle timeout** - QUIC connection closes
6. Process exits, Kubernetes restarts
7. Repeat from step 1

This is **working as designed** - the connector stays registered while there's activity.

### Files Reference

| File | Purpose |
|------|---------|
| `deploy/k8s/k8s-deploy-skill.md` | Comprehensive deployment skill guide |
| `deploy/k8s/base/` | Kustomize base manifests |
| `deploy/k8s/overlays/pi-home/` | Pi cluster overlay |
| `deploy/k8s/build-push.sh` | Multi-arch image builder |

---

## macOS ZtnaAgent E2E Test (Task 006 Phase 1.1) - NEW

This section demonstrates the **full E2E tunnel path** from macOS VPN app through k8s intermediate-server.

### Network Flow

```
┌─────────────────┐          ┌───────────────────┐          ┌─────────────────┐
│  macOS ZtnaAgent│          │  Pi k8s Cluster   │          │  Echo Service   │
│  (VPN Tunnel)   │          │                   │          │                 │
│                 │   QUIC   │  Intermediate     │   QUIC   │  App Connector  │
│  100.64.0.1 ────┼────────►│  Server           │◄─────────┤  (echo-service) │
│  utun6          │  UDP:4433│  10.0.150.205     │          │                 │
│                 │          │       │           │          │  ▼              │
└─────────────────┘          │       │ DATAGRAM  │          │  Echo Server    │
       │                     │       ▼ Relay     │          │  UDP :9999      │
       │ IP Packets          └───────────────────┘          └─────────────────┘
       ▼ (10.100.0.0/24 routed via VPN)
```

### Prerequisites

1. **Build macOS app with k8s IP:**
   ```bash
   # Verify PacketTunnelProvider.swift has k8s IP
   grep "serverHost" ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift
   # Should show: private let serverHost = "10.0.150.205"

   # Build the app
   xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
       -scheme ZtnaAgent -configuration Debug \
       -derivedDataPath /tmp/ZtnaAgent-build build
   ```

2. **K8s components running:**
   ```bash
   kubectl --context k8s1 get pods -n ztna
   # intermediate-server: Running
   # app-connector: Running (or CrashLoopBackOff - expected)
   # echo-server: Running
   ```

### Run the E2E Test

**Terminal 1 - K8s Intermediate Server Logs:**
```bash
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f
```

**Terminal 2 - Launch macOS App:**
```bash
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app --args --auto-start
```

**Terminal 3 - macOS System Logs:**
```bash
log stream --predicate 'subsystem CONTAINS "ztna"' --info
```

### Expected Results

1. **macOS VPN connects:**
   - System Settings → VPN shows "ZTNA Agent" as "Connected"
   - `ifconfig utun6` shows tunnel with 100.64.0.1

2. **K8s logs show connection:**
   ```
   [INFO] New connection from 10.0.0.22:XXXXX (scid=...)
   [INFO] Sent QAD to ... (observed: 10.0.0.22:XXXXX)
   ```
   Note: Source IP is SNAT'd to k8s node IP (externalTrafficPolicy: Cluster)

3. **Traffic tunneled successfully:**
   ```bash
   # Send UDP traffic through tunnel to echo-service (10.100.0.1 = virtual IP)
   echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999

   # K8s logs should show:
   # [INFO] Received 84 bytes to relay from ...
   # [WARN] No destination for relay from ...
   ```
   The "No destination" warning is expected - MVP routes by service ID, not by destination IP.

### Verify VPN Status

```bash
# Check VPN interface
ifconfig utun6

# Check routes
netstat -rn | grep utun6

# Check UDP connection to k8s
netstat -an | grep "$K8S_LB_HOST.$K8S_LB_PORT"

# Check Extension process
pgrep -fl Extension | grep tmp
```

### Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| VPN shows "Disconnected" | Extension not running | Click Start in app |
| No k8s connection logs | Wrong server IP in Extension | Rebuild with correct IP, clean DerivedData |
| Extension from wrong path | macOS caches old Extension | `rm -rf ~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*` |
| 30s connection timeout | No traffic to keep alive | Expected - QUIC idle timeout |

### Current Limitations

1. ~~**Return-path not implemented:**~~ ✅ FIXED (2026-01-31) - Agent reads DATAGRAMs from QUIC via `agent_recv_datagram()` FFI and injects into TUN via `packetFlow.writePackets()`
2. ~~**No keepalive:**~~ ✅ FIXED (2026-01-25) - macOS Agent sends keepalive PING every 10 seconds
3. ~~**No auto-recovery:**~~ ✅ FIXED (2026-01-31) - Connection resilience with exponential backoff + NWPathMonitor
4. **SNAT hides real IP:** QAD returns k8s node IP, not macOS real IP (due to externalTrafficPolicy: Cluster)
5. **Config via UI only:** macOS Agent config set in SwiftUI, not config file (acceptable for client app)

### Connection Resilience Testing (Added 2026-01-31)

The macOS Agent now auto-recovers when the QUIC connection drops.

**Test 1: Server Restart Recovery**
```bash
# Terminal 1: Start VPN, verify connected
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app
ping -c 3 10.100.0.1  # Should work

# Terminal 2: Restart intermediate server on AWS
$ZTNA_SSH 'sudo systemctl restart ztna-intermediate'

# Terminal 3: Watch macOS logs
log stream --predicate 'subsystem CONTAINS "ztna"' --info
# Expect: "Scheduling reconnect in 1.0s (reason: ...)"
# Then:   "Attempting reconnect to $ZTNA_PUBLIC_IP:$ZTNA_QUIC_PORT"
# Then:   "QUIC connection established"

# Terminal 1: Verify recovery
ping -c 3 10.100.0.1  # Should work again
```

**Test 2: WiFi Toggle Recovery**
```bash
# Disconnect WiFi in macOS System Settings, wait 5s, reconnect
# Watch logs for: "Network path unsatisfied" → "Network path changed (satisfied), scheduling reconnect"
# Verify: ping 10.100.0.1 works after reconnection
```

**Test 3: Backoff Verification**
```bash
# Stop intermediate server entirely (don't restart)
$ZTNA_SSH 'sudo systemctl stop ztna-intermediate'

# Watch logs — should see increasing backoff:
# "Scheduling reconnect in 1.0s"
# "Scheduling reconnect in 2.0s"
# "Scheduling reconnect in 4.0s"
# ... up to 30.0s cap

# Restart server — next successful connect resets backoff to 1s
$ZTNA_SSH 'sudo systemctl start ztna-intermediate'
```

**Expected behavior summary:**
| Trigger | Detection Path | Recovery |
|---------|---------------|----------|
| Server restart | `updateAgentState()` Closed/Error | Auto-reconnect with backoff |
| WiFi toggle | `NWPathMonitor` satisfied + Agent disconnected | Auto-reconnect |
| UDP connection failure | `NWConnection .failed` | Auto-reconnect with backoff |
| Keepalive timeout | `sendKeepalive()` NotConnected | Auto-reconnect with backoff |

### macOS Agent Keepalive (Added 2026-01-25)

The macOS Agent now includes keepalive to prevent QUIC idle timeout:

- **Interval:** 10 seconds (well under 30s idle timeout)
- **Mechanism:** Calls `agent_send_intermediate_keepalive()` which sends QUIC PING frame
- **Starts:** After successful service registration
- **Stops:** When tunnel stops or connection lost

**Verify keepalive is working:**
```bash
# Connect and wait 45+ seconds (past old 30s timeout)
networksetup -connectpppoeservice "ZTNA Agent"
sleep 45
scutil --nc status "ZTNA Agent" | grep -i status
# Should show: Connected (not Disconnected)
```

---

## Docker NAT Simulation Demo (Task 006 Phase 0)

This section demonstrates the ZTNA relay through **simulated NAT environments** using Docker.

### Network Topology

```
┌─────────────────────────────────────────────────────────────────┐
│                     Docker Host                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ztna-public (172.20.0.0/24) - "Internet" (no NAT)              │
│  └─ intermediate-server (172.20.0.10:4433)                      │
│  └─ nat-agent (172.20.0.2) - Agent's public interface           │
│  └─ nat-connector (172.20.0.3) - Connector's public interface   │
│                                                                  │
│  ztna-agent-lan (172.21.0.0/24) - Agent's private network       │
│  └─ quic-client (172.21.0.10) - behind NAT                      │
│  └─ nat-agent (172.21.0.2) - NAT gateway                        │
│                                                                  │
│  ztna-connector-lan (172.22.0.0/24) - Connector's private net   │
│  └─ app-connector (172.22.0.10) - behind NAT                    │
│  └─ echo-server (172.22.0.20:9999) - local service              │
│  └─ nat-connector (172.22.0.2) - NAT gateway                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Quick Start (One Command)

```bash
# Build everything and run full demo
tests/e2e/scenarios/docker-nat-demo.sh

# Or skip builds if images exist
tests/e2e/scenarios/docker-nat-demo.sh --no-build
```

### Demo Script Options

```bash
tests/e2e/scenarios/docker-nat-demo.sh [OPTIONS]

Options:
  --no-build    Skip Docker image builds (use existing images)
  --clean       Clean up containers and networks only
  --status      Show current container status
  --logs        Show container logs after demo
  --help        Show help
```

### What the Demo Tests

| Test | Description | Expected Result |
|------|-------------|-----------------|
| NAT simulation | Agent/Connector behind separate NATs | Both NATted to public IPs |
| Agent NAT | 172.21.0.10 → 172.20.0.2 | Correct NATted address |
| Connector NAT | 172.22.0.10 → 172.20.0.3 | Correct NATted address |
| UDP relay | Send "Hello from NAT!" through tunnel | Echo response received |
| End-to-end | Agent → NAT → Intermediate → NAT → Echo | Complete round-trip |

### Expected Output

```
╔══════════════════════════════════════════════════════════════╗
║          ZTNA Docker NAT Simulation Demo                     ║
╠══════════════════════════════════════════════════════════════╣
║  Agent (172.21.0.10)    --NAT-->  172.20.0.2                ║
║  Connector (172.22.0.10) --NAT-->  172.20.0.3               ║
║  Intermediate Server             @ 172.20.0.10:4433         ║
║  Echo Server                     @ 172.22.0.20:9999         ║
╚══════════════════════════════════════════════════════════════╝

==> Building Docker images...
[SUCCESS] All images built

==> Starting NAT simulation infrastructure...
[SUCCESS] Infrastructure started

==> Running NAT simulation test...
[INFO] Connection established!
[INFO] Registering as Agent for service: test-service
[INFO] Received DATAGRAM: 43 bytes
[SUCCESS] Echo response received through NAT tunnel!

╔══════════════════════════════════════════════════════════════╗
║                    Demo Summary                               ║
╠══════════════════════════════════════════════════════════════╣
║  ✓ Agent observed through NAT as: 172.20.0.2                 ║
║  ✓ Connector observed through NAT as: 172.20.0.3             ║
║  ✓ UDP relay through Intermediate Server working             ║
║  ✓ Echo response received through tunnel                     ║
╚══════════════════════════════════════════════════════════════╝
```

### Multi-Terminal Live Monitoring

Open 4 terminal windows to watch traffic flow in real-time:

**Terminal 1 - Intermediate Server (Relay Hub):**
```bash
docker logs -f ztna-intermediate
```
*Watch for: "New connection", "Registration", "Relayed X bytes"*

**Terminal 2 - App Connector:**
```bash
docker logs -f ztna-app-connector
```
*Watch for: "Registered as Connector", "QAD: Observed address is 172.20.0.3"*

**Terminal 3 - NAT Traffic Stats (Refreshing):**
```bash
watch -n1 'echo "=== Agent NAT ===" && docker exec ztna-nat-agent iptables -t nat -L POSTROUTING -v -n 2>/dev/null | grep -E "MASQ|pkts" && echo && echo "=== Connector NAT ===" && docker exec ztna-nat-connector iptables -t nat -L POSTROUTING -v -n 2>/dev/null | grep -E "MASQ|pkts"'
```
*Watch for: packet counts increasing on MASQUERADE rules*

**Terminal 4 - Run Test:**
```bash
cd deploy/docker-nat-sim
docker compose --profile test run --rm quic-client
```

**Alternative: Use the log watcher script:**
```bash
# Helper script with colored output
deploy/docker-nat-sim/watch-logs.sh intermediate  # Terminal 1
deploy/docker-nat-sim/watch-logs.sh connector     # Terminal 2
deploy/docker-nat-sim/watch-logs.sh traffic       # Terminal 3
```

### Quick Copy Commands

```bash
# Watch Intermediate Server relay activity
docker logs -f ztna-intermediate

# Watch Connector registration and forwarding
docker logs -f ztna-app-connector

# Watch NAT packet counts (one-shot)
docker exec ztna-nat-agent iptables -t nat -L POSTROUTING -v -n | grep MASQ

# Packet capture on Agent NAT gateway
docker exec ztna-nat-agent tcpdump -i eth1 -n udp port 4433

# All logs combined
docker compose -f deploy/docker-nat-sim/docker-compose.yml logs -f
```

### Manual Testing

**Start infrastructure only:**
```bash
cd deploy/docker-nat-sim
docker compose up -d intermediate-server echo-server nat-agent nat-connector app-connector
```

**Run test client manually:**
```bash
docker compose --profile test run --rm quic-client
```

**View NAT statistics:**
```bash
# Agent NAT gateway
docker exec ztna-nat-agent iptables -t nat -L -v

# Connector NAT gateway
docker exec ztna-nat-connector iptables -t nat -L -v
```

**Debug with netshoot containers:**
```bash
# Start debug containers
docker compose --profile debug up -d debug-agent debug-connector debug-public

# Test connectivity from agent LAN
docker exec ztna-debug-agent ping 172.20.0.10

# Packet capture on NAT gateway
docker exec ztna-nat-agent tcpdump -i eth1 -n
```

**Cleanup:**
```bash
tests/e2e/scenarios/docker-nat-demo.sh --clean
# Or manually:
cd deploy/docker-nat-sim && docker compose --profile debug --profile test down -v
```

### Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| "Docker daemon not running" | Docker Desktop not started | Start Docker Desktop |
| "Address already in use" | Previous containers still running | Run `--clean` first |
| "NAT not working" | Interface order changed | Containers detect interfaces dynamically |
| "Connection timeout" | Route not configured | Entrypoint scripts set up routes automatically |
| "No echo response" | Connector not registered | Check `docker logs ztna-app-connector` |

### Files Reference

| File | Purpose |
|------|---------|
| `deploy/docker-nat-sim/docker-compose.yml` | Network topology and services |
| `deploy/docker-nat-sim/Dockerfile.*` | Container images |
| `deploy/docker-nat-sim/entrypoint-*.sh` | Route setup scripts |
| `tests/e2e/scenarios/docker-nat-demo.sh` | Demo runner script |

---

## macOS Agent Demo (Task 005a)

This section demonstrates the full ZTNA stack using the **native macOS Agent app**.

### Quick Start (Automated)

```bash
# Build everything and run automated demo (30 seconds)
tests/e2e/scenarios/macos-agent-demo.sh --build --auto --duration 30

# Or run without rebuild
tests/e2e/scenarios/macos-agent-demo.sh --auto --duration 60
```

### Quick Start (Interactive)

```bash
# Start infrastructure and launch app (manual Start/Stop)
tests/e2e/scenarios/macos-agent-demo.sh --manual

# The app will open - click "Start" to connect
# View logs: log stream --predicate 'subsystem CONTAINS "ztna"' --info
# Click "Stop" when done, then Ctrl+C to stop infrastructure
```

### Demo Script Options

```bash
tests/e2e/scenarios/macos-agent-demo.sh [OPTIONS]

Options:
  --build         Build all components first (Rust + Xcode)
  --duration N    Run for N seconds (default: 30)
  --auto          Full automation (start, wait, stop, exit)
  --manual        Interactive mode (starts components, waits for user)
  --logs          Open log windows in separate Terminal tabs
  --help          Show help
```

### Manual Setup (Step by Step)

**Terminal 1: Echo Server**
```bash
tests/e2e/fixtures/echo-server/target/release/udp-echo --port 9999
```

**Terminal 2: Intermediate Server**
```bash
RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
  certs/cert.pem certs/key.pem
```

**Terminal 3: App Connector**
```bash
RUST_LOG=info app-connector/target/release/app-connector \
  --server 127.0.0.1:4433 \
  --service test-service \
  --forward 127.0.0.1:9999
```

**Terminal 4: Launch Agent App**
```bash
# Manual mode (use UI buttons)
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app

# OR automated mode (for testing)
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app \
  --args --auto-start --auto-stop 30 --exit-after-stop
```

### Agent App Command Line Arguments

| Argument | Description |
|----------|-------------|
| `--auto-start` | Automatically start VPN when app launches |
| `--auto-stop N` | Stop VPN after N seconds (requires `--auto-start`) |
| `--exit-after-stop` | Quit app after VPN stops (requires `--auto-stop`) |

### Viewing Agent Logs

```bash
# Real-time log stream (all agent logs)
log stream --predicate 'subsystem CONTAINS "ztna"' --info

# Recent logs (last 5 minutes)
log show --last 5m --predicate 'subsystem CONTAINS "ztna"' --info

# Filter for specific events
log show --last 5m --predicate 'subsystem CONTAINS "ztna"' --info | grep -E "(Starting|connected|established|QAD)"

# Extension-specific logs (find PID first)
EXT_PID=$(pgrep -f "com.hankyeomans.ztna-agent.ZtnaAgent.Extension" | head -1)
/usr/bin/log show --last 1m --predicate "processIdentifier == $EXT_PID" --info
```

### Expected Log Output

When the agent connects successfully:
```
Starting tunnel...
Tunnel settings applied successfully
QUIC agent created
UDP connection ready to 127.0.0.1:4433
QUIC connection initiated
QUIC connection established
QAD observed address: 127.0.0.1:XXXXX
```

### Troubleshooting macOS Agent

| Issue | Cause | Fix |
|-------|-------|-----|
| "Start Error" on first click | First-time VPN config race | Click Start again (retry logic handles this) |
| "Operation not permitted" | Missing entitlements | Rebuild with correct entitlements |
| No logs appearing | Log filter not matching | Use PID-specific query (see above) |
| Connection timeout | Infrastructure not running | Start Intermediate Server + Connector first |
| App won't launch | Not signed for development | Open Xcode, run from there first |

---

## Quick Start Demo (QUIC Test Client)

### 1. Build All Components

```bash
cd /Users/hank/dev/src/agent-driver/ztna-agent

# Build Intermediate Server
(cd intermediate-server && cargo build --release)

# Build App Connector
(cd app-connector && cargo build --release)

# Build Test Fixtures
(cd tests/e2e/fixtures/echo-server && cargo build --release)
(cd tests/e2e/fixtures/quic-client && cargo build --release)
```

### 2. Start Components (Manual)

```bash
# Terminal 1: Echo Server (test service)
tests/e2e/fixtures/echo-server/target/release/udp-echo --port 9999

# Terminal 2: Intermediate Server
# Note: E2E tests use certs/ at project root (see tests/e2e/config/env.local)
RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
  certs/cert.pem certs/key.pem

# Terminal 3: App Connector
RUST_LOG=info app-connector/target/release/app-connector \
  --server 127.0.0.1:4433 \
  --service test-service \
  --forward 127.0.0.1:9999
```

### 3. Run E2E Demo

```bash
# Send "HELLO" through the relay to echo server and back
tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --send-udp "HELLO_FROM_DEMO" \
  --dst 127.0.0.1:9999 \
  --wait 3000
```

**Expected output:**
```
[INFO] Connection established!
[INFO] Registering as Agent for service: test-service
[INFO] Built IP/UDP packet: 43 bytes (payload: 15 bytes)
[INFO] Received DATAGRAM: 43 bytes
RECV:4500002b...48454c4c4f5f46524f4d5f44454d4f
```

---

## Automated Test Suites

### Phase 1: Infrastructure Tests

```bash
# Run full MVP test suite (14 tests)
tests/e2e/run-mvp.sh
```

**Tests included:**
- Component startup/shutdown
- Direct UDP echo (bypasses relay)
- Port configuration validation
- Basic connectivity checks

### Phase 2 & 3.5: Protocol Validation Tests

```bash
# Run protocol validation suite (14 tests)
tests/e2e/scenarios/protocol-validation.sh
```

**Tests included:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| ALPN correct | Connect with `ztna-v1` | Connection established |
| ALPN wrong | Connect with wrong ALPN | Connection rejected |
| DATAGRAM at limit | Programmatic `max-1` sizing | Accepted + E2E verified |
| DATAGRAM over limit | Programmatic `max+1` sizing | BufferTooShort |
| Registration valid | `[0x10][len][id]` format | Accepted |
| Registration invalid | Malformed length | Handled gracefully |
| Zero-byte payload | Empty payload relay | OK |
| One-byte payload | Minimal payload relay | Echoed |
| Connector registration | `[0x11][len][id]` format | Accepted |
| Zero-length service ID | Empty ID (negative) | Handled gracefully |
| Overlong service ID | >255 bytes (negative) | Rejected |
| Unknown opcode | `0xFF` opcode | Handled gracefully |
| Multiple datagrams | Back-to-back sends | All queued |
| Malformed IP header | Non-UDP protocol | Dropped |

### Phase 4: Advanced UDP Tests

```bash
# Run advanced UDP test suite (11 tests)
tests/e2e/scenarios/udp-advanced.sh
```

**Tests included:**

**4.2 Echo Integrity Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| All-zeros payload | 64-byte zeros pattern | Echoed + verified |
| All-ones payload | 64-byte 0xFF pattern | Echoed + verified |
| Sequential payload | 256-byte 0x00..0xFF | Echoed + verified |
| Random payload | 128-byte random | Echoed + verified |
| Multiple payloads | 5 packets, 500ms delay | Multiple echoes |

**4.3 Concurrent Flow Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Parallel clients | 3 simultaneous clients | All receive responses |
| Flow isolation | Different source addresses | Independent flows |

**4.4 Long-Running Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Stream stability | 10 packets, 500ms interval | ≥80% success |
| Burst stress | 50 packets rapid-fire | All sent |
| Idle timeout | 5s idle within 30s limit | Connection alive |

### Phase 5: Reliability Tests

```bash
# Run reliability test suite (11 tests)
tests/e2e/scenarios/reliability-tests.sh
```

**Tests included:**

**5.1 Component Restart Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Intermediate restart | Stop/restart server, reconnect | Connectivity restored |
| Connector restart | Stop/restart connector | Data flow resumes |
| Active flow restart | Restart connector during stream | Partial delivery (≥1 packet) |

**5.2 Error Condition Tests:**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Unknown service ID | Send to non-existent service | No data echo (QAD only) |
| Unknown destination | Send to TEST-NET address | No data echo |
| Invalid certificates | Start server with bad cert path | Server refuses to start |
| Non-listening port | Connect to port 59999 | Connection fails/timeout |
| Rapid reconnection | 5 connections in 2 seconds | All succeed |

**5.3 Network Impairment Tests (Stretch):**
| Test | Description | Expected Result |
|------|-------------|-----------------|
| Packet loss | Simulate with pfctl/tc | Skipped (requires root) |
| Packet reorder | Simulate with tc netem | Skipped (requires root) |
| NAT rebinding | Port change simulation | Skipped (needs namespace) |

---

## Test Component Reference

### QUIC Test Client

**Location:** `tests/e2e/fixtures/quic-client/`

**Usage:**
```bash
quic-test-client [OPTIONS]

Options:
  --server ADDR      Intermediate server (default: 127.0.0.1:4433)
  --service ID       Register as Agent for service
  --send TEXT        Send raw text as DATAGRAM
  --send-hex HEX     Send hex-encoded data
  --send-udp TEXT    Send text wrapped in IP/UDP packet
  --dst IP:PORT      Destination for --send-udp
  --src IP:PORT      Source for --send-udp (default: 10.0.0.100:12345)
  --wait MS          Wait time for responses (default: 2000)

Protocol Validation (Phase 2):
  --alpn PROTO       Override ALPN (default: ztna-v1)
  --payload-size N   Generate N-byte payload (or 'max', 'max-1', 'max+1')
  --expect-fail      Expect connection to fail

Phase 3.5 - Programmatic DATAGRAM Sizing:
  --query-max-size   Print MAX_DGRAM_SIZE and MAX_UDP_PAYLOAD after connection

Phase 4 - Advanced Testing:
  --payload-pattern P  Payload pattern: zeros, ones, sequential, random
  --repeat N           Send N packets (default: 1)
  --delay MS           Delay between packets in repeat mode (default: 0)
  --burst N            Burst mode: send N packets as fast as possible
  --verify-echo        Verify echoed responses match sent data
```

**Examples:**
```bash
# Full E2E relay test
quic-test-client --service test-service --send-udp "Hello" --dst 127.0.0.1:9999

# ALPN negative test
quic-test-client --alpn "wrong" --expect-fail

# Boundary test (programmatic max)
quic-test-client --service test-service --payload-size max-1 --dst 127.0.0.1:9999

# Phase 4: Echo integrity with random payload
quic-test-client --service test-service --payload-size 100 --payload-pattern random \
  --dst 127.0.0.1:9999 --verify-echo

# Phase 4: Burst stress test (50 packets)
quic-test-client --service test-service --burst 50 --payload-size 100 --dst 127.0.0.1:9999
```

### UDP Echo Server

**Location:** `tests/e2e/fixtures/echo-server/`

**Usage:**
```bash
udp-echo --port 9999
```

Echoes back any UDP payload received.

---

## Log Locations

| Component | Log File |
|-----------|----------|
| Intermediate Server | `tests/e2e/artifacts/logs/intermediate-server.log` |
| App Connector | `tests/e2e/artifacts/logs/app-connector.log` |
| Echo Server | `tests/e2e/artifacts/logs/echo-server.log` |
| QUIC Test Client | `tests/e2e/artifacts/logs/quic-client.log` |

**View logs in real-time:**
```bash
# All components
tail -f tests/e2e/artifacts/logs/*.log

# Specific component with color
RUST_LOG=debug intermediate-server/target/release/intermediate-server ...
```

**Log levels:**
- `error` - Errors only
- `warn` - Warnings and errors
- `info` - Standard operation (default)
- `debug` - Detailed flow
- `trace` - QUIC packet-level detail

---

## Test Framework Reference

### Common Functions (lib/common.sh)

```bash
source tests/e2e/lib/common.sh

# Component lifecycle
start_intermediate           # Start with logging
start_connector             # Start with service ID
start_echo_server           # Start UDP echo
stop_all_components         # Clean shutdown

# Test helpers
test_start "Test name"      # Log test start
test_pass "Message"         # Log success
test_fail "Message"         # Log failure
test_warn "Message"         # Log warning

# QUIC helpers
send_via_quic "$data" "$server" "$wait_ms"
send_hex_via_quic "$hex" "$server" "$wait_ms"
```

### Environment Configuration

**File:** `tests/e2e/config/env.local`

```bash
# Network
INTERMEDIATE_HOST="127.0.0.1"
INTERMEDIATE_PORT="4433"
ECHO_SERVER_PORT="9999"

# Protocol
ALPN_PROTOCOL="ztna-v1"
MAX_DATAGRAM_SIZE="1350"  # Note: effective is ~1307

# Service
TEST_SERVICE_ID="test-service"

# Certificates
CERT_DIR="$PROJECT_ROOT/certs"
```

---

## Key Protocol Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `ALPN_PROTOCOL` | `b"ztna-v1"` | QUIC ALPN identifier |
| `MAX_DATAGRAM_SIZE` | 1350 | Config value |
| `EFFECTIVE_MAX` | ~1307 | Actual limit (QUIC overhead) |
| `IDLE_TIMEOUT_MS` | 30000 | 30 seconds |
| `Agent Registration` | `0x10` | `[0x10][len][service_id]` |
| `Connector Registration` | `0x11` | `[0x11][len][service_id]` |
| `Registration ACK` | `0x12` | `[0x12][status][len][service_id]` (Task 007) |
| `Registration NACK` | `0x13` | `[0x13][status][len][service_id]` (Task 007) |
| `Service-Routed Datagram` | `0x2F` | `[0x2F][len][service_id][ip_packet]` |
| `ZTNA_MAGIC` | `0x5A` | P2P keepalive prefix (Task 007) |
| `QAD Observed Address` | `0x01` | `[0x01][4 bytes IP][2 bytes port]` |

---

## Relay Path Verification

**How tests verify traffic flows through the QUIC relay (not directly):**

### 1. Port Isolation
| Test Type | Destination Port | Path |
|-----------|-----------------|------|
| Baseline | 9999 | Client → Echo Server (direct UDP) |
| Tunneled | 4433 | Client → Intermediate → Connector → Echo Server |

The QUIC test client connects to port **4433** (Intermediate Server), not port 9999. Traffic only reaches the Echo Server after being relayed through the Connector.

### 2. Protocol Enforcement
- **Agent registration** (`0x10`): QUIC client registers with a service ID
- **Connector registration** (`0x11`): App Connector registers with matching service ID
- **Intermediate Server**: Only routes between matching Agent↔Connector pairs
- Without both registrations, data won't flow

### 3. IP Encapsulation
The `--send-udp` flag wraps payloads in IP/UDP headers:
```
QUIC DATAGRAM payload (42+ bytes):
  ├─ IPv4 Header (20 bytes): src=10.0.0.100, dst=127.0.0.1
  ├─ UDP Header (8 bytes): src_port=12345, dst_port=9999
  └─ Application Data (N bytes): "HELLO"
```

The Connector **must parse** these headers to extract and forward the inner UDP payload. This proves the relay path is active.

### 4. Dependency Verification
| Component Stopped | Baseline Test | Tunneled Test |
|-------------------|---------------|---------------|
| Echo Server | ❌ Fails | ❌ Fails |
| Intermediate | ✅ Works | ❌ Fails |
| Connector | ✅ Works | ❌ Fails |

If Intermediate or Connector are stopped, tunneled tests fail immediately, proving traffic depends on the relay.

### 5. Latency Evidence
- **Baseline RTT**: ~30-100 µs (direct UDP loopback)
- **Tunneled RTT**: ~300-500 µs (QUIC + relay overhead)

The ~200-400 µs overhead demonstrates the additional QUIC protocol processing and relay hops.

---

## Troubleshooting

### Connection Timeout

**Symptom:** `Connection timeout` after 5 seconds

**Causes:**
1. Intermediate Server not running
2. Wrong port (check 4433)
3. Firewall blocking UDP

**Debug:**
```bash
# Check server is listening
lsof -i :4433

# Check with trace logging
RUST_LOG=trace quic-test-client --server 127.0.0.1:4433
```

### ALPN Mismatch

**Symptom:** `Connection closed during handshake`

**Cause:** Client and server ALPN don't match

**Fix:** Ensure both use `ztna-v1`:
```bash
# Check client
quic-test-client --alpn "ztna-v1" ...

# Check server logs for ALPN
grep ALPN tests/e2e/artifacts/logs/intermediate-server.log
```

### BufferTooShort

**Symptom:** `Failed to queue DATAGRAM: BufferTooShort`

**Cause:** Payload exceeds ~1307 byte effective limit

**Fix:** Reduce payload size:
```bash
# Max safe payload for IP/UDP wrapped data
# IP (20) + UDP (8) + payload (1278) = 1306 bytes OK
quic-test-client --payload-size 1278 --dst 127.0.0.1:9999
```

### No Response from Echo Server

**Symptom:** `No DATAGRAMs received` after sending

**Causes:**
1. Echo server not running
2. App Connector not forwarding
3. Flow mapping issue (single flow only currently)

**Debug:**
```bash
# Check echo server
nc -u 127.0.0.1 9999
# Type "test" and press Enter - should echo back

# Check connector logs
tail tests/e2e/artifacts/logs/app-connector.log
```

---

## Test Coverage Summary

### E2E Tests (Task 004)

| Phase | Tests | Status | Validates |
|-------|-------|--------|-----------|
| 1 | 14 | ✅ Complete | Component lifecycle, direct UDP |
| 1.5 | 1 | ✅ Complete | Full E2E relay path |
| 2 | 8 | ✅ Complete | ALPN, boundaries, registration |
| 3 | 5 | ✅ Complete | Relay validation, connectivity |
| 3.5 | 6 | ✅ Complete | Coverage gaps (connector reg, malformed headers) |
| 4.2 | 5 | ✅ Complete | Echo integrity (payload patterns) |
| 4.3 | 2 | ✅ Complete | Concurrent flows, isolation |
| 4.4 | 3 | ✅ Complete | Long-running, burst, idle timeout |
| 5.1 | 3 | ✅ Complete | Component restart behavior |
| 5.2 | 5 | ✅ Complete | Error conditions (invalid certs, ports) |
| 5.3 | 3 | ⚠️ Skipped | Network impairment (requires root) |
| 6.1 | 2 | ✅ Complete | Latency (baseline vs tunneled RTT, percentiles) |
| 6.2 | 1 | ✅ Complete | Throughput (PPS, Mbps) |
| 6.3 | 3 | ✅ Complete | Timing (handshake, resources, reconnect) |

**E2E Test Total: 61+**

### Unit Tests (All Tasks)

| Component | Tests | Status | Notes |
|-----------|-------|--------|-------|
| **packet_processor** | 82 | ✅ Pass | 24 agent/core + 58 P2P module |
| ├─ agent/core | 24 | ✅ | Agent FFI, registration, packet processing, recv_datagram |
| ├─ p2p/candidate | 11 | ✅ | ICE candidate types, gathering |
| ├─ p2p/signaling | 13 | ✅ | Message encode/decode |
| ├─ p2p/connectivity | 17 | ✅ | Binding, pairs, check list |
| └─ p2p/hole_punch | 17 | ✅ | Coordinator, path selection |
| **intermediate-server** | 16 | ✅ Pass | 6 signaling + 9 registry + 1 integ |
| **app-connector** | 18 | ✅ Pass | 8 unit + 5 TCP + 2 ICMP + 1 config + 2 integ |

**Unit Test Total: 114**

### Combined Test Count

| Category | Count | Status |
|----------|-------|--------|
| Unit tests (Rust) | 116 | ✅ All pass |
| E2E tests (Shell) | 61+ | ✅ All pass (except network impairment) |
| **Grand Total** | **177+** | ✅ |

---

## Phase 6: Performance Metrics

Run performance benchmarks:
```bash
tests/e2e/scenarios/performance-metrics.sh
```

**Configurable via environment:**
```bash
RTT_SAMPLES=100 BURST_COUNT=500 tests/e2e/scenarios/performance-metrics.sh
```

**Key Metrics Collected:**

| Metric | Description | Typical Value |
|--------|-------------|---------------|
| `BASELINE_RTT_*` | Direct UDP to echo server | 30-100 µs |
| `TUNNELED_RTT_*` | Through QUIC relay | 300-500 µs |
| `THROUGHPUT_PPS` | Packets per second (burst) | 200K-400K |
| `THROUGHPUT_MBPS` | Megabits per second | 2-4 Gbps (theoretical) |
| `HANDSHAKE_*` | QUIC connection setup | 750-900 µs |
| `*_MEM_KB` | Memory usage per component | 5-7 MB |

**Output:** `tests/e2e/artifacts/metrics/perf_YYYYMMDD_HHMMSS.txt`

---

## Phase 7: P2P Hole Punching Tests (Task 005 + 006)

> **Status:** ✅ P2P Swift Integration complete (Phase 6). Rust unit tests pass (81). macOS Agent has full P2P wiring. NAT testing pending (Phase 6.8).

### Unit Tests (Complete)

All P2P unit tests pass (see P2P Module Tests section below):
```bash
# Run all P2P unit tests
(cd core/packet_processor && cargo test p2p)
```

**Test modules:**
- `candidate.rs` - 11 tests (ICE candidate gathering)
- `signaling.rs` - 13 tests (message protocol)
- `connectivity.rs` - 17 tests (binding checks, pairs)
- `hole_punch.rs` - 17 tests (coordinator, path selection)

### macOS Agent P2P Integration (Task 006 Phase 6) ✅ IMPLEMENTED

The macOS Agent (PacketTunnelProvider.swift) now has full P2P wiring:

**What's wired:**
- 12 P2P FFI functions declared in bridging header
- Hole punch auto-starts after service registration
- Binding request pump (per-candidate NWConnections)
- P2P QUIC connection after hole punch succeeds
- Packet routing via `agent_get_active_path()` (Direct vs Relay)
- P2P keepalive (15s interval)
- Path stats logging and fallback detection
- Full cleanup on disconnect/reconnect

**Socket architecture:**
```
PacketTunnelProvider
├── udpConnection (NWConnection)          ← Intermediate (relay, always active)
├── bindingConnections (per-candidate)    ← Temporary, during hole punch only
└── p2pConnection (NWConnection)          ← Direct to Connector (after hole punch)
```

### P2P NAT Testing (Task 006 Phase 6.8) — ✅ COMPLETE (2026-01-31)

**Result:** Direct P2P QUIC path achieved. macOS Agent behind home router NAT successfully hole-punched to AWS Connector on port 4434. P2P keepalive stable for 3.5+ minutes with zero missed keepalives.

```bash
# macOS Agent logs — watch for P2P activity
log stream --predicate 'subsystem CONTAINS "ztna"' --info
```

**Actual log sequence (verified 2026-01-31):**
```
# 1. Tunnel starts, relay connects
"Starting tunnel..."
"QUIC connection established"
"Registered for service 'echo-service'"

# 2. Hole punch initiates
"Hole punch initiated for service 'echo-service'"

# 3. Binding checks (two candidates)
"Binding connection ready to 0.0.0.0:4434"       ← fails (non-routable, expected)
"Binding connection ready to 3.128.36.92:4434"    ← succeeds
"Processed binding response from 3.128.36.92:4434"

# 4. Hole punch succeeds
"Hole punch SUCCESS: direct path to 3.128.36.92:4434"

# 5. P2P QUIC connection
"P2P connection ready to 3.128.36.92:4434"
"P2P QUIC connection ESTABLISHED - switching to direct path"

# 6. Stable keepalive (every 15s)
"Path: DIRECT, RTT: 0ms, missed keepalives: 0, fallback: false"  ← repeated 14+ times
```

**Test procedure (macOS behind home NAT → AWS):**

**Terminal 1 — AWS Intermediate logs:**
```bash
$ZTNA_SSH
sudo journalctl -u ztna-intermediate -f
```

**Terminal 2 — AWS Connector logs:**
```bash
$ZTNA_SSH
sudo journalctl -u ztna-connector -f
```

**Terminal 3 — macOS Agent logs:**
```bash
log stream --predicate 'subsystem CONTAINS "ztna"' --info
```

**Terminal 4 — Launch and test:**
```bash
open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app
# After connected:
ping -c 3 10.100.0.1

# Check for direct traffic (non-relay):
sudo tcpdump -i en0 udp and not host $ZTNA_PUBLIC_IP
```

**Verified outcomes (2026-01-31):**
| Scenario | Result | Verification |
|----------|--------|-------------|
| Hole punch succeeds | ✅ Direct P2P QUIC path | 14 consecutive keepalive checks, 0 missed |
| Relay still works | ✅ No regression | Relay handles traffic during ~1.8s warm-up |
| P2P keepalive stable | ✅ 3.5+ minutes | `missed keepalives: 0` in all path stats |

**Bug found & fixed during testing:**
- Agent `recv()` passed raw keepalive responses (0x5A-prefixed, 6 bytes) to `quiche::recv()` which rejected them
- Fix: Added keepalive demux at top of `recv()` in `lib.rs` — intercepts 0x10/0x11 before QUIC routing
- Before fix: missed keepalives hit 3 at 30s → fallback to relay
- After fix: zero missed keepalives over 3.5+ minutes

**Known minor issues (acceptable for MVP):**
- 0.0.0.0:4434 candidate always fails (non-routable host candidate from Connector binding)
- RTT reports 0ms (keepalive RTT tracking not yet calibrated)
- ~1.8s warm-up window before P2P accepts traffic (relay covers this gap)

### E2E Tests (Planned — post-NAT testing)

```bash
# Run P2P hole punching test suite (when available)
tests/e2e/scenarios/p2p-hole-punching.sh
```

**Planned tests:**

| Test | Description | Expected Result |
|------|-------------|-----------------|
| Direct connection (localhost) | Agent connects directly to Connector | Connection established |
| Candidate exchange | Agent/Connector exchange candidates via Intermediate | Both receive peer candidates |
| Path selection (direct) | RTT comparison prefers direct | Uses direct path |
| Fallback to relay | Direct fails, falls back to relay | Relay path used |
| Simulated multi-host | Agent on 127.0.0.2, Connector on 127.0.0.3 | Direct path established |

### Logging Commands for P2P Debugging

```bash
# Watch P2P-specific logs in real-time
RUST_LOG=debug,ztna::p2p=trace cargo run ...

# Filter logs for candidate gathering
grep -i "candidate" tests/e2e/artifacts/logs/*.log

# Filter logs for signaling messages
grep -i "signaling\|offer\|answer" tests/e2e/artifacts/logs/*.log

# Filter logs for binding checks
grep -i "binding\|check" tests/e2e/artifacts/logs/*.log

# Filter logs for path selection
grep -i "path\|direct\|relay" tests/e2e/artifacts/logs/*.log
```

### Manual P2P Demo (Localhost)

Once Phase 4 integration is complete:

```bash
# Terminal 1: Start Intermediate Server
RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
  certs/cert.pem certs/key.pem

# Terminal 2: Start App Connector (with P2P server mode)
RUST_LOG=info app-connector/target/release/app-connector \
  --server 127.0.0.1:4433 \
  --service test-service \
  --forward 127.0.0.1:9999 \
  --p2p-cert app-connector/certs/connector-cert.pem \
  --p2p-key app-connector/certs/connector-key.pem

# Terminal 3: Start Echo Server
tests/e2e/fixtures/echo-server/target/release/udp-echo --port 9999

# Terminal 4: Agent (QUIC test client with P2P)
# When P2P is enabled, client should:
# 1. Connect to Intermediate
# 2. Exchange candidates via signaling
# 3. Attempt direct connection to Connector
# 4. Fall back to relay if direct fails
tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --send-udp "HELLO_P2P" \
  --dst 127.0.0.1:9999 \
  --enable-p2p \
  --wait 5000
```

### Local Testing Limitations

| Feature | Testable? | Notes |
|---------|-----------|-------|
| Host candidates | ✅ Yes | Enumerate local interfaces |
| Signaling protocol | ✅ Yes | Via Intermediate relay |
| Direct QUIC connection | ✅ Yes | Agent → Connector localhost |
| Binding checks | ✅ Yes | Request/response validation |
| Path selection | ✅ Yes | RTT-based decision logic |
| Fallback logic | ✅ Yes | Simulate direct failure |
| **NAT hole punching** | ❌ No | Requires real NAT (Task 006) |
| **Reflexive addresses** | ❌ No | QAD returns 127.0.0.1 locally |
| **NAT type detection** | ❌ No | Requires real NAT |

---

## Unit Tests

### Running All Unit Tests

```bash
# All Rust components (packet_processor, intermediate-server, app-connector)
cargo test --workspace

# Specific component
(cd core/packet_processor && cargo test)
(cd intermediate-server && cargo test)
(cd app-connector && cargo test)
```

### P2P Module Tests (Task 005)

**Location:** `core/packet_processor/src/p2p/`

| Module | Tests | Description |
|--------|-------|-------------|
| `candidate.rs` | 11 | ICE candidate types, priority calculation, gathering |
| `signaling.rs` | 13 | Message encode/decode, framing, session IDs |
| `connectivity.rs` | 17 | Binding protocol, pairs, check list management |
| `hole_punch.rs` | 17 | Coordinator state machine, path selection |

**Run P2P tests specifically:**
```bash
(cd core/packet_processor && cargo test p2p)
```

**Key test categories:**

**Candidate Module:**
- `test_candidate_type_preference` - Type ordering per RFC 8445
- `test_calculate_priority` - Priority formula validation
- `test_gather_host_candidates` - Local interface enumeration
- `test_gather_reflexive_candidate` - Server-reflexive from QAD
- `test_sort_candidates_by_priority` - Priority-based sorting

**Signaling Module:**
- `test_encode_decode_*` - All message types round-trip
- `test_decode_multiple_messages` - Stream parsing with length prefixes
- `test_partial_message_decode` - Incomplete buffer handling
- `test_generate_session_id` - Unique ID generation

**Connectivity Module:**
- `test_binding_request_serialization` - Binding request encode/decode
- `test_candidate_pair_priority` - RFC 8445 §6.1.2.3 pair priority
- `test_check_list_priority_ordering` - Highest priority first
- `test_foundation_based_unfreezing` - ICE unfreezing logic
- `test_exponential_backoff` - RTO calculation (100ms → 1600ms)
- `test_nomination` - Candidate pair nomination

**Hole Punch Module:**
- `test_coordinator_initial_state` - Initial Idle state
- `test_state_transitions` - State machine progression
- `test_gathering_state_with_*` - Candidate gathering for host/reflexive/relay
- `test_signaling_creates_offer` - CandidateOffer message generation
- `test_handle_answer_*` - Processing CandidateAnswer messages
- `test_handle_start_punching` - State transition to Checking
- `test_checking_produces_binding_requests` - Binding request generation
- `test_handle_binding_response_*` - Response handling (success/failure)
- `test_path_selection_*` - Direct vs relay decision logic
- `test_should_switch_to_direct` - Threshold-based switching (50% faster)
- `test_should_switch_to_relay` - Failure-based fallback

### Intermediate Server Tests

**Location:** `intermediate-server/src/`

| Module | Tests | Description |
|--------|-------|-------------|
| `signaling.rs` | 6 | Session manager, agent/connector tracking |
| `registry.rs` | 9 | Client registry, pair matching, multi-service agent, service lookup, Connector replacement |
| `main.rs` | 1 | Integration (handshake + QAD) |

**Run Intermediate tests:**
```bash
(cd intermediate-server && cargo test)
```

### App Connector Tests

**Location:** `app-connector/src/`

| Module | Tests | Description |
|--------|-------|-------------|
| Packet parsing | 8 | IP/UDP construction, header parsing |
| TCP proxy | 5 | `test_tcp_flags`, `test_build_tcp_packet_syn_ack`, `test_build_tcp_packet_with_data`, `test_tcp_checksum_validity`, `test_max_tcp_payload_fits_datagram` |
| ICMP handling | 2 | `test_build_icmp_reply`, `test_icmp_checksum_validity` |
| Config | 1 | JSON config loading |
| Integration | 2 | QUIC handshake, registration |

**Run App Connector tests:**
```bash
(cd app-connector && cargo test)
```

### Test Count Summary

| Component | Tests | Notes |
|-----------|-------|-------|
| packet_processor | 82 | 24 agent/core + 11 candidate + 13 signaling + 17 connectivity + 17 hole_punch |
| intermediate-server | 16 | 6 signaling + 9 registry + 1 integration |
| app-connector | 18 | 8 unit + 5 TCP + 2 ICMP + 1 config + 2 integration |
| **Total** | **116** | All passing, 0 ignored |

---

## Next Steps

After running the demo, you can:

1. **Explore logs** - See packet flow through components
2. **Modify tests** - Add scenarios in `tests/e2e/scenarios/`
3. **Run performance tests** - `tests/e2e/scenarios/performance-metrics.sh`
4. **Deploy to cloud** - See Task 006 for cloud deployment

---

## Complete End-to-End Demo

This section demonstrates the full ZTNA stack including relay and P2P foundations.

### 1. Build Everything

```bash
cd /Users/hank/dev/src/agent-driver/ztna-agent

# Build all components
(cd intermediate-server && cargo build --release)
(cd app-connector && cargo build --release)
(cd tests/e2e/fixtures/echo-server && cargo build --release)
(cd tests/e2e/fixtures/quic-client && cargo build --release)

# Run all unit tests (114 tests)
cargo test --workspace
```

### 2. Run Full E2E Test Suite

```bash
# Run all E2E tests (61+ tests)
tests/e2e/run-mvp.sh

# Or run specific test suites:
tests/e2e/scenarios/protocol-validation.sh    # Phase 2 & 3.5 (14 tests)
tests/e2e/scenarios/udp-advanced.sh           # Phase 4 (11 tests)
tests/e2e/scenarios/reliability-tests.sh      # Phase 5 (11 tests)
tests/e2e/scenarios/performance-metrics.sh    # Phase 6 (6 tests)
```

### 3. Interactive Demo with Config Files (Manual)

Components now support JSON config files for dynamic configuration.

**Terminal 1: Echo Server**
```bash
tests/e2e/fixtures/echo-server/target/release/udp-echo --port 9999
```

**Terminal 2: Intermediate Server (with config)**
```bash
RUST_LOG=info intermediate-server/target/release/intermediate-server \
  --config deploy/config/intermediate.json
# Or legacy positional args:
# RUST_LOG=info intermediate-server/target/release/intermediate-server 4433 \
#   certs/cert.pem certs/key.pem
```

**Terminal 3: App Connector (with config)**
```bash
RUST_LOG=info app-connector/target/release/app-connector \
  --config deploy/config/connector.json
# Or legacy CLI args:
# RUST_LOG=info app-connector/target/release/app-connector \
#   --server 127.0.0.1:4433 --service test-service --forward 127.0.0.1:9999
```

**Terminal 4: Send Data Through Relay**
```bash
# Basic relay test
tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --send-udp "HELLO_WORLD" \
  --dst 127.0.0.1:9999 \
  --wait 3000

# Verify echo with integrity check
tests/e2e/fixtures/quic-client/target/release/quic-test-client \
  --server 127.0.0.1:4433 \
  --service test-service \
  --payload-size 100 \
  --payload-pattern random \
  --dst 127.0.0.1:9999 \
  --verify-echo
```

### Config File Reference

| Component | Config Path | Default Search Paths |
|-----------|------------|---------------------|
| Intermediate | `deploy/config/intermediate.json` | `/etc/ztna/intermediate.json`, `intermediate.json` |
| Connector | `deploy/config/connector.json` | `/etc/ztna/connector.json`, `connector.json` |
| Agent | `deploy/config/agent.json` | N/A (configured via macOS app UI) |

### 4. View Logs

```bash
# All component logs
tail -f tests/e2e/artifacts/logs/*.log

# Intermediate Server only
tail -f tests/e2e/artifacts/logs/intermediate-server.log

# App Connector only
tail -f tests/e2e/artifacts/logs/app-connector.log

# Filter for specific events
grep "Registered\|DATAGRAM\|forward" tests/e2e/artifacts/logs/*.log
```

### 5. Cleanup

```bash
# Stop all components (if running)
pkill -f intermediate-server
pkill -f app-connector
pkill -f udp-echo
```

---

## File Reference

| Purpose | Path |
|---------|------|
| **Test Framework** | |
| Common functions | `tests/e2e/lib/common.sh` |
| Environment config | `tests/e2e/config/env.local` |
| **E2E Test Scripts** | |
| Main test runner | `tests/e2e/run-mvp.sh` |
| Protocol validation (Phase 2 & 3.5) | `tests/e2e/scenarios/protocol-validation.sh` |
| Advanced UDP tests (Phase 4) | `tests/e2e/scenarios/udp-advanced.sh` |
| Boundary tests | `tests/e2e/scenarios/udp-boundary.sh` |
| Connectivity tests | `tests/e2e/scenarios/udp-connectivity.sh` |
| Echo tests | `tests/e2e/scenarios/udp-echo.sh` |
| Reliability tests (Phase 5) | `tests/e2e/scenarios/reliability-tests.sh` |
| Performance metrics (Phase 6) | `tests/e2e/scenarios/performance-metrics.sh` |
| P2P hole punching (Phase 7) | `tests/e2e/scenarios/p2p-hole-punching.sh` (planned) |
| **macOS Agent Demo (Task 005a)** | `tests/e2e/scenarios/macos-agent-demo.sh` |
| **Docker NAT Demo (Task 006)** | `tests/e2e/scenarios/docker-nat-demo.sh` |
| **Docker NAT Infrastructure** | `deploy/docker-nat-sim/` |
| **Test Fixtures** | |
| QUIC test client | `tests/e2e/fixtures/quic-client/` |
| UDP echo server | `tests/e2e/fixtures/echo-server/` |
| **Binaries (after build)** | |
| Intermediate Server | `intermediate-server/target/release/intermediate-server` |
| App Connector | `app-connector/target/release/app-connector` |
| QUIC Test Client | `tests/e2e/fixtures/quic-client/target/release/quic-test-client` |
| UDP Echo Server | `tests/e2e/fixtures/echo-server/target/release/udp-echo` |
| **Artifacts** | |
| Logs | `tests/e2e/artifacts/logs/` |
| Metrics | `tests/e2e/artifacts/metrics/` |
| **Certificates** | |
| Server TLS (E2E tests) | `certs/cert.pem`, `certs/key.pem` |
| Connector P2P TLS | `app-connector/certs/connector-cert.pem`, `app-connector/certs/connector-key.pem` |
| **P2P Source (Task 005)** | |
| Candidate gathering | `core/packet_processor/src/p2p/candidate.rs` |
| Signaling protocol | `core/packet_processor/src/p2p/signaling.rs` |
| Connectivity checks | `core/packet_processor/src/p2p/connectivity.rs` |
| Hole punch coordinator | `core/packet_processor/src/p2p/hole_punch.rs` |
| **Configuration (Task 006)** | |
| Agent config example | `deploy/config/agent.json` |
| Connector config example | `deploy/config/connector.json` |
| Intermediate config example | `deploy/config/intermediate.json` |
| **AWS Deployment (Task 006)** | |
| AWS deploy skill guide | `deploy/aws/aws-deploy-skill.md` |
| **macOS Agent Source** | |
| PacketTunnelProvider | `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` |
| ContentView (VPN UI) | `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift` |
| **Documentation** | |
| Task 006 state | `tasks/006-cloud-deployment/state.md` |
| Task 005 state | `tasks/005-p2p-hole-punching/state.md` |
| Task 005 plan | `tasks/005-p2p-hole-punching/plan.md` |
| Task 005 todo | `tasks/005-p2p-hole-punching/todo.md` |

---

## Return-Path: DATAGRAM → TUN ✅ COMPLETE

> **Status:** ✅ Implemented 2026-01-31. Full bidirectional tunnel working. `ping 10.100.0.1` returns replies.

### Complete Data Flow

```
                  OUTGOING (✅ Working)
┌──────────┐    ┌──────────────────────┐    ┌──────────────┐    ┌───────────┐
│ macOS App│───►│ PacketTunnelProvider │───►│ Intermediate │───►│ Connector │
│ ping ... │    │ readPackets() →      │    │ Server       │    │ → backend │
│          │    │ agent_send_datagram()│    │ 0x2F routing  │    │           │
└──────────┘    └──────────────────────┘    └──────────────┘    └───────────┘

                  INCOMING (✅ Working)
┌──────────┐    ┌──────────────────────┐    ┌──────────────┐    ┌───────────┐
│ macOS App│◄───│ PacketTunnelProvider │◄───│ Intermediate │◄───│ Connector │
│ gets     │    │ drainIncomingDgrams()│    │ Server       │    │ ← backend │
│ reply!   │    │ writePackets()→TUN  │    │ implicit rte │    │  response │
└──────────┘    └──────────────────────┘    └──────────────┘    └───────────┘
```

### Implementation Details

**1. Rust FFI: `agent_recv_datagram()`** (`core/packet_processor/src/lib.rs`)
- `received_datagrams: VecDeque<Vec<u8>>` queue on Agent struct
- Non-QAD DATAGRAMs queued in `process_incoming_datagrams()`
- `agent_recv_datagram()` FFI polls queue, returns `AgentResult::Ok` or `AgentResult::NoData`

**2. C Bridging** (`ios-macos/Shared/PacketProcessor-Bridging-Header.h`)
- `AgentResult agent_recv_datagram(Agent* agent, uint8_t* out_data, size_t* out_len);`

**3. Swift: `drainIncomingDatagrams()`** (`PacketTunnelProvider.swift`)
- Called after every `agent_recv()` that processes incoming UDP data
- Polls `agent_recv_datagram()` in a loop until `AgentResultNoData`
- Validates IPv4 version nibble
- Batches packets into single `packetFlow.writePackets()` call
- macOS kernel delivers replies to originating apps

### Verification

```bash
# ICMP ping through tunnel (Connector generates Echo Reply)
ping -c 3 10.100.0.1
# Expected: 3 packets transmitted, 3 received, 0% packet loss, RTT ~80-150ms

# UDP echo through tunnel
echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999

# Split tunnel (should NOT go through QUIC)
ping -c 1 8.8.8.8  # Works via normal routing
```

### Server-Side Verification

```bash
# SSH to AWS EC2
$ZTNA_SSH

# Check bidirectional relay
sudo journalctl -u ztna-intermediate --since "1 min ago" | grep "Relayed"
# Should see: "Relayed 84 bytes for 'echo-service' from [Agent] to [Connector]"
# And:        "Relayed 84 bytes from [Connector] to [Agent]"

# Check Connector ICMP handling (set RUST_LOG=debug temporarily)
sudo journalctl -u ztna-connector --since "1 min ago" | grep "ICMP"
```

---

## Phase 7: HTTP Through Tunnel (Multi-Service) ✅ COMPLETE

### AWS Services

Two Connector instances run on the AWS EC2:

| Service | systemd Unit | Port | Mode | Virtual IP |
|---------|-------------|------|------|-----------|
| echo-service | `ztna-connector.service` | 4434 | P2P + relay | 10.100.0.1 |
| web-app | `ztna-connector-web.service` | 4435 | relay-only | 10.100.0.2 |

Plus:
- `http-server.service` — Python HTTP server on `:8080` serving `/opt/ztna/www/index.html`
- `ztna-intermediate.service` — QUIC relay on `:4433`

### HTTP Test Procedure

```bash
# 1. Ensure VPN connected (both services registered)
# Agent logs should show: "Registered for service 'echo-service'"
# And: "Registered for service 'web-app'"

# 2. Test HTTP through tunnel (relay path)
curl -v http://10.100.0.2:8080/
# Expected: 200 OK with HTML content "ZTNA Test Page"

# 3. Multiple concurrent requests
for i in $(seq 1 10); do
    curl -s -o /dev/null -w "%{http_code} %{time_total}s\n" http://10.100.0.2:8080/
done
# Expected: all 200, ~70-80ms each

# 4. Regression check — UDP echo still works
ping -c 3 10.100.0.1
# Expected: replies via P2P direct path (~32ms)
```

### AWS Service Management

```bash
$ZTNA_SSH

# Check all services
systemctl status ztna-intermediate ztna-connector ztna-connector-web http-server echo-server

# Restart a connector
sudo systemctl restart ztna-connector-web

# View web-app connector logs
sudo journalctl -u ztna-connector-web -f
```

---

## Phase 8: Performance Metrics ✅ COMPLETE

### Benchmark Results (2026-02-21)

| Metric | P2P (Direct) | Relay | Ratio |
|--------|-------------|-------|-------|
| RTT avg | 32.6 ms | 76.0 ms | 2.3x faster |
| RTT min | 31.2 ms | 64.6 ms | |
| RTT max | 34.5 ms | 165.5 ms | |

### P2P Latency Test

```bash
# 50-sample ping via P2P direct path (10.100.0.1 → echo-service)
ping -c 50 10.100.0.1
# Expected: ~32ms avg, 0% loss after hole punch completes
```

### Relay Latency Test

```bash
# HTTP timing via relay (10.100.0.2 → web-app, relay-only)
for i in $(seq 1 50); do
    curl -s -o /dev/null -w "%{time_total}\n" http://10.100.0.2:8080/
done
# Expected: ~76ms avg
```

### 10-Minute Stability Test

```bash
# 600 pings via P2P path
ping -c 600 10.100.0.1
# Expected: 600/600 received, 0.0% loss

# Monitor Agent logs during test
log stream --predicate 'subsystem CONTAINS "ztna"' --info | grep -E "Path:|fallback|DIRECT|RELAY"
# Expected: stable "Path: DIRECT", no flapping
```

---

## Phase 8.5: P2P → Relay Failover Test ✅ COMPLETE

### Test Design

Block P2P traffic on the external interface only, preserving Connector↔Intermediate relay on loopback.

**Key Discovery:** The Connector uses a SINGLE `quic_socket` (port 4434) for both P2P and relay QUIC. Blocking port 4434 globally kills both paths. Must use interface-specific iptables.

### Test Procedure

```bash
# SSH to AWS
$ZTNA_SSH

# 1. Verify P2P baseline
ping -c 5 10.100.0.1  # Should get replies at ~32ms

# 2. Block P2P on external interface only
sudo iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP
# This blocks: Agent → Connector P2P (from internet via ens5)
# This preserves: Connector → Intermediate relay (loopback, not ens5)

# 3. Run sustained ping (3 minutes)
ping -c 180 10.100.0.1
# Expected: 180/180, 0% loss, ~32ms avg (relay path)

# 4. Monitor Intermediate logs (separate terminal)
sudo journalctl -u ztna-intermediate -f | grep -E "Relayed|echo-service"
# Expected: continuous "Relayed N bytes for 'echo-service'" entries

# 5. Verify recovery — unblock P2P
sudo iptables -F INPUT
sleep 60
ping -c 5 10.100.0.1  # Should work at ~32ms (P2P may re-establish)

# 6. Clean up
sudo iptables -F INPUT
```

### How Failover Works

`sendP2PDatagram()` in PacketTunnelProvider.swift provides **per-packet** failover:
1. Tries `agent_send_datagram_p2p()` for each packet
2. If it returns error (NotConnected=4), immediately calls `sendRoutedDatagram()`
3. `sendRoutedDatagram()` wraps with 0x2F header and sends via relay
4. Zero-downtime — no waiting for keepalive timeout or reconnection

### Results (2026-02-21)

```
180 packets transmitted, 180 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 30.238/31.821/35.293/0.668 ms
```

### WRONG Way to Test (Common Mistake)

```bash
# ❌ WRONG: Blocks ALL traffic on port 4434 (kills relay too!)
sudo iptables -A INPUT -p udp --dport 4434 -j DROP

# ✅ RIGHT: Block only external interface (preserves loopback relay)
sudo iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP
```

---

## Manual Failover Testing Commands (Quick Reference)

> **See also:** `docs/demo-runbook.md` for the full 5-terminal demo with talking points.

All commands assume SSH to AWS: `$ZTNA_SSH`

### Block P2P (Force Relay Fallback)

```bash
# Block P2P on external interface only (preserves Connector↔Intermediate relay on loopback)
sudo iptables -A INPUT -i ens5 -p udp --dport 4434 -j DROP
```

### Verify Relay Takeover

```bash
# Check Intermediate logs for relay activity
sudo journalctl -u ztna-intermediate --since "30 sec ago" | grep "Relayed"
# Expected: "Relayed 84 bytes for 'echo-service'" entries
```

### Unblock P2P (Allow Recovery)

```bash
sudo iptables -F INPUT
# Wait 30-60s for P2P keepalive to re-establish direct path
```

### Restart Intermediate (Test Resilience)

```bash
sudo systemctl restart ztna-intermediate
# macOS Agent will auto-recover (1s backoff + ~40ms handshake ≈ 1s recovery)
# Watch Agent logs: log stream --predicate 'subsystem CONTAINS "ztna"' --info
```

### Stop/Start Connector (Test No-Backend)

```bash
# Stop echo-service connector
sudo systemctl stop ztna-connector
# Traffic to 10.100.0.1 will have no backend — packets relayed but undelivered

# Restart
sudo systemctl start ztna-connector
# Connector re-registers, traffic resumes
```

### Stop/Start Web-App Connector

```bash
sudo systemctl stop ztna-connector-web
# curl http://10.100.0.2:8080/ will timeout (no backend)

sudo systemctl start ztna-connector-web
# HTTP resumes
```

### Clean Up All iptables Rules

```bash
sudo iptables -F INPUT
sudo iptables -L INPUT  # Verify empty chain
```
