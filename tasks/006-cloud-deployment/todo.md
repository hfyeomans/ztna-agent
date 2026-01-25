# TODO: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** Task 004, Task 005, Task 005a
**Last Updated:** 2026-01-25

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
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`: `serverHost = "10.0.150.205"`
- `deploy/k8s/overlays/pi-home/kustomization.yaml`: `io.cilium/lb-ipam-ips: "10.0.150.205"`
- Test commands use `1.1.1.1` which has no semantic meaning

**Required Changes for Cloud Deployment:**
- [ ] macOS Agent: Read server address from configuration (UserDefaults, plist, or MDM profile)
- [ ] K8s overlays: Use environment-specific kustomization files (`pi-home/`, `do-nyc1/`, `aws-us-east-2/`)
- [ ] Test scripts: Use a service-specific virtual IP (e.g., `100.64.0.100` for echo-service) instead of `1.1.1.1`
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

## Phase 4: AWS VPC Deployment (Production-like)

### 4.1 VPC Infrastructure
- [ ] Create VPC (10.0.0.0/16) in us-east-2
- [ ] Create public subnet (10.0.1.0/24)
- [ ] Create private subnet (10.0.2.0/24)
- [ ] Create Internet Gateway
- [ ] Create NAT Gateway
- [ ] Configure route tables

### 4.2 Security Groups
- [ ] Create `ztna-intermediate-sg`
  - [ ] UDP 4433 from 0.0.0.0/0
  - [ ] TCP 22 from admin IP
- [ ] Create `ztna-connector-sg`
  - [ ] UDP 4434 from 0.0.0.0/0
  - [ ] All outbound allowed
- [ ] Create `ztna-backend-sg`
  - [ ] TCP 8080 from connector SG
  - [ ] TCP 27960 from connector SG (Quake)

### 4.3 EC2 for Intermediate Server
- [ ] Launch t3.micro with Ubuntu 24.04
- [ ] Assign Elastic IP
- [ ] Install and configure Intermediate Server
- [ ] Verify listening on UDP 4433

### 4.4 ECR Repositories
- [ ] Create `ztna/app-connector` repository
- [ ] Create `ztna/http-app` repository
- [ ] Create `ztna/quakekube` repository

### 4.5 Container Images
- [ ] Build app-connector for linux/amd64
- [ ] Push to ECR
- [ ] Build/tag httpbin image
- [ ] Push to ECR

### 4.6 ECS Cluster and Services
- [ ] Create ECS Fargate cluster
- [ ] Create task definition for App Connector
  - [ ] UDP port 4434
  - [ ] TCP health check
- [ ] Create task definition for HTTP App
- [ ] Create NLB with UDP listeners
- [ ] Create target groups
- [ ] Deploy services

### 4.7 AWS Validation
- [ ] Verify Intermediate accessible from internet
- [ ] Verify NLB routes to Fargate tasks
- [ ] Test relay through AWS infrastructure
- [ ] Compare latency with DigitalOcean

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
  - Test: `echo "ZTNA-TEST" | nc -u -w1 1.1.1.1 9999` (routed via VPN tunnel)

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
- [ ] **Note:** macOS Agent also needs keepalive (30s timeout observed)

### Alternative: Use P2P Hole Punching for E2E
- [ ] Call `agent_start_hole_punch(agent, "echo-service")` instead
- [ ] This sends CandidateOffer with service_id (implicit registration)
- [ ] Bonus: tests P2P signaling path

---

## Phase 6: NAT-to-NAT Hole Punching Validation

> **Prerequisites:**
> - Phase 5a complete (Agent registration working)
> - OR use hole punching CandidateOffer which implicitly registers
>
> **Test Topology:**
> - macOS Agent behind home router NAT
> - App Connector in k8s (also behind home NAT = hairpin NAT)
> - OR App Connector on different network/NAT (e.g., mobile hotspot)

### 6.1 Direct Path Verification Setup
- [ ] Add path selection logging to Agent
  ```rust
  info!("Path selected: {} (peer: {})", path_type, peer_addr);
  ```
- [ ] Add path selection logging to Connector
- [ ] Rebuild and redeploy components

### 6.2 Hole Punch Tests
- [ ] Start Agent on macOS (home WiFi)
- [ ] Start Connector on Pi k8s (home network)
- [ ] Both connect to cloud Intermediate
- [ ] Verify candidate exchange completes
- [ ] Check logs for "Path selected: DIRECT"

### 6.3 Direct Path Proof
- [ ] **Log proof:** Grep logs for "DIRECT" path selection
- [ ] **Packet capture proof:** Run tcpdump on Mac
  ```bash
  tcpdump -i en0 udp and not host <intermediate-ip>
  ```
  - Verify packets going to Connector's IP, not Intermediate
- [ ] **Relay disable proof:**
  - SSH to Intermediate, stop DATAGRAM relay
  - Verify traffic continues (if truly direct)
  - Restart relay after test

### 6.4 Fallback Test
- [ ] Block direct UDP between Agent and Connector (iptables or firewall)
- [ ] Verify traffic switches to relay
- [ ] Measure relay latency vs direct latency
- [ ] Unblock and verify direct path resumes

### 6.5 Hairpin NAT Test
- [ ] Document home router model
- [ ] If hairpin fails, document limitation
- [ ] Test with mobile hotspot (different NAT) if available

---

## Phase 7: Test Application Validation

### 7.1 HTTP App Testing
- [ ] Configure Agent to route to http-app service
- [ ] Test basic connectivity: `curl http://<tunnel>/get`
- [ ] Test latency endpoint: `curl http://<tunnel>/delay/1`
- [ ] Test POST echo: `curl -X POST http://<tunnel>/post -d "test"`
- [ ] Measure end-to-end latency
- [ ] Document results

### 7.2 QuakeKube Testing
- [ ] Configure Connector to route to QuakeKube service
- [ ] Connect browser to QuakeJS web client
- [ ] Verify game loads and connects
- [ ] Measure in-game ping
- [ ] Play test game, note any lag
- [ ] Document playability results:
  - [ ] Ping < 150ms? (Acceptable)
  - [ ] Ping < 80ms? (Good)
  - [ ] Visible lag during movement?
  - [ ] Packet loss observed?

### 7.3 Native Quake Client (Optional)
- [ ] Install native Quake 3 client
- [ ] Connect via UDP to QuakeKube
- [ ] Compare latency with WebSocket client

---

## Phase 8: Performance Metrics

### 8.1 Latency Testing
- [ ] Measure relay path latency (Agent → Intermediate → Connector)
- [ ] Measure direct path latency (Agent → Connector P2P)
- [ ] Calculate latency improvement from direct path
- [ ] Test under load (multiple requests/packets)

### 8.2 Throughput Testing
- [ ] Measure sustained throughput via relay
- [ ] Measure sustained throughput via direct path
- [ ] Compare with baseline (non-tunneled)

### 8.3 Stability Testing
- [ ] Run continuous traffic for 1 hour
- [ ] Monitor for disconnections
- [ ] Monitor for path flapping (direct ↔ relay)
- [ ] Check keepalive effectiveness

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
