# TODO: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** Task 004, Task 005, Task 005a
**Status:** ✅ COMPLETE (MVP)
**Last Updated:** 2026-02-21

---

## Prerequisites

- [x] Task 004 (E2E Relay Testing) complete and merged
- [x] Task 005 (P2P Hole Punching) protocol implementation complete
- [x] Task 005a (Swift Agent Integration) complete
- [x] Create feature branch: `git checkout -b feature/006-cloud-deployment`
- [x] AWS CLI access verified
- [ ] DigitalOcean API key configured (`doctl auth init`)
- [x] Raspberry Pi k8s cluster accessible ✅ (deployed and tested)

---

## ⚠️ Oracle Review Findings (2026-01-25)

Critical issues identified and tasks added:
- [x] Document NAT-to-NAT topology requirement (plan.md updated)
- [x] Define P2P listener ports (--p2p-listen-port 4434 added to app-connector)
- [ ] Add direct vs relay verification methods
- [ ] Add NAT classification tooling
- [ ] Parameterize hard-coded configs before remote testing
- [ ] Document TLS trust flow for self-signed certs

---

## Phase 0: Docker NAT Simulation ✅ COMPLETE

- [x] Create Docker NAT simulation environment
  - [x] Network A: ztna-public (172.20.0.0/24 - "Internet")
  - [x] Network B: ztna-agent-lan (172.21.0.0/24 with NAT)
  - [x] Network C: ztna-connector-lan (172.22.0.0/24 with NAT)
- [x] Deploy Intermediate in public network
- [x] Deploy Agent and Connector in different NAT networks
- [x] Test signaling protocol through simulated NAT
- [x] Verify address observation (QAD)
- [x] Document results in state.md

**Location:** `deploy/docker-nat-sim/`
**Demo script:** `tests/e2e/scenarios/docker-nat-demo.sh`

---

## Phase 1: Configuration Parameterization

> **Must complete before remote testing (Oracle finding)**
> **CRITICAL for cloud deployment and scaling**

### 1.1 Environment Configuration
- [ ] Create `deploy/env/` directory
- [ ] Create `cloud.env` template
- [ ] Create `home.env` template
- [ ] Update test scripts to source env files

### 1.2 Remove Hard-Coded Values
- [ ] Replace `127.0.0.1:4433` with `$INTERMEDIATE_HOST:$INTERMEDIATE_PORT`
- [ ] Replace `test-service` with `$SERVICE_ID`
- [ ] Replace cert paths with `$CERT_DIR`
- [ ] Test locally with parameterized configs

### 1.3 Address Hardcoding Technical Debt (Added 2026-01-25)

**Problem**: Multiple places have hardcoded IPs that must stay in sync:
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`: `serverHost` (now configurable per deploy)
- `deploy/k8s/overlays/pi-home/kustomization.yaml`: `io.cilium/lb-ipam-ips: "10.0.150.205"`

**Progress (2026-01-26):**
- [x] Test scripts: Now use `10.100.0.0/24` as ZTNA virtual service range
  - `10.100.0.1` = echo-service (UDP 9999)
  - Removed all `1.1.1.1` references from documentation
- [x] macOS Agent VPN routing: Updated to route `10.100.0.0/24` through tunnel
- [x] AWS deployment: serverHost = "3.128.36.92"
- [x] Pi k8s deployment: serverHost = "10.0.150.205"

**Remaining Work:**
- [x] macOS Agent: Read server address from configuration (UserDefaults + providerConfiguration) ✅ (Task #2)
- [ ] K8s overlays: Use environment-specific kustomization files (`pi-home/`, `do-nyc1/`, `aws-us-east-2/`)
- [ ] Document IP allocation scheme for different environments

**Why This Matters:**
- Pi cluster: 10.0.150.x (home LAN)
- DigitalOcean: Public IP from droplet
- AWS: Elastic IP or NLB DNS
- Each requires different LoadBalancer annotations and macOS Agent config

### 1.3 P2P Port Definition
- [x] Add `--p2p-listen-port` CLI arg to app-connector
- [x] Default to 4434 (or configurable)
- [ ] Update all firewall documentation to include UDP 4434
- [x] Test P2P with fixed port locally

---

## Phase 2: DigitalOcean Deployment (Simple Relay Testing)

### 2.1 Account Setup
- [ ] Configure doctl CLI: `doctl auth init`
- [ ] Verify API access: `doctl account get`
- [ ] Note SSH key ID for droplet creation

### 2.2 Infrastructure Provisioning
- [ ] Create Droplet (Ubuntu 24.04, s-1vcpu-1gb, nyc1)
  ```bash
  doctl compute droplet create ztna-relay \
    --image ubuntu-24-04-x64 \
    --size s-1vcpu-1gb \
    --region nyc1 \
    --ssh-keys $SSH_KEY_ID
  ```
- [ ] Create firewall rules
  - [ ] UDP 4433 inbound (Intermediate)
  - [ ] UDP 4434 inbound (Connector P2P)
  - [ ] TCP 22 inbound (SSH, admin IP only)
- [ ] Note public IP address

### 2.3 Component Deployment
- [ ] SSH to droplet
- [ ] Install Rust and build dependencies
- [ ] Clone repository and build release binaries
- [ ] Generate self-signed TLS certificates
- [ ] Create `ztna` user
- [ ] Install Intermediate Server systemd service
- [ ] Install App Connector systemd service
- [ ] Start services and verify

### 2.4 Test Backend Deployment
- [ ] Install httpbin or nginx as test backend
- [ ] Verify backend accessible locally (curl localhost:8080)

### 2.5 TLS Certificate Trust
- [ ] Copy cert to local machine
- [ ] Add to macOS Keychain (for Agent)
- [ ] Verify trust: `security verify-cert`

---

## Phase 3: Basic Relay Validation (DO + Home NAT)

### 3.1 Agent Configuration
- [ ] Update Agent config to use cloud Intermediate IP
- [ ] Configure service ID to match Connector
- [ ] Verify cert trust on macOS

### 3.2 Connectivity Tests
- [ ] Agent connects to cloud Intermediate
- [ ] QAD returns correct public IP (home NAT external)
- [ ] DATAGRAM relay works end-to-end
- [ ] HTTP backend accessible through tunnel
- [ ] Measure relay latency (expected: 50-150ms)

### 3.3 NAT Classification
- [ ] Run `pystun3` from home network
- [ ] Record NAT type in `nat-classification.md`
- [ ] Run QAD multiple times, check port consistency
- [ ] Document results

---

## Phase 4: AWS Deployment ✅ COMPLETE (Simplified EC2)

> **Status (2026-01-25):** Deployed simplified AWS setup (EC2 only, no ECS/Fargate).
> All components running on single EC2 instance for initial testing.

### 4.1 AWS Resources Created ✅
- [x] Using existing VPC: `vpc-0b18aa8ab8f451328` (masque_proxy-vpc, us-east-2)
- [x] Public subnet: `subnet-0876a3d9e3624de7f` (10.0.2.0/24)
- [x] Internet Gateway attached
- [x] Route table configured (0.0.0.0/0 → IGW)

### 4.2 Security Group ✅
- [x] Created `ztna-intermediate` (sg-0d15ab7f7b196d540)
  - [x] UDP 4433 from 0.0.0.0/0 (QUIC)
  - [x] UDP 4434 from 0.0.0.0/0 (P2P)
  - [x] TCP 22 from 0.0.0.0/0 (SSH)

### 4.3 EC2 Instance ✅
- [x] Instance: `i-021d9b1765cb49ca7` (ztna-intermediate-server)
- [x] Type: t3.micro, Ubuntu 22.04
- [x] Elastic IP: `3.128.36.92` (eipalloc-018675ba117990c48)
- [x] Private IP: `10.0.2.126`
- [x] SSH Key: `~/.ssh/hfymba.aws.pem`
- [x] Tailscale VPC access configured for SSH

### 4.4 Software Deployment ✅
- [x] Rust toolchain installed (1.93.0)
- [x] cmake and build dependencies installed
- [x] Repository cloned and binaries built (release)
- [x] systemd services created and enabled:
  - [x] `ztna-intermediate.service` (UDP 4433)
  - [x] `ztna-connector.service` (echo-service → 127.0.0.1:8080)
  - [x] `echo-server.service` (Python echo on TCP 8080)

### 4.5 Service Configuration
```bash
# SSH via Tailscale (recommended)
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126

# Service management
sudo systemctl status ztna-intermediate ztna-connector echo-server
sudo journalctl -u ztna-intermediate -f  # View logs
```

### 4.6 macOS Agent Configuration ✅
- [x] Updated `PacketTunnelProvider.swift` to use AWS IP: `3.128.36.92`
- [x] Rebuilt and deployed to /Applications/ZtnaAgent.app

### 4.7 Testing
- [ ] Verify macOS Agent connects to AWS Intermediate
- [ ] Verify full E2E relay path works
- [ ] Measure latency vs Pi k8s deployment

### 4.8 Future: ECS/Fargate (Deferred)
- [ ] ECR repositories for container images
- [ ] ECS cluster for scalable deployment
- [ ] NLB for UDP load balancing

### 4.9 Connection Resilience (Auto-Recovery)

> **Problem:** macOS Agent tunnel doesn't recover when the QUIC connection drops (server restart, network change, timeout). User must manually stop/start the VPN.
>
> **Key finding:** Rust Agent is reusable — `agent_connect()` replaces the old connection without destroy/recreate.
> No Rust changes needed. Swift-only implementation.

#### 4.9.1 Reconnection State
- [x] Add reconnection properties (pathMonitor, reconnectTimer, backoff, previousAgentState)

#### 4.9.2 NWPathMonitor
- [x] Add `startPathMonitor()` to detect WiFi → Cellular transitions
- [x] Trigger reconnect when network becomes satisfied and Agent is disconnected
- [x] Wire into `startTunnel()` and cleanup in `stopTunnel()`

#### 4.9.3 Exponential Backoff
- [x] Add `scheduleReconnect(reason:)` with 1s → 2s → 4s → 8s → 16s → 30s cap
- [x] Coalesce multiple reconnect triggers into single timer

#### 4.9.4 Reconnect Logic
- [x] Add `attemptReconnect()` — cancel old NWConnection, call `setupUdpConnection()`, reset `hasRegistered`
- [x] On `.ready`: `agent_connect()` → QUIC handshake → re-register → restart keepalive

#### 4.9.5 Detection Points
- [x] `setupUdpConnection()` `.failed` → `scheduleReconnect()`
- [x] `updateAgentState()` Closed/Error/Disconnected transitions → `scheduleReconnect()`
- [x] `sendKeepalive()` NotConnected → `scheduleReconnect()`

#### 4.9.6 State Transition Tracking
- [x] Add `previousAgentState` to prevent duplicate reconnect scheduling
- [x] Reset `reconnectBackoff` to 1.0 on successful connection

#### 4.9.7 Testing
- [x] Test: Restart intermediate server on AWS → Agent auto-recovers (1s backoff + 40ms handshake = ~1s recovery)
- [x] Test: Toggle WiFi off/on → Agent auto-recovers (LAN→WiFi, NWPathMonitor detected, 1.3s total recovery)
- [x] Test: Verify backoff logging (1s → 2s → 4s → 8s confirmed, 30s QUIC handshake timeout between attempts, reset on success)

---

## Phase 5: Home MVP Deployment (Pi k8s) ✅ MOSTLY COMPLETE

### 5.1 Kubernetes Preparation
- [x] Verify kubectl access to Pi cluster
- [x] Create `ztna` namespace
- [x] Create TLS secrets for intermediate and connector

### 5.2 Container Images (arm64)
- [x] Build intermediate-server for linux/arm64
- [x] Build app-connector for linux/arm64
- [x] Build echo-server for linux/arm64
- [x] Push to Docker Hub (public repos)
- [ ] Verify httpbin/nginx arm64 image available (not done - using echo-server)
- [ ] Verify QuakeKube arm64 support (future)

### 5.3 Deploy Intermediate Server (LOCAL, not cloud)
- [x] Create `deploy/k8s/base/intermediate-server.yaml`
- [x] Configure Cilium L2 LoadBalancer (10.0.150.205:4433)
- [x] Deploy via kustomize
- [x] Verify QUIC connections from macOS

### 5.4 Deploy App Connector
- [x] Create `deploy/k8s/base/app-connector.yaml`
- [x] Configure to connect to LOCAL intermediate (ClusterIP)
- [x] Deploy: `kubectl apply -k deploy/k8s/overlays/pi-home`
- [x] Verify pod running (CrashLoopBackOff expected - 30s idle timeout)
- [x] Check logs for connection to Intermediate

### 5.5 Deploy Echo Server (Test App)
- [x] Create `deploy/k8s/base/echo-server.yaml`
- [x] Deploy echo-server (UDP :9999)
- [x] Verify service accessible from Connector pod

### 5.6 macOS Agent E2E Test ✅ COMPLETE
- [x] Configure macOS Extension with k8s LoadBalancer IP (10.0.150.205)
- [x] Verify VPN tunnel creation (utun6)
- [x] Verify QUIC connection to intermediate
- [x] Verify packet tunneling (DATAGRAM)
- [x] **Full E2E routing to echo-server** ✅ (2026-01-25)
  - UDP traffic relayed: Agent → Intermediate → Connector → echo-server → response back
  - Test: `echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999` (routed via VPN tunnel)

### 5.7 Documentation
- [x] Create comprehensive skill guide (`deploy/k8s/k8s-deploy-skill.md`)
- [x] Update testing guide with k8s demo instructions
- [x] Document E2E test results and limitations

### 5.8 Future Items
- [ ] Deploy HTTP test app (httpbin) for HTTP testing
- [ ] Deploy QuakeKube for gaming latency testing
- [ ] Add service-based routing to complete E2E path

---

## Phase 5a: Full E2E Relay Routing ✅ REGISTRATION IMPLEMENTED

> **Problem:** macOS Agent doesn't register with a service ID, so intermediate can't route packets.
>
> **Solution (2026-01-25):** Added explicit agent registration via FFI.

### Current State (FIXED)
```
Agent → Intermediate → "No destination for relay" → Dropped
                      (Agent not registered for any service)
```
**Now:**
```
Agent registers → "I want to reach 'echo-service'" (0x10 message)
Connector registered → "I handle 'echo-service'" (0x11 message)
Agent → Intermediate → Connector → Echo Server → Response
```

### 5a.1 Add Agent Registration (Rust FFI) ✅ DONE
- [x] Add `agent_register(agent, service_id)` FFI function in `core/packet_processor/src/lib.rs`
- [x] Function sends registration DATAGRAM: `[0x10, len, service_id_bytes]`
- [x] Test with unit tests (81 tests passing)

### 5a.2 Add Agent Registration (Swift) ✅ DONE
- [x] Add `agent_register` to bridging header
- [x] Call from `PacketTunnelProvider.swift` after QUIC connection established
- [x] Use service ID "echo-service" (hardcoded for MVP, matches k8s app-connector)

### 5a.3 Test Full E2E ✅ COMPLETE (2026-01-25)
- [x] Agent registers for 'echo-service' ✅ VERIFIED IN K8S LOGS
- [x] Connector registers for 'echo-service' ✅ VERIFIED IN K8S LOGS
- [x] **Routing logic implemented:** Registry uses implicit single-service-per-connection model
  - Agent connection → registered service → Connector connection
  - No per-packet service ID needed (MVP approach)
  - See `tasks/_context/components.md` for architecture decision
- [x] **TIMING FIXED:** Connector keepalive implemented (KEEPALIVE_INTERVAL_SECS = 10)
  - Connector now sends QUIC PING frames every 10 seconds
  - Connector stays connected indefinitely (tested 20+ minutes)
  - kustomize patch added to skip gosu in k8s securityContext
- [x] **macOS Agent → k8s Intermediate → k8s App Connector → k8s Echo Server** ✅
- [x] **Relay logs confirmed:**
  ```
  [21:02:13] Received 38 bytes to relay from aa7443... (Agent)
  [21:02:13] Found destination e8780... for aa7443...
  [21:02:13] Relayed 38 bytes from aa7443... to e8780... (→ Connector)
  [21:02:13] Received 38 bytes to relay from e8780... (Connector echo response)
  [21:02:13] Found destination 176b5... for e8780...
  [21:02:13] Relayed 38 bytes from e8780... to 176b5... (→ Agent)
  ```
- [ ] Measure end-to-end latency (deferred to Phase 8)
- [x] **macOS Agent keepalive implemented** (2026-01-25) - 10s interval prevents 30s idle timeout

### Alternative: Use P2P Hole Punching for E2E
- [ ] Call `agent_start_hole_punch(agent, "echo-service")` instead
- [ ] This sends CandidateOffer with service_id (implicit registration)
- [ ] Bonus: tests P2P signaling path

---

## Phase 6: P2P Swift Integration + NAT Testing

> **Prerequisites:**
> - Phase 5a complete (Agent registration working) ✅
> - Connection resilience implemented (Phase 4.9) ✅
> - 12 P2P FFI functions implemented in Rust (81 tests passing) ✅
>
> **Implementation:** Swift-only (bridging header + PacketTunnelProvider). No Rust changes needed.
>
> **Test Topology:**
> - macOS Agent behind home router NAT
> - AWS Intermediate Server (3.128.36.92) as signaling relay
> - AWS App Connector (registered for echo-service)

### 6.1 P2P FFI Bridge
- [x] Add 12 P2P FFI declarations to PacketProcessor-Bridging-Header.h
- [x] Verify static library has P2P symbols (nm check — all 12 confirmed)
- [x] Build succeeds with new declarations

### 6.2 Hole Punch Initiation
- [x] Add P2P state properties to PacketTunnelProvider
- [x] Implement startHolePunching() — calls agent_start_hole_punch after registration
- [x] Start hole punch poll timer (50ms interval)

### 6.3 Binding Request Pump
- [x] Implement sendPendingBindingRequests() — polls agent_poll_binding_request
- [x] Create per-candidate NWConnections for binding delivery
- [x] Implement binding response receive loop → agent_process_binding_response
- [x] Poll agent_poll_hole_punch for completion

### 6.4 P2P QUIC Connection
- [x] Implement setupP2PConnection() — NWConnection to working address
- [x] Call agent_connect_p2p() on UDP ready
- [x] Implement P2P receive loop (agent_recv with P2P source)
- [x] Implement pumpP2POutbound() — agent_poll_p2p

### 6.5 Packet Routing
- [x] Check agent_get_active_path() in packet processing
- [x] Route via agent_send_datagram_p2p when direct path active
- [x] Fallback to relay on P2P send failure

### 6.6 P2P Keepalive & Path Monitoring
- [x] Implement P2P keepalive timer (15s, agent_poll_keepalive)
- [x] Log path state (agent_get_path_stats)
- [x] Detect fallback (agent_is_in_fallback)

### 6.7 Cleanup & Resilience
- [x] Clean up P2P resources in stopTunnel()
- [x] Reset P2P state in attemptReconnect()
- [x] Hole punch restarts automatically after reconnection

### 6.8 Testing ✅ COMPLETE (2026-01-31)
- [x] Build succeeds (zero errors, zero warnings)
- [x] Relay still works (no regression)
- [x] Hole punch initiates after registration (log check)
- [x] Test: macOS (home NAT) → AWS Intermediate → Connector
- [x] Document result: **direct path achieved** ✅
- [x] P2P QUIC connection established — 14 consecutive keepalive checks (3.5+ min), zero missed
- [x] Bug found & fixed: Agent `recv()` fed raw keepalive responses to `quiche::recv()` → added keepalive demux in `recv()` before QUIC routing
- [ ] If direct: tcpdump proof of non-relay traffic (deferred — functional proof via logs sufficient for MVP)
- [x] If direct: fallback test (block direct, verify relay takeover) ✅ Phase 8.5 — 180/180 packets, 0% loss, seamless per-packet failover

---

## Phase 7: Test Application Validation (HTTP through tunnel)

> **Revised 2026-02-21:** Using simple HTTP server on AWS instead of httpbin/QuakeKube.
> Second connector instance for web-app service (relay-only).

### 7.1 Deploy HTTP Server on AWS
- [x] Create /opt/ztna/www/index.html test content
- [x] Create http-server.service (python3 -m http.server 8080)
- [x] Verify HTTP server running locally on AWS

### 7.2 Deploy Second Connector (web-app)
- [x] Create ztna-connector-web.service (web-app, port 4435, relay-only)
- [x] Verify connector registers with Intermediate for 'web-app'
- [x] Verify no port conflict with existing echo-service connector

### 7.3 Update macOS Agent for Multi-Service
- [x] Add web-app service to ContentView.swift providerConfiguration
- [x] Build succeeds (zero errors)
- [x] Both services register in Agent logs after VPN connect
- [x] Test: `curl http://10.100.0.2:8080/` returns 200 with HTML content
- [x] Test: UDP echo to 10.100.0.1 still works (no regression)
- [x] Test: Multiple concurrent HTTP requests succeed
- [x] Document results

### 7.4 Deferred (Post-MVP)
- [ ] QuakeKube gaming latency testing
- [ ] httpbin advanced endpoint testing
- [ ] Per-service backend routing in Connector (currently single forward_addr)

---

## Phase 8: Performance Metrics

> **Revised 2026-02-21:** Using application-level ping RTT instead of FFI path stats
> (agent_get_path_stats returns 0ms RTT). P2P via 10.100.0.1, relay via 10.100.0.2.

### 8.1 P2P Latency (Direct Path)
- [x] ping -c 50 10.100.0.1 (P2P direct after hole punch)
- [x] Record min/avg/max/stddev: 31.154/32.644/34.492/0.760 ms

### 8.2 Relay Latency
- [x] HTTP curl -c 50 10.100.0.2 (relay-only via web-app connector)
- [x] Record min/avg/max: 64.6/76.0/165.5 ms
- [x] Calculate P2P improvement ratio: 2.3x faster

### 8.3 Sustained Stability Test
- [x] ping -c 600 10.100.0.1 (10 minutes via P2P)
- [x] Verify 0% packet loss (600/600 received)
- [x] Monitor Agent logs for path flapping (none observed)
- [x] Verify keepalive missed_keepalives stays at 0 (confirmed)
- [x] Document results

### 8.4 Deferred (Post-MVP)
- [ ] Throughput benchmarks (iperf)
- [ ] 1-hour stability test
- [ ] Fix agent_get_path_stats() 0ms RTT bug
- [ ] Load testing under concurrent connections

---

## Phase 9: Documentation

### 9.1 Deployment Guides
- [ ] `docs/deploy-digitalocean.md` - DO deployment guide
- [ ] `docs/deploy-aws.md` - AWS deployment guide
- [ ] `docs/deploy-k8s.md` - Kubernetes deployment guide

### 9.2 Test Results
- [ ] `tasks/006-cloud-deployment/nat-classification.md` - NAT types tested
- [ ] `tasks/006-cloud-deployment/results.md` - Test results summary
- [ ] `tasks/006-cloud-deployment/performance.md` - Performance metrics

### 9.3 Architecture Updates
- [ ] Update `docs/architecture.md` with cloud diagrams
- [ ] Update `tasks/_context/` with cloud deployment status

---

## Phase 10: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Review all documentation
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] Multi-region deployment (DO NYC + SF)
- [ ] Automated certificate renewal (Let's Encrypt)
- [ ] Monitoring/alerting setup (Prometheus/Grafana)
- [ ] CI/CD pipeline for cloud deployment
- [ ] Terraform infrastructure-as-code
- [ ] Different NAT environments (mobile hotspot, coffee shop WiFi)

---

---

## ✅ MVP COMPLETE (2026-02-21)

> **All 11 deliverables complete.** Post-MVP work organized into Tasks 007-012.
> See `tasks/_context/README.md` Post-MVP Roadmap for details.

### Recommended Task Sequence (Option 2 - Config First)

| Order | Task | Status | Why |
|-------|------|--------|-----|
| 1 | **AWS E2E Test** | ✅ Complete | Validated AWS deployment (2026-01-26) |
| 2 | **Config File Mechanism** | ✅ Complete | All 3 components configurable (2026-01-31) |
| 3 | **IP→Service Routing** | ✅ Complete | Multi-service routing with 0x2F protocol (2026-01-31) |
| 4 | **TCP Support in App Connector** | ✅ Complete | Userspace TCP proxy (2026-01-31) |
| 5 | **ICMP Support** | ✅ Complete | Echo Reply at Connector (2026-01-31) |
| 6 | **Return-Path (DATAGRAM→TUN)** | ✅ Complete | agent_recv_datagram + drainIncomingDatagrams (2026-01-31) |
| 7 | **Connection Resilience** | ✅ Implemented | Auto-recovery with exponential backoff + NWPathMonitor (2026-01-31) |
| 8 | **P2P NAT Testing** | ✅ Complete | Phase 6.8 — direct P2P path achieved (2026-01-31) |
| 9 | **HTTP App Validation** | ✅ Complete | Phase 7 — HTTP through tunnel + multi-service (2026-02-21) |
| 10 | **Performance Metrics** | ✅ Complete | Phase 8 — P2P 32.9ms vs Relay 76ms, 0% loss 10min (2026-02-21) |
| 11 | **P2P→Relay Failover** | ✅ Complete | Phase 8.5 — 180/180, 0% loss, per-packet fallback (2026-02-21) |
| 12 | **Admin Dashboard** | → Task 010 | UI layer on config mechanism |

### Task Details

#### Task 1: AWS E2E Test ⬜ PENDING VPN
- [ ] Connect macOS VPN to AWS intermediate (3.128.36.92:4433)
- [ ] Verify route 10.100.0.0/24 through tunnel
- [ ] Test: `echo "ZTNA-TEST-AWS" | nc -u -w1 10.100.0.1 9999`
- [ ] Verify response from echo-server
- [ ] Document latency compared to Pi k8s

#### Task 2: Config File Mechanism ✅ COMPLETE (2026-01-31)
**Purpose:** Eliminate hardcoded IPs, enable dynamic service definitions

**Deliverables:**
- [x] Define config file format (JSON chosen for cross-platform simplicity)
- [x] macOS Agent: Config via `NETunnelProviderProtocol.providerConfiguration` + UserDefaults UI
- [x] App Connector: Read `/etc/ztna/connector.json` (or `--config` flag)
- [x] Intermediate Server: Read `/etc/ztna/intermediate.json` (or `--config` flag)
- [x] Example configs: `deploy/config/{connector,intermediate,agent}.json`

**Config Schema (Draft):**
```yaml
# Agent config
intermediate_server:
  host: "3.128.36.92"
  port: 4433

services:
  - id: echo-service
    virtual_ip: 10.100.0.1
    port: 9999
    protocol: udp
  - id: web-app
    virtual_ip: 10.100.0.2
    port: 443
    protocol: tcp
```

#### Task 3: IP→Service Routing ✅ COMPLETE (2026-01-31)
**Purpose:** Route packets based on destination IP, not just service ID

**Implementation:**
- [x] Agent: Build route table from config (IP → service_id) via `services` array in providerConfiguration
- [x] Agent: Wrap packets with `[0x2F, id_len, service_id, ip_packet]` when route table populated
- [x] Intermediate: Parse 0x2F service-routed datagrams and route to correct Connector
- [x] Registry: Support multiple services per Agent connection (HashSet)
- [x] Backward compatible: non-0x2F datagrams use implicit routing
- [ ] Test with 2+ services (echo + web) - requires deploying second Connector

#### Task 4: TCP Support in App Connector ✅ COMPLETE (2026-01-31)
**Purpose:** Support web apps, databases, APIs (most enterprise traffic)

**Implementation:** Userspace TCP proxy with session tracking in `app-connector/src/main.rs`

**Deliverables:**
- [x] Parse IP header for protocol type (dispatches protocol 6 to `handle_tcp_packet()`)
- [x] Handle TCP packets: SYN→connect, data→forward, FIN→close, RST→reset
- [x] Manage TCP state/sessions over QUIC (`TcpSession` struct, `tcp_sessions` HashMap)
- [x] Build TCP/IP response packets with proper checksums (`build_tcp_packet()`, `tcp_checksum()`)
- [x] Poll backend TcpStreams for return data (`process_tcp_sessions()`)
- [x] Session cleanup on idle timeout (120s)
- [ ] Test with HTTP backend (curl through tunnel) - requires deployment
- [x] Document TCP over QUIC design (state.md Phase 4.4)

#### Task 5: ICMP Support ✅ COMPLETE (2026-01-31)
**Purpose:** Enable ping/traceroute through tunnel for diagnostics

**Implementation:** Connector generates Echo Reply directly (no backend forwarding needed)

**Deliverables:**
- [x] Handle ICMP packets (protocol 1) in `forward_to_local()` dispatch
- [x] Parse Echo Request (type 8), generate Echo Reply (type 0) with `build_icmp_reply()`
- [x] Preserve identifier, sequence number, and payload data
- [x] Proper ICMP checksum calculation via `icmp_checksum()`
- [ ] Test: `ping 10.100.0.1` through tunnel - requires deployment
- [x] Document ICMP handling (state.md Phase 4.5)

---

## Future: Dynamic Service Configuration (Post-Cloud Deployment)

> **Critical Requirement:** After successful cloud deployment validation, eliminate all hardcoded IPs
> and move to a dynamic, configurable system.

### Problem Statement

Currently hardcoded values that must be eliminated:
- **macOS Agent:** `serverHost = "3.128.36.92"` (intermediate server address)
- **macOS Agent:** `10.100.0.0/24` (routed service range)
- **macOS Agent:** `targetServiceId = "echo-service"`
- **App Connector:** `--server`, `--service`, `--forward` CLI args
- **Intermediate Server:** Port 4433 (less critical but still hardcoded)

### Required Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                      ZTNA Control Plane (Future)                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌─────────────────────┐         ┌─────────────────────────────────────────┐   │
│  │   Policy Service    │         │         Service Registry                 │   │
│  │   (defines what     │◄───────►│   - echo-service → 10.100.0.1:9999      │   │
│  │    agents can       │         │   - web-app → 10.100.0.2:443            │   │
│  │    access)          │         │   - quake-server → 10.100.0.3:27960     │   │
│  └─────────────────────┘         └─────────────────────────────────────────┘   │
│            │                                    │                               │
│            │ Policy Push                        │ Service Discovery             │
│            ▼                                    ▼                               │
│  ┌─────────────────────┐         ┌─────────────────────────────────────────┐   │
│  │   macOS Agent       │         │         App Connector                    │   │
│  │   - Receives policy │         │   - Registers services dynamically       │   │
│  │   - Updates routes  │         │   - Reports health/status                │   │
│  │   - Knows services  │         │   - Receives backend config              │   │
│  └─────────────────────┘         └─────────────────────────────────────────┘   │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Implementation Phases

#### Phase A: Configuration Files
- [ ] macOS Agent: Read config from `~/Library/Application Support/ZtnaAgent/config.json`
  ```json
  {
    "intermediate": { "host": "3.128.36.92", "port": 4433 },
    "services": [
      { "id": "echo-service", "virtualIp": "10.100.0.1", "port": 9999 }
    ]
  }
  ```
- [ ] App Connector: Read config from `/etc/ztna/connector.yaml`
- [ ] Intermediate Server: Read config from `/etc/ztna/intermediate.yaml`

#### Phase B: MDM/Provisioning Support (macOS)
- [ ] Support MDM configuration profiles for enterprise deployment
- [ ] Support UserDefaults for manual configuration via UI
- [ ] Support environment-based config (dev/staging/prod)

#### Phase C: Dynamic Service Discovery
- [ ] Control plane API for service registration
- [ ] Agent polls for policy updates
- [ ] Connector announces services on startup
- [ ] Intermediate routes based on live registry

#### Phase D: Virtual IP Allocation
- [ ] Automatic virtual IP assignment for new services
- [ ] DNS-based service discovery (e.g., `echo-service.ztna.local` → `10.100.0.1`)
- [ ] Conflict detection and resolution

### Key Design Decisions (To Be Made)

| Decision | Options | Notes |
|----------|---------|-------|
| Config format | JSON, YAML, plist, protobuf | JSON likely easiest cross-platform |
| Config delivery | File, API, MDM, DNS | May need multiple for different scenarios |
| Service discovery | Static, mDNS, custom API | Custom API most flexible |
| Virtual IP range | 10.100.0.0/24, 100.64.0.0/10 | Must not conflict with real networks |

### Why This Matters

Without dynamic configuration:
- Every deployment requires code changes and rebuilds
- Cannot add new services without Agent updates
- Cannot scale to multiple environments (dev/staging/prod)
- Cannot support enterprise multi-tenant deployments
- P2P hole punching config is static

---

## Open Questions (From Oracle)

1. **Can Connector be placed behind a second real NAT?**
   - Answer: Yes - using Raspberry Pi k8s behind home NAT ✅

2. **What UDP port does the P2P listener use?**
   - Current: ephemeral (0.0.0.0:0)
   - Action: Add `--p2p-listen-port 4434` flag ⬜

3. **Do Agent/Connector emit "path = direct/relay" signal?**
   - Current: Not explicitly
   - Action: Add logging in Phase 6 ⬜

---

## Quick Reference

### DigitalOcean Commands
```bash
# Create droplet
doctl compute droplet create ztna-relay --image ubuntu-24-04-x64 --size s-1vcpu-1gb --region nyc1

# Get IP
doctl compute droplet list --format ID,Name,PublicIPv4

# Create firewall
doctl compute firewall create --name ztna-fw --inbound-rules "protocol:udp,ports:4433-4434,address:0.0.0.0/0"
```

### AWS Commands
```bash
# Create VPC
aws ec2 create-vpc --cidr-block 10.0.0.0/16 --region us-east-2

# Create ECR repo
aws ecr create-repository --repository-name ztna/app-connector --region us-east-2

# Create ECS cluster
aws ecs create-cluster --cluster-name ztna-cluster --region us-east-2
```

### Kubernetes Commands
```bash
# Deploy to Pi cluster
kubectl apply -f deploy/k8s/ -n ztna

# Check pods
kubectl get pods -n ztna

# View connector logs
kubectl logs -f deployment/app-connector -n ztna
```
