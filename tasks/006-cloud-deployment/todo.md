# TODO: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** Task 004 (E2E Relay Testing)
**Last Updated:** 2026-01-19

---

## Prerequisites

- [ ] Task 004 (E2E Relay Testing) complete and merged
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

## Phase 7: Documentation

- [ ] Document deployment steps in README
- [ ] Create troubleshooting guide
- [ ] Document firewall/network requirements
- [ ] Update architecture docs

---

## Phase 8: Automation (Stretch)

- [ ] Create Terraform configuration (optional)
- [ ] Create Ansible playbook (optional)
- [ ] Document IaC usage

---

## Phase 9: PR & Merge

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
