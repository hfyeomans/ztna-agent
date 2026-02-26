# Implementation Plan: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Branch:** `feature/006-cloud-deployment`
**Depends On:** 004 (E2E Testing), 005 (P2P Hole Punching), 005a (Swift Integration)
**Last Updated:** 2026-02-21

---

## Goal

Deploy ZTNA components to multiple environments for comprehensive NAT testing:
1. Validate relay functionality (Agent behind NAT → Cloud → Backend)
2. **Validate true NAT-to-NAT hole punching** (Agent behind NAT ↔ Connector behind NAT)
3. Test with real-world applications (HTTP, gaming)
4. Prepare infrastructure for production deployment

---

## ⚠️ Oracle Review Findings (2026-01-25)

### Critical Issues Addressed

| Issue | Severity | Resolution |
|-------|----------|------------|
| NAT-to-NAT topology missing | Critical | Added Home MVP with Connector on Pi k8s behind home NAT |
| P2P listener ports undefined | High | Added P2P Port Requirements section |
| No direct vs relay proof | High | Added verification methods (logs, counters, relay disable) |
| NAT classification method missing | Medium | Added NAT detection tooling |
| Hard-coded endpoints | Medium | Added config parameterization phase |
| TLS trust flow unspecified | Medium | Added explicit cert trust steps |

### Key Insight: True NAT-to-NAT Testing

> **Previous topology was insufficient.** Connector on cloud with public IP does NOT test hole punching -
> it's just client-to-server. For true NAT-to-NAT:
>
> - **Agent:** Behind home/mobile NAT
> - **Connector:** ALSO behind NAT (different network)
> - **Intermediate:** Public IP (signaling server only)

---

## Deployment Environments Overview

| Environment | Purpose | Intermediate | Connector | Backend Apps |
|-------------|---------|--------------|-----------|--------------|
| **AWS** | Production-like, relay testing | EC2 (public) | Fargate/ECS (public*) | HTTP, QuakeKube |
| **DigitalOcean** | Simple relay testing | Droplet (public) | Same Droplet | HTTP echo |
| **Home MVP** | NAT-to-NAT hole punching | Cloud (public) | Pi k8s (behind NAT) | HTTP, QuakeKube |

*AWS Connector has public IP via NLB - tests relay, not full hole punching

---

## P2P Port Requirements

> **Oracle Finding:** Firewall rules only open UDP 4433, but P2P uses different ports.

### Current Port Usage

| Component | Port | Protocol | Purpose |
|-----------|------|----------|---------|
| Intermediate Server | 4433 | UDP/QUIC | Relay, signaling |
| Connector (relay) | ephemeral | UDP/QUIC | Outbound to Intermediate |
| **Connector (P2P listener)** | ephemeral (0.0.0.0:0) | UDP/QUIC | Accept Agent direct connections |
| Agent (P2P) | ephemeral | UDP/QUIC | Connect directly to Connector |

### Required Firewall Changes

**Decision: Fixed P2P Port 4434 ✅**
```bash
# Add to app-connector CLI
--p2p-listen-port 4434

# Firewall rules (all environments)
UDP 4433: Intermediate Server (relay/signaling)
UDP 4434: App Connector P2P listener
```

### Implementation Required

- [x] Add `--p2p-listen-port` CLI arg to app-connector ✅ (commit ef40f19)
- [ ] Update firewall rules in all deployments
- [ ] Document port requirements

---

## Direct vs Relay Verification

> **Oracle Finding:** No deterministic proof of path selection - tests could pass via relay.

### Verification Methods

1. **Log-Based Proof**
   ```rust
   // Add to Agent/Connector logging
   info!("Path selected: {} (peer: {})",
         if direct { "DIRECT" } else { "RELAY" },
         peer_addr);
   ```

2. **Counter-Based Proof**
   - Add metrics: `packets_via_direct`, `packets_via_relay`
   - Expose via stats endpoint or logs

3. **Relay Disable Test**
   ```bash
   # On Intermediate Server, temporarily disable DATAGRAM relay
   # Traffic should continue if direct path works
   ```

4. **Packet Capture Proof**
   ```bash
   # On Agent, capture traffic to Connector's public IP (not Intermediate)
   tcpdump -i en0 udp and host <connector-direct-ip>
   ```

### Acceptance Criteria

| Test | Pass Criteria |
|------|---------------|
| Direct path established | Log shows "Path selected: DIRECT" |
| Direct traffic flows | Packets captured going to Connector IP, not Intermediate |
| Relay disabled, traffic continues | Application works with relay disabled on Intermediate |
| Relay fallback works | Block direct UDP, traffic switches to relay |

---

## NAT Classification Method

> **Oracle Finding:** NAT types listed but no way to classify them.

### NAT Detection Tools

1. **STUN-based detection (our QAD)**
   ```bash
   # QAD already returns reflexive address
   # Compare reflexive ports across multiple Intermediate connections
   # Same port = Endpoint-Independent (Full/Restricted Cone)
   # Different port = Endpoint-Dependent (Symmetric)
   ```

2. **External tool: pystun3**
   ```bash
   pip install pystun3
   pystun3  # Returns NAT type classification
   ```

3. **punch-check tool**
   ```bash
   # https://github.com/delthas/punch-check
   # Checks UDP hole-punching support and NAT properties
   ```

### Classification Recording

Create `nat-classification.md` for each test environment:
```markdown
## Home Network (Test 1)
- Router: Netgear R7000
- External tool result: Restricted Cone NAT
- QAD ports (3 tests): 45123, 45123, 45123 (consistent = not symmetric)
- Hole punch success: Yes
```

---

## TLS Certificate Trust Flow

> **Oracle Finding:** Self-signed cert trust unspecified.

**Decision: Self-signed certificates for MVP ✅**
- Domain + Let's Encrypt available for production migration
- Self-signed simplifies initial deployment and testing

### For macOS Agent

```bash
# 1. Copy cert to Mac
scp user@cloud:/opt/ztna/cert.pem ~/Desktop/ztna-intermediate.pem

# 2. Add to Keychain
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain \
  ~/Desktop/ztna-intermediate.pem

# 3. Verify trust
security verify-cert -c ~/Desktop/ztna-intermediate.pem
```

### For Connector (Rust)

```rust
// In connector config, explicitly trust the cert
config.load_cert_chain_from_pem_file("intermediate-cert.pem")?;
// Or: config.verify_peer(false) for MVP (NOT production)
```

### For Production

Use Let's Encrypt with a real domain - no manual trust required.

---

## Environment 1: AWS Deployment

**Decision: Create NEW VPC ✅** (dedicated ZTNA testing environment)

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     AWS VPC - ztna-test (us-east-2)                          │
│                     CIDR: 10.0.0.0/16 (NEW VPC)                              │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      Public Subnet (10.0.1.0/24)                         ││
│  │                                                                          ││
│  │  ┌─────────────────────┐     ┌─────────────────────────────────────────┐││
│  │  │    EC2 Instance     │     │         Network Load Balancer           │││
│  │  │  (t3.micro)         │     │                                         │││
│  │  │                     │     │  UDP 4433 → Target Group (Intermediate) │││
│  │  │  Intermediate       │◄────│  UDP 4434 → Target Group (Connector)    │││
│  │  │  Server             │     │                                         │││
│  │  │  :4433              │     └─────────────────────────────────────────┘││
│  │  └─────────────────────┘                       │                         ││
│  │                                                │                         ││
│  └────────────────────────────────────────────────┼─────────────────────────┘│
│                                                   │                          │
│  ┌────────────────────────────────────────────────┼─────────────────────────┐│
│  │                      Private Subnet (10.0.2.0/24)                        ││
│  │                                                ▼                         ││
│  │  ┌─────────────────────────────────────────────────────────────────────┐││
│  │  │                    ECS Fargate Cluster                              │││
│  │  │                                                                     │││
│  │  │  ┌───────────────────┐  ┌───────────────────┐  ┌─────────────────┐ │││
│  │  │  │   App Connector   │  │    HTTP App       │  │   QuakeKube     │ │││
│  │  │  │   Task            │  │    Task           │  │   Task          │ │││
│  │  │  │   :4434 (P2P)     │  │    :8080          │  │   :27960        │ │││
│  │  │  └───────────────────┘  └───────────────────┘  └─────────────────┘ │││
│  │  └─────────────────────────────────────────────────────────────────────┘││
│  │                                                                          ││
│  │  ┌─────────────────────┐                                                 ││
│  │  │    NAT Gateway      │ (for Fargate outbound to Intermediate)          ││
│  │  └─────────────────────┘                                                 ││
│  └──────────────────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────────────┘
```

### AWS Resources Required

| Resource | Type | Config | Purpose |
|----------|------|--------|---------|
| VPC | vpc | 10.0.0.0/16 | Network isolation |
| Public Subnet | subnet | 10.0.1.0/24 | EC2, NLB |
| Private Subnet | subnet | 10.0.2.0/24 | Fargate tasks |
| Internet Gateway | igw | - | Public internet access |
| NAT Gateway | nat | - | Fargate outbound |
| EC2 Instance | t3.micro | Ubuntu 24.04 | Intermediate Server |
| NLB | network | UDP listeners | Load balancing |
| ECS Cluster | fargate | - | Container orchestration |
| ECR Repositories | ecr | 3 repos | Container images |
| Security Groups | sg | UDP 4433, 4434 | Network ACLs |

### AWS CLI Deployment Commands

```bash
# 1. Create VPC
aws ec2 create-vpc --cidr-block 10.0.0.0/16 --region us-east-2 \
  --tag-specifications 'ResourceType=vpc,Tags=[{Key=Name,Value=ztna-vpc}]'

# 2. Create subnets
aws ec2 create-subnet --vpc-id $VPC_ID --cidr-block 10.0.1.0/24 \
  --availability-zone us-east-2a --tag-specifications 'ResourceType=subnet,Tags=[{Key=Name,Value=ztna-public}]'

aws ec2 create-subnet --vpc-id $VPC_ID --cidr-block 10.0.2.0/24 \
  --availability-zone us-east-2a --tag-specifications 'ResourceType=subnet,Tags=[{Key=Name,Value=ztna-private}]'

# 3. Create security groups
aws ec2 create-security-group --group-name ztna-intermediate-sg \
  --description "Intermediate Server" --vpc-id $VPC_ID

aws ec2 authorize-security-group-ingress --group-id $SG_ID \
  --protocol udp --port 4433 --cidr 0.0.0.0/0

# 4. Create ECR repositories
aws ecr create-repository --repository-name ztna/app-connector --region us-east-2
aws ecr create-repository --repository-name ztna/http-app --region us-east-2
aws ecr create-repository --repository-name ztna/quakekube --region us-east-2

# 5. Create ECS cluster
aws ecs create-cluster --cluster-name ztna-cluster --region us-east-2

# See deploy/aws/ for full scripts
```

### AWS Fargate UDP Considerations

Per [AWS documentation](https://aws.amazon.com/blogs/containers/aws-fargate-now-supports-udp-load-balancing-with-network-load-balancer/):
- UDP requires NLB (not ALB)
- Platform version 1.4+ required
- TCP health checks required (UDP doesn't support health checks)
- Cannot mix TCP and UDP in single ECS service
- Source IP preserved (no header parsing needed)

### AWS QUIC Support (New!)

Per [AWS announcement](https://aws.amazon.com/blogs/networking-and-content-delivery/introducing-quic-protocol-support-for-network-load-balancer-accelerating-mobile-first-applications/):
- NLB now supports QUIC protocol type
- Session stickiness via QUIC Connection IDs
- TCP_QUIC option for HTTP/3 with TCP fallback

---

## Environment 2: DigitalOcean Deployment

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│              DigitalOcean Droplet (nyc1 or sfo3)                │
│              Ubuntu 24.04, 1GB RAM, 1 vCPU                       │
│              Public IP: x.x.x.x                                  │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Firewall Rules:                                            │ │
│  │  - UDP 4433 inbound (Intermediate)                          │ │
│  │  - UDP 4434 inbound (Connector P2P)                         │ │
│  │  - TCP 22 inbound (SSH, admin IPs only)                     │ │
│  │  - TCP 8080 inbound (HTTP app, optional for direct test)    │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────┐  ┌────────────────────┐                 │
│  │ Intermediate       │  │ App Connector      │                 │
│  │ Server             │  │                    │                 │
│  │ :4433              │◄─│ connects via QUIC  │                 │
│  │                    │  │ :4434 (P2P)        │                 │
│  └────────────────────┘  └─────────┬──────────┘                 │
│                                    │                             │
│                          ┌─────────▼──────────┐                 │
│                          │ HTTP Echo Server   │                 │
│                          │ :8080              │                 │
│                          │ (nginx/httpbin)    │                 │
│                          └────────────────────┘                 │
└──────────────────────────────────────────────────────────────────┘
```

### DigitalOcean API Deployment

```bash
# Using doctl CLI

# 1. Create Droplet
doctl compute droplet create ztna-relay \
  --image ubuntu-24-04-x64 \
  --size s-1vcpu-1gb \
  --region nyc1 \
  --ssh-keys $SSH_KEY_ID \
  --tag-names ztna

# 2. Create Firewall
doctl compute firewall create \
  --name ztna-firewall \
  --droplet-ids $DROPLET_ID \
  --inbound-rules "protocol:udp,ports:4433,address:0.0.0.0/0" \
  --inbound-rules "protocol:udp,ports:4434,address:0.0.0.0/0" \
  --inbound-rules "protocol:tcp,ports:22,address:YOUR_IP/32" \
  --outbound-rules "protocol:tcp,ports:all,address:0.0.0.0/0" \
  --outbound-rules "protocol:udp,ports:all,address:0.0.0.0/0"

# 3. Get IP
DROPLET_IP=$(doctl compute droplet get $DROPLET_ID --format PublicIPv4 --no-header)

# See deploy/digitalocean/ for full scripts
```

### DigitalOcean Simplicity Benefits

- Single VM = simpler debugging
- No VPC/subnet complexity
- Direct public IP (no NAT Gateway)
- Fast provisioning (< 60 seconds)
- Cheap ($6/month)

---

## Environment 3: Home MVP (Raspberry Pi Kubernetes)

### Architecture Overview

> **This is the ONLY topology that tests true NAT-to-NAT hole punching!**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              HOME NETWORK                                    │
│                              (Behind NAT)                                    │
│                                                                              │
│  ┌──────────────────────┐            ┌──────────────────────────────────┐   │
│  │    macOS Machine     │            │     Raspberry Pi Kubernetes      │   │
│  │                      │            │     (arm64 cluster)              │   │
│  │  ┌────────────────┐  │            │                                  │   │
│  │  │    Agent       │  │            │  ┌────────────────────────────┐  │   │
│  │  │  (ZtnaAgent    │  │            │  │  App Connector Pod         │  │   │
│  │  │   .app)        │  │            │  │  (arm64 binary)            │  │   │
│  │  └───────┬────────┘  │            │  │  connects to cloud         │  │   │
│  │          │           │            │  │  Intermediate              │  │   │
│  │          │           │            │  └────────────┬───────────────┘  │   │
│  └──────────┼───────────┘            │               │                  │   │
│             │                        │  ┌────────────▼───────────────┐  │   │
│             │                        │  │  HTTP App Pod              │  │   │
│             │                        │  │  (nginx or httpbin)        │  │   │
│             │                        │  │  :8080                     │  │   │
│             │                        │  └────────────────────────────┘  │   │
│             │                        │                                  │   │
│             │                        │  ┌────────────────────────────┐  │   │
│             │                        │  │  QuakeKube Pod             │  │   │
│             │                        │  │  (Quake 3 server)          │  │   │
│             │                        │  │  :27960 (game)             │  │   │
│             │                        │  │  :8080 (web client)        │  │   │
│             │                        │  └────────────────────────────┘  │   │
│             │                        └──────────────────────────────────┘   │
│             │                                                               │
└─────────────┼───────────────────────────────────────────────────────────────┘
              │                              ▲
              │ QUIC (relay)                 │ QUIC (relay)
              │                              │
              │    ┌───────────────────────────────────────────┐
              │    │           Cloud VM (DigitalOcean)         │
              │    │           (Public IP: y.y.y.y)            │
              └───►│                                           │◄───────┘
                   │  ┌─────────────────────────────────────┐  │
                   │  │       Intermediate Server           │  │
                   │  │       :4433                         │  │
                   │  │       (Signaling + Relay)           │  │
                   │  └─────────────────────────────────────┘  │
                   │                                           │
                   │  P2P Hole Punching:                       │
                   │  Agent ◄─────────────────────► Connector  │
                   │  (home NAT)    direct UDP    (home NAT)   │
                   └───────────────────────────────────────────┘
```

### Pi Cluster Configuration

| Role | IP Address | Notes |
|------|------------|-------|
| Control Plane | 10.0.150.101 | kubectl access confirmed |
| Worker 1 | 10.0.150.102 | |
| Worker 2 | 10.0.150.103 | |
| Worker 3 | 10.0.150.104 | |
| Worker 4 | 10.0.150.105 | |
| Worker 5 | 10.0.150.106 | |
| Worker 6 | 10.0.150.107 | |
| Worker 7 | 10.0.150.108 | |

### Why This Tests NAT-to-NAT

| Component | Network | NAT Status |
|-----------|---------|------------|
| Agent | Home WiFi | Behind home router NAT |
| Connector | Home k8s (10.0.150.x) | Behind SAME home router NAT |
| Intermediate | Cloud | Public IP (signaling only) |

**For P2P to work:** Agent and Connector must punch holes through the home NAT to reach each other directly, using reflexive addresses learned via Intermediate.

**Hairpin NAT Note:** Since both are behind the same NAT, this also tests hairpin translation. Many home routers don't support hairpin well, which is a realistic test case.

### Kubernetes Deployment

```yaml
# deploy/k8s/app-connector.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: app-connector
  namespace: ztna
spec:
  replicas: 1
  selector:
    matchLabels:
      app: app-connector
  template:
    metadata:
      labels:
        app: app-connector
    spec:
      containers:
      - name: app-connector
        image: ghcr.io/hfyeomans/ztna-agent/app-connector:latest
        args:
          - --intermediate
          - "cloud-intermediate-ip:4433"
          - --service-id
          - "home-test-service"
          - --local-addr
          - "http-app.ztna.svc.cluster.local:8080"
          - --p2p-cert
          - /certs/connector-cert.pem
          - --p2p-key
          - /certs/connector-key.pem
        ports:
        - containerPort: 4434
          protocol: UDP
        volumeMounts:
        - name: certs
          mountPath: /certs
      volumes:
      - name: certs
        secret:
          secretName: connector-tls
---
apiVersion: v1
kind: Service
metadata:
  name: app-connector
  namespace: ztna
spec:
  type: NodePort  # Or LoadBalancer if using MetalLB
  selector:
    app: app-connector
  ports:
  - port: 4434
    targetPort: 4434
    protocol: UDP
    nodePort: 30434  # Fixed port for firewall rules
```

### HTTP Test App Deployment

```yaml
# deploy/k8s/http-app.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: http-app
  namespace: ztna
spec:
  replicas: 1
  selector:
    matchLabels:
      app: http-app
  template:
    metadata:
      labels:
        app: http-app
    spec:
      containers:
      - name: httpbin
        image: kennethreitz/httpbin:latest
        ports:
        - containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: http-app
  namespace: ztna
spec:
  selector:
    app: http-app
  ports:
  - port: 8080
    targetPort: 80
```

### QuakeKube Deployment

```yaml
# deploy/k8s/quakekube.yaml
# Using Helm chart from https://grahamplata.github.io/charts/charts/quake-kube/
apiVersion: v1
kind: Namespace
metadata:
  name: quake
---
# Via Helm:
# helm repo add grahamplata https://grahamplata.github.io/charts
# helm install quake grahamplata/quake-kube -n quake
```

**QuakeKube Notes:**
- [QuakeKube](https://github.com/criticalstack/quake-kube) runs Quake 3 server in Kubernetes
- Browser clients connect via WebSocket (QuakeJS)
- Native clients connect via UDP :27960
- Supports arm64 (Raspberry Pi compatible)
- Tests real-time gaming latency through ZTNA tunnel

---

## Test Applications

### 1. Simple HTTP App

**Purpose:** Basic connectivity and latency testing

**Options:**
- `kennethreitz/httpbin` - Full HTTP test server with various endpoints
- `nginx` - Simple static content
- Custom Go server - Minimal, controlled

**Test Endpoints:**
```bash
# Basic connectivity
curl http://<service>/get

# Latency test (returns request timing)
curl http://<service>/delay/1

# Echo test
curl -X POST http://<service>/post -d "test data"
```

### 2. QuakeKube (Gaming Latency Test)

**Purpose:** Test real-time gaming playability through ZTNA tunnel

**Architecture:**
```
Browser (QuakeJS) ──WebSocket──► QuakeKube Pod ──► ioquake3 server
                                     │
Native Quake 3 ──────UDP──────────────┘
```

**Why QuakeKube:**
- Real-time game = sensitive to latency and jitter
- Browser client (WebSocket) + Native client (UDP)
- If game is playable through ZTNA, latency is acceptable
- arm64 support for Raspberry Pi

**Playability Criteria:**
| Metric | Acceptable | Good | Excellent |
|--------|------------|------|-----------|
| Ping | < 150ms | < 80ms | < 40ms |
| Jitter | < 30ms | < 15ms | < 5ms |
| Packet loss | < 3% | < 1% | 0% |

---

## Configuration Parameterization

> **Oracle Finding:** Hard-coded localhost assumptions will break remote testing.

### Current Hard-Coded Values

| Value | Location | Fix |
|-------|----------|-----|
| `127.0.0.1:4433` | test scripts, configs | `$INTERMEDIATE_HOST:$INTERMEDIATE_PORT` |
| `test-service` | scripts | `$SERVICE_ID` |
| Cert paths | common.sh | `$CERT_DIR` |
| DATAGRAM sizes | quic-client | Query `dgram_max_writable_len()` |

### Environment File

```bash
# deploy/env/cloud.env
INTERMEDIATE_HOST=<cloud-ip>
INTERMEDIATE_PORT=4433
SERVICE_ID=cloud-test-service
CERT_DIR=/opt/ztna/certs

# deploy/env/home.env
INTERMEDIATE_HOST=<cloud-ip>
INTERMEDIATE_PORT=4433
SERVICE_ID=home-test-service
CERT_DIR=/etc/ztna/certs
```

---

## Dynamic Service Configuration (Future Requirement)

> **Post-Deployment Note:** After successful cloud deployment, we must revisit all hard-coding
> and move to dynamic, variable-based configuration. This is critical for production readiness.

### Problem Statement

The current MVP has many hard-coded values that work for testing but won't scale:

| Component | Hard-Coded Issue | Production Requirement |
|-----------|------------------|------------------------|
| macOS Agent | `serverHost = "3.128.36.92"` | User-configurable intermediate server |
| macOS Agent | VPN routes `10.100.0.0/24` | Dynamic route table from service catalog |
| App Connector | `--server 127.0.0.1:4433` | Environment-based config |
| App Connector | `--service echo-service` | Service registration from config |
| Intermediate | Port 4433 hard-coded | Configurable port |
| All | Service ID "echo-service" | Dynamic service discovery |

### Target Architecture: Service Catalog

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        SERVICE CATALOG (Future)                              │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  services.yaml / services.json                                          ││
│  │                                                                          ││
│  │  services:                                                               ││
│  │    - id: "echo-service"                                                 ││
│  │      display_name: "Echo Test Service"                                  ││
│  │      virtual_ip: "10.100.0.1"                                           ││
│  │      port: 9999                                                         ││
│  │      protocol: udp                                                      ││
│  │      backend: "127.0.0.1:9999"                                          ││
│  │                                                                          ││
│  │    - id: "web-app"                                                      ││
│  │      display_name: "Internal Web App"                                   ││
│  │      virtual_ip: "10.100.0.2"                                           ││
│  │      port: 443                                                          ││
│  │      protocol: tcp                                                      ││
│  │      backend: "internal-app.corp.local:443"                             ││
│  │                                                                          ││
│  │  intermediate_server:                                                    ││
│  │    host: "ztna.example.com"                                             ││
│  │    port: 4433                                                           ││
│  │    cert_fingerprint: "sha256:abc123..."                                 ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Configuration Flow (Target State)

```
                    ┌──────────────────┐
                    │  Admin Portal    │
                    │  (Web/API)       │
                    └────────┬─────────┘
                             │ Configure services,
                             │ assign permissions
                             ▼
                    ┌──────────────────┐
                    │  Control Plane   │
                    │  (Config Store)  │
                    └────────┬─────────┘
                             │
          ┌──────────────────┼──────────────────┐
          ▼                  ▼                  ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│   macOS Agent    │ │  Intermediate    │ │  App Connector   │
│                  │ │  Server          │ │                  │
│  Receives:       │ │                  │ │  Receives:       │
│  - Server addr   │ │  Receives:       │ │  - Server addr   │
│  - Service list  │ │  - Service map   │ │  - Service IDs   │
│  - Route table   │ │  - Auth config   │ │  - Backend maps  │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

### Phase 1: Environment Variables (Immediate)

Minimum viable improvement - externalize hard-coded values:

**macOS Agent (Swift):**
```swift
// Instead of: private let serverHost = "3.128.36.92"
// Read from app configuration or environment:
private var serverHost: String {
    ProcessInfo.processInfo.environment["ZTNA_SERVER_HOST"]
        ?? UserDefaults.standard.string(forKey: "serverHost")
        ?? "localhost"
}
```

**App Connector (Rust):**
```rust
// Already uses CLI args, but add environment fallbacks:
let server = args.server.unwrap_or_else(|| {
    std::env::var("ZTNA_INTERMEDIATE_SERVER")
        .unwrap_or_else(|_| "127.0.0.1:4433".to_string())
});
```

### Phase 2: Configuration File (Short-term)

**Config file format:** YAML or JSON for human readability

```yaml
# ~/.ztna/config.yaml (macOS Agent)
intermediate_server:
  host: "ztna.example.com"
  port: 4433

services:
  - id: "echo-service"
    virtual_ip: "10.100.0.1"
    routes: ["10.100.0.0/24"]

# /etc/ztna/connector.yaml (App Connector)
intermediate_server:
  host: "ztna.example.com"
  port: 4433

services:
  - id: "echo-service"
    backend: "127.0.0.1:9999"
    protocol: "udp"
```

### Phase 3: Dynamic Configuration API (Medium-term)

**Flow:**
1. Agent/Connector authenticate to Intermediate
2. Intermediate returns authorized service list
3. Clients configure routes/backends dynamically

**Protocol addition:**
```rust
// New DATAGRAM type for config sync
const CONFIG_SYNC: u8 = 0x20;

// Config sync request (Agent → Intermediate)
// [0x20, 0x01]  // Type=CONFIG_SYNC, Subtype=REQUEST

// Config sync response (Intermediate → Agent)
// [0x20, 0x02, len, json_config...]
```

### Phase 4: Full Service Discovery (Long-term)

- Integration with identity provider (OIDC, SAML)
- Role-based service access
- Real-time service health/availability
- Automatic failover and load balancing

### Implementation Priority

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| P0 | Remove 1.1.1.1 from all code | Done ✅ | Critical |
| P1 | Externalize server host to env/config | Low | High |
| P1 | Externalize service IDs | Low | High |
| P2 | VPN route configuration from file | Medium | High |
| P2 | Config file parser for Agent/Connector | Medium | Medium |
| P3 | Dynamic config API via DATAGRAM | High | High |
| P4 | Full service discovery | High | Future |

### Files Requiring Updates

| File | Current Hard-Code | Required Change |
|------|-------------------|-----------------|
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | `serverHost = "3.128.36.92"` | Config-driven |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | `10.100.0.0/24` route | Dynamic route table |
| `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` | `echo-service` | Config-driven service ID |
| `app-connector/src/main.rs` | CLI args only | Add config file support |
| `intermediate-server/src/main.rs` | CLI args only | Add config file support |
| `deploy/aws/aws-deploy-skill.md` | Example IPs | Document config requirements |
| `deploy/k8s/k8s-deploy-skill.md` | Example IPs | Document config requirements |

---

## Post-Cloud Deployment: Next Steps (2026-01-26)

> **Decision:** Config-first approach (Option 2) - establish configuration mechanism before
> expanding protocol support. This enables visible progress toward multi-service demos.

### Task Sequence

| Order | Task | Effort | Outcome |
|-------|------|--------|---------|
| 1 | **Validate AWS E2E** | Low | Proves cloud deployment works |
| 2 | **Config File Mechanism** | Medium | Foundation for all dynamic config |
| 3 | **IP→Service Routing** | Medium | Multiple services per connection |
| 4 | **TCP Support** | Medium-High | Enables web apps, databases |
| 5 | **ICMP Support** | Low | Enables ping diagnostics |
| 6 | **Admin Dashboard** | High | UI for policy management |

### Task 2: Config File Mechanism (Details)

**Goal:** Single source of truth for service definitions, readable by all components.

**Config Locations:**
- macOS Agent: `~/Library/Application Support/ZtnaAgent/config.yaml`
- App Connector: `/etc/ztna/connector.yaml` or `~/.ztna/connector.yaml`
- Intermediate: `/etc/ztna/intermediate.yaml`

**Shared Schema (services.yaml):**
```yaml
version: 1
intermediate_server:
  host: "ztna.example.com"
  port: 4433

services:
  - id: echo-service
    display_name: "Echo Test Service"
    virtual_ip: 10.100.0.1
    port: 9999
    protocol: udp
    backend: "127.0.0.1:9999"  # Connector-side only

  - id: web-app
    display_name: "Internal Web App"
    virtual_ip: 10.100.0.2
    port: 443
    protocol: tcp
    backend: "internal.corp.local:443"
```

**Implementation Order:**
1. Define YAML schema (shared across components)
2. Add config parser to App Connector (Rust - serde_yaml)
3. Add config parser to macOS Agent (Swift - Yams or Foundation)
4. Add config parser to Intermediate Server (Rust)
5. Test with multi-service config

### Task 3: IP→Service Routing (Details)

**Current limitation:** Routing is implicit (single service per connection).

**Problem:**
```
Agent sends packet to 10.100.0.1:9999  → routed to echo-service (implicit)
Agent sends packet to 10.100.0.2:443  → ??? (no service context)
```

**Solution: Include service_id in relay DATAGRAM:**
```
Current:  [RELAY_TYPE, payload...]
Proposed: [RELAY_TYPE, service_id_len, service_id, payload...]
```

OR: Agent maintains IP→service mapping from config, intermediate routes based on
registered service associations.

### Task 4: TCP Support (Details)

**Current app-connector code:**
```rust
// Only UDP supported currently
if ip_header.protocol != 17 {
    debug!("Dropping non-UDP packet (protocol {})", ip_header.protocol);
    return;
}
```

**Required changes:**
1. Accept protocol 6 (TCP) in addition to 17 (UDP)
2. Handle TCP differently - may need QUIC streams instead of DATAGRAMs for reliability
3. Manage NAT state for TCP connections (source port mapping)
4. Handle TCP flags (SYN, ACK, FIN, RST) appropriately

**Design consideration:** TCP over QUIC DATAGRAM vs TCP over QUIC Stream
- DATAGRAM: Lower latency, but unreliable (may need app-layer retransmit)
- Stream: Reliable, but adds latency (double-ordering problem)

---

## Success Criteria (Updated)

### Relay Validation (AWS/DigitalOcean)

- [ ] Intermediate Server running on cloud with public IP
- [ ] App Connector connected to Intermediate
- [ ] Agent (behind home NAT) connects to cloud Intermediate
- [ ] QAD returns correct public IP for Agent
- [ ] DATAGRAM relay works: Agent → Intermediate → Connector → Backend
- [ ] HTTP app accessible through tunnel
- [ ] Latency acceptable (< 150ms RTT for same region)

### NAT-to-NAT Hole Punching (Home MVP)

- [ ] Connector running on Pi k8s (behind home NAT)
- [ ] Agent running on Mac (behind same home NAT)
- [ ] Both connect to cloud Intermediate
- [ ] Candidate exchange completes
- [ ] **PROOF:** Log shows "Path selected: DIRECT"
- [ ] **PROOF:** Traffic captured going directly to peer, not Intermediate
- [ ] **PROOF:** Relay disabled on Intermediate, traffic continues
- [ ] Fallback test: Block direct UDP, verify relay takes over

### Gaming Test (QuakeKube)

- [ ] QuakeKube deployed and accessible via Connector
- [ ] Browser client connects via WebSocket
- [ ] Game playable with < 150ms ping
- [ ] No visible lag during movement

---

## Risks & Mitigations (Updated)

| Risk | Impact | Mitigation |
|------|--------|------------|
| P2P appears to work but only via relay | False confidence | Add deterministic path proof (logs, counters) |
| UDP 4433-only firewall blocks P2P | All tests use relay | Define and open P2P ports (4434+) |
| Hairpin NAT not supported | Home MVP fails | Document, fall back to relay, note limitation |
| Self-signed cert trust issues | Connection failures | Document trust flow, test locally first |
| Mobile/enterprise blocks UDP | Intermittent failures | Document, always have relay fallback |
| Hard-coded configs break cloud | Test failures | Parameterize before deployment |

---

## Phase Summary

| Phase | Description | Environment | Status |
|-------|-------------|-------------|--------|
| 0 | Docker NAT simulation | Local | ✅ Complete |
| 1 | Config parameterization | Local | Partial |
| 2 | DigitalOcean deployment | Cloud | Skipped (AWS used) |
| 3 | Basic relay validation | DO + Home NAT | Skipped (AWS used) |
| 4 | AWS VPC deployment | Cloud | ✅ Complete |
| 4.9 | Connection resilience | Swift-only | ✅ Complete |
| 5 | Home MVP (Pi k8s) deployment | Home | ✅ Complete |
| 5a | Full E2E relay routing | Rust + Swift | ✅ Complete |
| 6 | P2P Swift integration + NAT testing | AWS + Home NAT | ✅ Complete |
| 7 | HTTP test application validation | AWS | In Progress |
| 8 | Performance metrics (P2P vs relay) | AWS | Pending |
| 9 | Documentation | - | Pending |
| 10 | PR & Merge | - | Pending |

---

## Phase 7: Test Application Validation (Revised 2026-02-21)

> **Original plan:** httpbin + QuakeKube on Pi k8s.
> **Actual implementation:** Simple HTTP server on AWS EC2, second connector for web-app service.
> **Reason:** Validates TCP-through-tunnel with minimal infra. QuakeKube deferred to post-MVP.

### Architecture

```
macOS Agent
├── 10.100.0.1 → echo-service  → ztna-connector  (port 4434, P2P enabled)  → echo-server :9999
└── 10.100.0.2 → web-app       → ztna-connector-web (port 4435, relay-only) → http-server :8080
```

### Key Constraint: Single forward_addr per Connector

The App Connector only forwards to a single `forward_addr`. Per-service backend routing is NOT implemented (config struct has fields but they're `#[allow(dead_code)]`). Workaround: run separate connector instances per service.

### Changes

| Component | Change |
|-----------|--------|
| AWS: http-server.service | python3 -m http.server 8080 serving /opt/ztna/www/ |
| AWS: ztna-connector-web.service | Second connector: web-app service, relay-only, port 4435 |
| ContentView.swift | Add `["id": "web-app", "virtualIp": "10.100.0.2"]` to services array |

### Verification

- `curl http://10.100.0.2:8080/` returns 200 with HTML content
- UDP echo to 10.100.0.1 still works (no regression)
- Both services registered in Agent logs

---

## Phase 8: Performance Metrics (Revised 2026-02-21)

> **Original plan:** Relay/direct latency + throughput + 1-hour stability + QuakeKube gaming test.
> **Actual implementation:** Ping RTT comparison (P2P vs relay), 10-min stability test.
> **Reason:** Application-level ping is sufficient for MVP. agent_get_path_stats() returns 0ms RTT.

### Methodology

| Metric | P2P Path (10.100.0.1) | Relay Path (10.100.0.2) |
|--------|-----------------------|------------------------|
| Latency | ping -c 50 via P2P direct | ping -c 50 via relay |
| Stability | ping -c 600 (10 min) | N/A |
| Path flapping | Agent log monitoring | N/A |

### Not in Scope (Post-MVP)

- Throughput benchmarks (iperf)
- QuakeKube gaming latency
- 1-hour stability test
- Fixing agent_get_path_stats() 0ms RTT bug

---

## References

### Cloud Providers
- [DigitalOcean API](https://docs.digitalocean.com/reference/api/)
- [AWS Fargate UDP Support](https://aws.amazon.com/blogs/containers/aws-fargate-now-supports-udp-load-balancing-with-network-load-balancer/)
- [AWS NLB QUIC Support](https://aws.amazon.com/blogs/networking-and-content-delivery/introducing-quic-protocol-support-for-network-load-balancer-accelerating-mobile-first-applications/)

### NAT Traversal
- [UDP Hole Punching - Wikipedia](https://en.wikipedia.org/wiki/UDP_hole_punching)
- [NAT Hole Punching with QUIC - arXiv](https://arxiv.org/abs/2408.01791)
- [Tailscale NAT Traversal](https://tailscale.com/blog/nat-traversal-improvements-pt-1)

### Test Applications
- [QuakeKube - GitHub](https://github.com/criticalstack/quake-kube)
- [QuakeKube Helm Chart](https://grahamplata.github.io/charts/charts/quake-kube/)
- [httpbin](https://httpbin.org/)

### Kubernetes
- [k3s Documentation](https://docs.k3s.io/)
- [Helm Documentation](https://helm.sh/docs/)
