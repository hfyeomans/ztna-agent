# Implementation Plan: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** 004 (E2E Testing)
**Last Updated:** 2026-01-19

---

## Goal

Deploy Intermediate Server and App Connector to cloud infrastructure to:
1. Test Agent behavior behind real NAT environments
2. Validate QAD (QUIC Address Discovery) with actual public IPs
3. Prepare infrastructure for production deployment
4. Enable Task 005 (P2P Hole Punching) testing with real NATs

---

## Branching Workflow

```bash
# Before starting:
git checkout master
git pull origin master
git checkout -b feature/006-cloud-deployment

# While working:
git add . && git commit -m "006: descriptive message"

# When complete:
git push -u origin feature/006-cloud-deployment
# Create PR → Review → Merge to master
```

---

## Phase 1: Infrastructure Selection

### 1.1 Cloud Provider Evaluation

| Provider | Pros | Cons | Cost Estimate |
|----------|------|------|---------------|
| **DigitalOcean** | Simple, fast, good docs | Limited regions | $4-6/mo |
| AWS Lightsail | AWS ecosystem, predictable | AWS complexity | $3.50-5/mo |
| Vultr | Cheap, global, fast | Smaller community | $2.50-5/mo |
| GCP | Free tier, good networking | Complex pricing | Free-$5/mo |

### 1.2 Requirements

**Minimum VM Specs:**
- 1 vCPU
- 512MB RAM (1GB preferred)
- 10GB SSD
- Public IPv4 address
- Ubuntu 22.04 or 24.04 LTS

**Network Requirements:**
- UDP port 4433 inbound (QUIC)
- Outbound internet access
- Static public IP (or stable DHCP)

### 1.3 Decision

TBD - Select provider based on:
- Cost
- NAT/network configuration options
- Ease of automation
- Regional availability

---

## Phase 2: Infrastructure Setup

### 2.1 VM Provisioning

- [ ] Create cloud account (if needed)
- [ ] Provision VM with required specs
- [ ] Configure firewall/security group
  - Allow UDP 4433 inbound
  - Allow SSH (22) from admin IPs only
- [ ] Set up SSH access
- [ ] Install required packages

### 2.2 Install Script

```bash
#!/bin/bash
# install-dependencies.sh

# Update system
apt update && apt upgrade -y

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Install build dependencies
apt install -y build-essential pkg-config libssl-dev

# Clone repository
git clone https://github.com/YOUR_ORG/ztna-agent.git
cd ztna-agent

# Build components
cargo build --release -p intermediate-server
cargo build --release -p app-connector
```

### 2.3 TLS Certificates

**Option A: Self-Signed (MVP)**
```bash
# Generate self-signed cert (development)
openssl req -x509 -newkey rsa:4096 \
  -keyout key.pem -out cert.pem \
  -days 365 -nodes \
  -subj "/CN=intermediate.example.com"
```

**Option B: Let's Encrypt (Production)**
```bash
# Install certbot
apt install -y certbot

# Get certificate (requires domain)
certbot certonly --standalone -d intermediate.example.com

# Certificates at:
# /etc/letsencrypt/live/intermediate.example.com/fullchain.pem
# /etc/letsencrypt/live/intermediate.example.com/privkey.pem
```

---

## Phase 3: Component Deployment

### 3.1 Intermediate Server

**Systemd Service:**
```ini
# /etc/systemd/system/intermediate-server.service
[Unit]
Description=ZTNA Intermediate Server
After=network.target

[Service]
Type=simple
User=ztna
ExecStart=/opt/ztna/intermediate-server \
  --listen 0.0.0.0:4433 \
  --cert /opt/ztna/cert.pem \
  --key /opt/ztna/key.pem
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Deployment Steps:**
- [ ] Copy binary to `/opt/ztna/`
- [ ] Copy certificates
- [ ] Create `ztna` user
- [ ] Install systemd service
- [ ] Enable and start service
- [ ] Verify listening on port 4433

### 3.2 App Connector

**Systemd Service:**
```ini
# /etc/systemd/system/app-connector.service
[Unit]
Description=ZTNA App Connector
After=network.target intermediate-server.service

[Service]
Type=simple
User=ztna
ExecStart=/opt/ztna/app-connector \
  --intermediate 127.0.0.1:4433 \
  --service-id "test-service" \
  --local-addr 127.0.0.1:8080
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Deployment Steps:**
- [ ] Copy binary to `/opt/ztna/`
- [ ] Install systemd service
- [ ] Enable and start service
- [ ] Verify connection to Intermediate

### 3.3 Test Service

**Simple UDP Echo Server:**
```bash
# Install socat
apt install -y socat

# Run UDP echo server
socat UDP4-LISTEN:8080,fork EXEC:/bin/cat
```

Or deploy a simple test application.

---

## Phase 4: Connectivity Testing

### 4.1 Local Validation (on cloud VM)

- [ ] Verify Intermediate Server running
- [ ] Verify App Connector connected
- [ ] Test QAD response (should show public IP)
- [ ] Test local echo server

### 4.2 Remote Agent Testing

- [ ] Configure Agent to connect to cloud Intermediate
- [ ] Verify Agent receives QAD with correct public IP
- [ ] Test DATAGRAM relay through cloud
- [ ] Measure latency (Agent → Cloud → Connector → Service)

### 4.3 NAT Scenarios

| Scenario | Test |
|----------|------|
| Home NAT | Agent behind home router |
| Mobile NAT | Agent on mobile hotspot |
| Corporate NAT | Agent behind enterprise NAT (if available) |

---

## Phase 5: Automation (Optional)

### 5.1 Infrastructure as Code

**Terraform Example (DigitalOcean):**
```hcl
resource "digitalocean_droplet" "ztna" {
  name   = "ztna-intermediate"
  region = "nyc1"
  size   = "s-1vcpu-512mb"
  image  = "ubuntu-24-04-x64"

  ssh_keys = [data.digitalocean_ssh_key.default.id]
}

resource "digitalocean_firewall" "ztna" {
  name = "ztna-firewall"

  inbound_rule {
    protocol         = "udp"
    port_range       = "4433"
    source_addresses = ["0.0.0.0/0"]
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = "22"
    source_addresses = ["YOUR_IP/32"]
  }
}
```

### 5.2 Ansible Playbook

```yaml
# deploy-ztna.yml
- hosts: ztna
  become: yes
  tasks:
    - name: Install dependencies
      apt:
        name: [build-essential, pkg-config, libssl-dev]
        state: present

    - name: Copy binaries
      copy:
        src: "{{ item }}"
        dest: /opt/ztna/
        mode: '0755'
      loop:
        - target/release/intermediate-server
        - target/release/app-connector

    - name: Install systemd services
      template:
        src: "{{ item }}.service.j2"
        dest: /etc/systemd/system/{{ item }}.service
      loop:
        - intermediate-server
        - app-connector
      notify: reload systemd
```

---

## Phase 6: Documentation

- [ ] Document cloud deployment steps
- [ ] Create troubleshooting guide
- [ ] Document firewall requirements
- [ ] Update architecture docs with cloud deployment

---

## Success Criteria

1. [ ] Intermediate Server running on cloud VM with public IP
2. [ ] App Connector connected and receiving DATAGRAMs
3. [ ] Agent connects from behind NAT
4. [ ] QAD correctly reports Agent's public IP
5. [ ] UDP traffic flows end-to-end through cloud relay
6. [ ] Latency acceptable (< 200ms for intercontinental)

---

## Deployment Topology Options

### Option A: Single VM (Simple)

```
┌─────────────────────────────────────────┐
│           Cloud VM (1.2.3.4)            │
│                                          │
│  ┌──────────────────────────────────┐   │
│  │    Intermediate Server :4433     │   │
│  └──────────────────────────────────┘   │
│                  │                       │
│                  │ localhost             │
│                  ▼                       │
│  ┌──────────────────────────────────┐   │
│  │    App Connector                 │   │
│  └──────────────────────────────────┘   │
│                  │                       │
│                  │ localhost             │
│                  ▼                       │
│  ┌──────────────────────────────────┐   │
│  │    Test Service :8080            │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

### Option B: Separate VMs (Production-like)

```
┌─────────────────────┐      ┌─────────────────────┐
│  Intermediate VM    │      │   Connector VM      │
│  (1.2.3.4)          │      │   (5.6.7.8)         │
│                     │      │                     │
│  ┌───────────────┐  │      │  ┌───────────────┐  │
│  │ Intermediate  │  │◄────►│  │ App Connector │  │
│  │ Server :4433  │  │ QUIC │  └───────┬───────┘  │
│  └───────────────┘  │      │          │          │
└─────────────────────┘      │          ▼          │
                             │  ┌───────────────┐  │
                             │  │ Test Service  │  │
                             │  └───────────────┘  │
                             └─────────────────────┘
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Cloud costs | Use small VMs, shut down when not testing |
| IP changes | Use static IP or update Agent config |
| Certificate issues | Start with self-signed, add LE later |
| Firewall blocks | Document all required ports clearly |
| Performance variance | Test multiple regions/times |

---

## References

- [DigitalOcean Rust Deployment](https://www.digitalocean.com/community/tutorials/how-to-deploy-rust-web-app)
- [Let's Encrypt Certbot](https://certbot.eff.org/)
- [Terraform DigitalOcean Provider](https://registry.terraform.io/providers/digitalocean/digitalocean/latest/docs)
- [Ansible Getting Started](https://docs.ansible.com/ansible/latest/getting_started/index.html)
