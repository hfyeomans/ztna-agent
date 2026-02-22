# AWS Deployment Skill Guide

A comprehensive guide covering all commands, architecture, and operations for deploying ZTNA to AWS EC2.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [AWS Resources](#aws-resources)
3. [Prerequisites](#prerequisites)
4. [SSH Access](#ssh-access)
5. [Service Management](#service-management)
6. [Troubleshooting](#troubleshooting)
7. [Testing](#testing)
8. [Human Demo Guide](#human-demo-guide)
9. [Cleanup](#cleanup)

---

## Architecture Overview

### Network Topology

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                        AWS EC2 Deployment (us-east-2)                            │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌─────────────┐                    ┌───────────────────────────────────────┐   │
│  │   macOS     │                    │           AWS VPC                      │   │
│  │   Agent     │                    │      (vpc-0b18aa8ab8f451328)          │   │
│  │             │                    │           10.0.0.0/16                 │   │
│  │  (Anywhere  │◄───── QUIC ───────►│                                       │   │
│  │   Internet) │   UDP:4433         │  ┌──────────────────────────────────┐ │   │
│  │             │                    │  │     EC2 Instance (t3.micro)       │ │   │
│  └─────────────┘                    │  │     i-021d9b1765cb49ca7           │ │   │
│                                      │  │                                   │ │   │
│                                      │  │  Elastic IP: 3.128.36.92         │ │   │
│                                      │  │  Private IP: 10.0.2.126          │ │   │
│                                      │  │                                   │ │   │
│                                      │  │  ┌────────────────────────────┐  │ │   │
│                                      │  │  │ intermediate-server :4433  │  │ │   │
│                                      │  │  │         (systemd)          │  │ │   │
│                                      │  │  └────────────────────────────┘  │ │   │
│                                      │  │              │                    │ │   │
│                                      │  │              │ localhost          │ │   │
│                                      │  │              ▼                    │ │   │
│                                      │  │  ┌────────────────────────────┐  │ │   │
│                                      │  │  │ app-connector :4433        │  │ │   │
│                                      │  │  │ (echo-service → :8080)     │  │ │   │
│                                      │  │  └────────────────────────────┘  │ │   │
│                                      │  │              │                    │ │   │
│                                      │  │              │ localhost          │ │   │
│                                      │  │              ▼                    │ │   │
│                                      │  │  ┌────────────────────────────┐  │ │   │
│                                      │  │  │ echo-server :8080 (python) │  │ │   │
│                                      │  │  └────────────────────────────┘  │ │   │
│                                      │  └──────────────────────────────────┘ │   │
│                                      └───────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Component Summary

| Component | Port | Protocol | Purpose |
|-----------|------|----------|---------|
| intermediate-server | 4433 | UDP/QUIC | QUIC relay, QAD, signaling |
| app-connector | ephemeral | UDP/QUIC | Connects to intermediate, forwards traffic |
| echo-server | 8080 | TCP | Test service for E2E validation |

---

## AWS Resources

### Current Infrastructure

| Resource | ID | Details |
|----------|-----|---------|
| VPC | vpc-0b18aa8ab8f451328 | masque_proxy-vpc, 10.0.0.0/16 |
| Subnet | subnet-0876a3d9e3624de7f | Public, 10.0.2.0/24, us-east-2a |
| Internet Gateway | igw-0d68aaa0c6205968c | Attached to VPC |
| Security Group | sg-0d15ab7f7b196d540 | ztna-intermediate |
| EC2 Instance | i-021d9b1765cb49ca7 | ztna-intermediate-server |
| Elastic IP | 3.128.36.92 | eipalloc-018675ba117990c48 |

### Security Group Rules (ztna-intermediate)

| Direction | Protocol | Port | Source | Description |
|-----------|----------|------|--------|-------------|
| Inbound | UDP | 4433 | 0.0.0.0/0 | QUIC (Intermediate) |
| Inbound | UDP | 4434 | 0.0.0.0/0 | P2P (future) |
| Inbound | TCP | 22 | 0.0.0.0/0 | SSH |
| Outbound | All | All | 0.0.0.0/0 | Default |

---

## Prerequisites

### Required Tools

```bash
# AWS CLI (authenticated)
aws --version
aws sts get-caller-identity

# SSH key
ls -la ~/.ssh/hfymba.aws.pem
```

### Tailscale VPC Access (Recommended)

Direct SSH to the public IP may not work from some networks. Use Tailscale for reliable access:

```bash
# Verify Tailscale access to VPC
ping 10.0.2.126  # Private IP should be reachable
```

---

## SSH Access

### Via Tailscale (Recommended)

```bash
# SSH using private IP through Tailscale
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126
```

### Via Public IP (If Available)

```bash
# SSH using Elastic IP
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@3.128.36.92
```

### SSH Tips

```bash
# After instance restart, update known hosts with ssh-keyscan
ssh-keyscan -H 10.0.2.126 >> ~/.ssh/known_hosts 2>/dev/null
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126

# With keepalive for long sessions
ssh -i ~/.ssh/hfymba.aws.pem -o ServerAliveInterval=60 ubuntu@10.0.2.126
```

> **Security warning:** Avoid using `StrictHostKeyChecking=no` as it disables
> host key verification and is vulnerable to man-in-the-middle attacks. Use
> `ssh-keyscan` to update known hosts after instance restarts instead.

---

## Service Management

### Systemd Services

All ZTNA components run as systemd services:

| Service | Binary | Configuration |
|---------|--------|---------------|
| ztna-intermediate | intermediate-server | 4433 cert.pem key.pem |
| ztna-connector | app-connector | --server 127.0.0.1:4433 --service echo-service --forward 127.0.0.1:8080 |
| echo-server | Python script | TCP 8080 |

### Service Commands

```bash
# Check all service status
sudo systemctl status ztna-intermediate ztna-connector echo-server --no-pager

# View service logs (follow mode)
sudo journalctl -u ztna-intermediate -f
sudo journalctl -u ztna-connector -f
sudo journalctl -u echo-server -f

# Restart services
sudo systemctl restart ztna-intermediate ztna-connector

# Stop all services
sudo systemctl stop ztna-intermediate ztna-connector echo-server

# Start all services (in order)
sudo systemctl start echo-server
sudo systemctl start ztna-intermediate
sudo systemctl start ztna-connector

# Enable services at boot
sudo systemctl enable echo-server ztna-intermediate ztna-connector
```

### Service File Locations

```bash
# View service configurations
cat /etc/systemd/system/ztna-intermediate.service
cat /etc/systemd/system/ztna-connector.service
cat /etc/systemd/system/echo-server.service

# After editing service files
sudo systemctl daemon-reload
```

---

## Troubleshooting

### EC2 Instance Not Reachable

```bash
# Check instance status
aws ec2 describe-instance-status --instance-ids i-021d9b1765cb49ca7 --region us-east-2

# Reboot instance
aws ec2 reboot-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2

# Stop/Start cycle (fixes hypervisor issues)
aws ec2 stop-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2
aws ec2 wait instance-stopped --instance-ids i-021d9b1765cb49ca7 --region us-east-2
aws ec2 start-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2
aws ec2 wait instance-running --instance-ids i-021d9b1765cb49ca7 --region us-east-2

# Get console output (for boot issues)
aws ec2 get-console-output --instance-id i-021d9b1765cb49ca7 --region us-east-2 --output text | tail -50
```

### Service Failures

```bash
# Check detailed service status
sudo systemctl status ztna-intermediate -l --no-pager

# View recent logs with context
sudo journalctl -u ztna-intermediate -n 50 --no-pager

# Check if port is in use
sudo ss -tulpn | grep 4433

# Test certificate loading
openssl x509 -in /home/ubuntu/ztna-agent/certs/cert.pem -text -noout
```

### Common Issues

| Issue | Symptom | Solution |
|-------|---------|----------|
| TlsFail | "Key: --key" in logs | Service uses positional args, not --key flag |
| Connection timeout | SSH hangs | Use Tailscale private IP or stop/start EC2 |
| Connector not registered | No relay logs | Check connector connected to intermediate |
| Port blocked | UDP timeout | Verify security group rules |

---

## Testing

### Verify Services Running

```bash
# SSH to EC2
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126

# Check all services
sudo systemctl status ztna-intermediate ztna-connector echo-server | grep Active

# Verify intermediate-server accepting connections
sudo journalctl -u ztna-intermediate -n 5 --no-pager | grep -E "New connection|Registration"

# Verify connector registered
sudo journalctl -u ztna-connector -n 5 --no-pager | grep -E "Registered|echo-service"
```

### Test Echo Server Locally

```bash
# From EC2 instance
echo "test" | nc 127.0.0.1 8080
```

### Test macOS Agent Connection

1. Ensure macOS Agent is configured with AWS IP (`3.128.36.92`)
2. Open ZtnaAgent.app and enable VPN
3. Monitor EC2 logs:
   ```bash
   sudo journalctl -u ztna-intermediate -f
   ```
4. Look for:
   - "New connection from" (Agent connected)
   - "Registration: Agent for service" (Agent registered)
   - "Relayed X bytes" (Traffic flowing)

---

## Human Demo Guide

### 4-Terminal Setup

For a comprehensive E2E demo, open 4 terminal windows:

#### Terminal 1: Intermediate Server Logs

```bash
# SSH to EC2 and watch intermediate-server
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126
sudo journalctl -u ztna-intermediate -f
```

Expected output:
```
[INFO] New connection from <mac-public-ip>:xxxxx (scid=...)
[INFO] Registration: Agent for service 'echo-service'
[INFO] Relayed 38 bytes from ... to ... (→ Connector)
```

#### Terminal 2: App Connector Logs

```bash
# SSH to EC2 and watch app-connector
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126
sudo journalctl -u ztna-connector -f
```

Expected output:
```
[INFO] Registered as Connector for service 'echo-service'
[INFO] Forwarding to backend: 127.0.0.1:8080
```

#### Terminal 3: Echo Server Logs

```bash
# SSH to EC2 and watch echo-server
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126
sudo journalctl -u echo-server -f
```

Expected output:
```
Echo: b'ZTNA-TEST'
Connected by ('127.0.0.1', xxxxx)
```

#### Terminal 4: macOS Test Commands

```bash
# Enable VPN tunnel
# (Use System Preferences > VPN or ZtnaAgent.app UI)

# Verify tunnel interface
ifconfig | grep -A2 utun

# Send test packet (routed through VPN tunnel)
echo "ZTNA-TEST-AWS" | nc -u -w1 10.100.0.1 9999
# Note: 10.100.0.1:9999 is intercepted by VPN and relayed through ZTNA

# Verify response in other terminals
```

### Demo Flow

1. **Start**: Show all 4 terminals
2. **Agent connects**: Point to Terminal 1 showing "New connection"
3. **Registration**: Show Agent registered for 'echo-service'
4. **Send traffic**: Execute nc command in Terminal 4
5. **Relay logs**: Show bidirectional relay in Terminal 1
6. **Backend receives**: Show echo in Terminal 3
7. **Response path**: Show response relayed back

---

## Cleanup

### Stop Services (Keep Instance)

```bash
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126 'sudo systemctl stop ztna-intermediate ztna-connector echo-server'
```

### Stop EC2 Instance (Save Costs)

```bash
aws ec2 stop-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2
```

### Terminate Everything (Full Cleanup)

```bash
# Terminate EC2 instance
aws ec2 terminate-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2

# Release Elastic IP
aws ec2 release-address --allocation-id eipalloc-018675ba117990c48 --region us-east-2

# Delete security group
aws ec2 delete-security-group --group-id sg-0d15ab7f7b196d540 --region us-east-2
```

---

## Quick Reference

### Key Information

| Item | Value |
|------|-------|
| Region | us-east-2 (Ohio) |
| Public IP | 3.128.36.92 |
| Private IP | 10.0.2.126 |
| SSH Key | ~/.ssh/hfymba.aws.pem |
| SSH User | ubuntu |
| QUIC Port | 4433/UDP |
| Binary Path | ~/ztna-agent/{intermediate-server,app-connector}/target/release/ |
| Cert Path | ~/ztna-agent/certs/ |

### Useful AWS Commands

```bash
# Get instance public IP
aws ec2 describe-instances --instance-ids i-021d9b1765cb49ca7 --region us-east-2 \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text

# Check Elastic IP association
aws ec2 describe-addresses --public-ips 3.128.36.92 --region us-east-2

# View security group rules
aws ec2 describe-security-groups --group-ids sg-0d15ab7f7b196d540 --region us-east-2 \
  --query 'SecurityGroups[0].IpPermissions'
```

### Rebuild Binaries (After Code Changes)

```bash
# SSH to EC2
ssh -i ~/.ssh/hfymba.aws.pem ubuntu@10.0.2.126

# Pull latest code
cd ~/ztna-agent && git pull

# Rebuild
source ~/.cargo/env
cd intermediate-server && cargo build --release
cd ../app-connector && cargo build --release

# Restart services
sudo systemctl restart ztna-intermediate ztna-connector
```

---

## Comparison: AWS vs Pi k8s

| Aspect | AWS EC2 | Pi k8s |
|--------|---------|--------|
| Access | Public IP (3.128.36.92) | LAN IP (10.0.150.205) |
| Management | systemd | kubectl/kustomize |
| Scaling | Manual (single instance) | Kubernetes replicas |
| Cost | $8-15/month (t3.micro) | One-time hardware |
| Latency | ~40-80ms from home | <1ms (same LAN) |
| Use Case | Internet-accessible testing | Local development |

Both deployments support the same ZTNA protocol - choose based on your testing needs.
