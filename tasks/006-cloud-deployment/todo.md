# TODO: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** Task 004 (E2E Relay Testing), Task 005 (P2P Hole Punching - protocol implementation)
**Last Updated:** 2026-01-20

---

## Prerequisites

- [x] Task 004 (E2E Relay Testing) complete and merged
- [ ] Task 005 (P2P Hole Punching) protocol implementation complete
- [ ] Cloud provider account set up
- [ ] Create feature branch: `git checkout -b feature/006-cloud-deployment`

---

## Phase 1: Infrastructure Selection

- [ ] Evaluate cloud providers (DigitalOcean, AWS, Vultr, GCP)
- [ ] Choose provider based on cost/features/regions
- [ ] Create cloud account (if needed)
- [ ] Document provider choice in research.md

---

## Phase 2: VM Provisioning

- [ ] Provision cloud VM with required specs
  - [ ] 1 vCPU, 512MB-1GB RAM
  - [ ] Public IPv4 address
  - [ ] Ubuntu 22.04 or 24.04 LTS
- [ ] Configure firewall/security group
  - [ ] Allow UDP 4433 inbound (QUIC)
  - [ ] Allow SSH (22) from admin IPs only
- [ ] Set up SSH key access
- [ ] Install required packages (Rust, build tools)

---

## Phase 3: TLS Certificates

- [ ] Generate TLS certificates
  - [ ] Option A: Self-signed for MVP
  - [ ] Option B: Let's Encrypt for domain (if available)
- [ ] Place certificates on VM

---

## Phase 4: Component Deployment

### 4.1 Intermediate Server
- [ ] Build release binary: `cargo build --release -p intermediate-server`
- [ ] Copy binary to cloud VM
- [ ] Create systemd service file
- [ ] Create `ztna` user for service
- [ ] Enable and start service
- [ ] Verify listening on UDP 4433
- [ ] Check logs for errors

### 4.2 App Connector
- [ ] Build release binary: `cargo build --release -p app-connector`
- [ ] Copy binary to cloud VM
- [ ] Create systemd service file
- [ ] Enable and start service
- [ ] Verify connection to Intermediate
- [ ] Check logs for QAD response

### 4.3 Test Service
- [ ] Deploy simple UDP echo server
- [ ] Verify echo server is reachable from Connector

---

## Phase 5: Local Validation (on cloud VM)

- [ ] Verify Intermediate Server is running
- [ ] Verify App Connector is connected
- [ ] Test echo service locally (nc or similar)
- [ ] Check firewall rules are correct

---

## Phase 6: Remote Agent Testing

### 6.1 Agent Configuration
- [ ] Update Agent to connect to cloud Intermediate IP
- [ ] Use matching self-signed cert (or skip verification for MVP)

### 6.2 Basic Connectivity
- [ ] Verify Agent connects to cloud Intermediate
- [ ] Verify Agent receives QAD with correct public IP
- [ ] Test DATAGRAM relay: Agent → Cloud → Connector → Echo

### 6.3 NAT Testing
- [ ] Test from home network (behind home NAT)
- [ ] Test from mobile hotspot (carrier NAT)
- [ ] Document observed behaviors

### 6.4 Performance Testing
- [ ] Measure round-trip latency (Agent to Echo)
- [ ] Compare to local-only testing
- [ ] Document network overhead

---

## Phase 7: P2P Hole Punching Validation (From Task 005)

> **Note:** Task 005 implements P2P protocol locally. This phase validates it works with real NATs.

### 7.1 NAT Type Testing Matrix

| NAT Type | Hole Punching | Priority | Test Method |
|----------|---------------|----------|-------------|
| Full Cone | Easy | P1 | Home router (most common) |
| Restricted Cone | Medium | P1 | Some home routers |
| Port Restricted | Medium | P1 | Some enterprise routers |
| Symmetric | Hard (relay fallback) | P2 | Carrier-grade NAT, enterprise |

- [ ] Test Full Cone NAT (home router)
- [ ] Test Restricted Cone NAT
- [ ] Test Port Restricted Cone NAT
- [ ] Test Symmetric NAT (carrier-grade/enterprise)
- [ ] Document success/failure rates per NAT type

### 7.2 Connection Priority Validation

```
Expected Priority Order:
1. Direct LAN (same network) → lowest latency
2. Direct WAN (hole punching) → moderate latency
3. Relay (via Intermediate) → highest latency (fallback)
```

- [ ] Same network: Agent and Connector on same cloud VPC
- [ ] Different networks: Agent behind home NAT, Connector on cloud
- [ ] Relay fallback: Block direct path, verify relay works
- [ ] Measure and compare latency for each path type

### 7.3 Hole Punching Protocol Tests

| Test | Description | Expected Result |
|------|-------------|-----------------|
| Address exchange | Candidates exchanged via Intermediate | Both sides receive peer candidates |
| Simultaneous open | Both sides send UDP simultaneously | NAT mappings created, packets pass |
| Direct QUIC connection | Agent connects directly to Connector | New QUIC connection established |
| Path selection | Compare direct vs relay RTT | Direct path selected when faster |
| Fallback to relay | Block direct path | Traffic continues via relay |

- [ ] Verify address exchange via Intermediate works across NAT
- [ ] Verify simultaneous UDP open creates NAT mappings
- [ ] Verify direct QUIC connection establishes after hole punch
- [ ] Verify path selection prefers direct when available
- [ ] Verify fallback to relay when hole punching fails

### 7.4 NAT Behavior Observation

Document observed behaviors for each NAT type tested:

- [ ] QAD reflexive address accuracy
- [ ] Port mapping consistency (same port for multiple destinations?)
- [ ] Binding timeout (how long until NAT mapping expires?)
- [ ] Filtering behavior (IP-restricted vs port-restricted)

### 7.5 Symmetric NAT Handling

- [ ] Detect symmetric NAT (different reflexive port per destination)
- [ ] Verify relay fallback when hole punching fails
- [ ] Test port prediction (if implemented in Task 005)
- [ ] Document success/failure rates for symmetric NAT

### 7.6 QUIC Connection Migration (If Applicable)

> Note: P2P uses NEW QUIC connection, not path migration. But test if relevant.

- [ ] Verify direct QUIC connection data flow
- [ ] Verify connection survives brief network interruption
- [ ] Test graceful transition between paths

---

## Phase 8: Documentation

- [ ] Document deployment steps in README
- [ ] Create troubleshooting guide
- [ ] Document firewall/network requirements
- [ ] Update architecture docs
- [ ] Document P2P validation results and NAT compatibility

---

## Phase 9: Automation (Stretch)

- [ ] Create Terraform configuration (optional)
- [ ] Create Ansible playbook (optional)
- [ ] Document IaC usage

---

## Phase 10: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] Multi-region deployment
- [ ] Automated certificate renewal
- [ ] Monitoring/alerting setup
- [ ] Load testing with cloud infrastructure
- [ ] CI/CD pipeline for cloud deployment

---

## Deferred P2P Items (From Task 005)

> These items require real NAT testing and were deferred from Task 005's local PoC.

- [ ] Reflexive candidate validation (requires real NAT)
- [ ] Port prediction for symmetric NAT
- [ ] IPv6 P2P support
- [ ] UPnP/NAT-PMP port mapping
- [ ] Mobile handoff (WiFi → Cellular)
- [ ] ICE restart on path failure
- [ ] Multiple simultaneous paths (QUIC multipath)
