# ZTNA Kubernetes Deployment

Deploy ZTNA components to Kubernetes clusters (tested on Pi k8s with Cilium).

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        KUBERNETES DEPLOYMENT                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐                ┌─────────────────────────────────────────┐ │
│  │   macOS     │                │           Pi k8s Cluster                 │ │
│  │   Agent     │                │                                          │ │
│  │             │                │  ┌─────────────────────────────────────┐ │ │
│  │  (Home      │◄──── QUIC ────►│  │    Intermediate Server              │ │ │
│  │   Network)  │                │  │    LoadBalancer: 10.0.150.200:4433  │ │ │
│  │             │                │  │    (Cilium L2 announcement)         │ │ │
│  └─────────────┘                │  └─────────────────────────────────────┘ │ │
│                                  │                    │                      │ │
│                                  │                    │ ClusterIP            │ │
│                                  │                    ▼                      │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    App Connector                    │ │ │
│                                  │  │    (Cluster internal)               │ │ │
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

## Prerequisites

### 1. Kubernetes Cluster

- Kubernetes 1.28+ (tested on Pi k8s with Cilium)
- Cilium with L2 announcements enabled (for LoadBalancer)
- `kubectl` configured to access the cluster

### 2. Container Registry Access

```bash
# Login to GHCR
echo $GITHUB_TOKEN | docker login ghcr.io -u YOUR_USERNAME --password-stdin

# Create image pull secret in cluster (if private repo)
kubectl create secret docker-registry ghcr-secret \
  --docker-server=ghcr.io \
  --docker-username=YOUR_USERNAME \
  --docker-password=$GITHUB_TOKEN \
  -n ztna
```

### 3. Docker Buildx (for multi-arch builds)

```bash
# Install buildx (usually included with Docker Desktop)
docker buildx version

# Create builder for multi-arch
docker buildx create --name ztna-builder --use --bootstrap
```

## Quick Start

### 1. Build and Push Images

```bash
cd deploy/k8s

# Build all images for arm64 + amd64 and push to GHCR
./build-push.sh

# Or build for arm64 only (faster for Pi testing)
./build-push.sh --arm64-only

# Or build specific component
./build-push.sh --arm64-only intermediate
```

### 2. Generate TLS Certificates

```bash
# Generate intermediate server certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout intermediate-key.pem -out intermediate-cert.pem \
  -days 365 -nodes -subj "/CN=ztna-intermediate"

# Generate connector certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout connector-key.pem -out connector-cert.pem \
  -days 365 -nodes -subj "/CN=ztna-connector"

# Create secrets in cluster
kubectl create namespace ztna
kubectl create secret tls ztna-intermediate-tls -n ztna \
  --cert=intermediate-cert.pem --key=intermediate-key.pem
kubectl create secret tls ztna-connector-tls -n ztna \
  --cert=connector-cert.pem --key=connector-key.pem

# Clean up local cert files
rm -f intermediate-*.pem connector-*.pem
```

### 3. Configure Pi Home Overlay

Edit `overlays/pi-home/kustomization.yaml`:

```yaml
# Update GHCR owner if different
images:
  - name: ghcr.io/OWNER/ztna-intermediate-server
    newName: ghcr.io/YOUR_USERNAME/ztna-intermediate-server
    newTag: latest

# Update Cilium L2 IP to match your IP pool
patches:
  - patch: |-
      - op: replace
        path: /metadata/annotations/io.cilium~1lb-ipam-ips
        value: "YOUR_LOADBALANCER_IP"
```

### 4. Deploy

```bash
# Preview what will be deployed
kubectl kustomize overlays/pi-home

# Deploy to cluster
kubectl apply -k overlays/pi-home

# Watch deployment status
kubectl get pods -n ztna -w
```

### 5. Verify Deployment

```bash
# Check all pods are running
kubectl get pods -n ztna

# Check services
kubectl get svc -n ztna

# Check LoadBalancer got external IP
kubectl get svc intermediate-server -n ztna

# View intermediate server logs
kubectl logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# View app connector logs
kubectl logs -n ztna -l app.kubernetes.io/name=app-connector -f
```

## Testing

### From macOS Agent

Once deployed, test connectivity from your macOS machine:

```bash
# Build and run the QUIC test client
cd tests/e2e/fixtures/quic-client
cargo build --release

# Test connection to Pi cluster (replace IP with your LoadBalancer IP)
./target/release/quic-test-client 10.0.150.200:4433 \
  --cert ../certs/client-cert.pem \
  --key ../certs/client-key.pem
```

### Verify End-to-End Flow

```bash
# In terminal 1: Watch intermediate server logs
kubectl logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# In terminal 2: Watch connector logs
kubectl logs -n ztna -l app.kubernetes.io/name=app-connector -f

# In terminal 3: Watch echo server logs
kubectl logs -n ztna -l app.kubernetes.io/name=echo-server -f

# In terminal 4: Run test client
./quic-test-client 10.0.150.200:4433
```

Expected log flow:
1. Intermediate: "New connection from <macOS_IP>"
2. Intermediate: "Agent connection: <macOS_IP>"
3. Intermediate: "Relaying DATAGRAM..."
4. Connector: "Forwarding to echo-server..."
5. Echo: "Received packet, echoing..."
6. Test client: "Received echo response"

## Cilium Configuration

### Check Cilium L2 Announcement Status

```bash
# Check if Cilium LB IPAM is working
kubectl get ciliumloadbalancerippool -A
kubectl get ciliumbgppeeringpolicy -A

# Check if L2 announcements are configured
kubectl get ciliuml2announcementpolicy -A

# View Cilium status
cilium status
```

### Create L2 Announcement Policy (if needed)

```yaml
apiVersion: cilium.io/v2alpha1
kind: CiliumL2AnnouncementPolicy
metadata:
  name: ztna-l2-policy
spec:
  loadBalancerIPs: true
  interfaces:
    - eth0
  nodeSelector:
    matchLabels:
      kubernetes.io/os: linux
---
apiVersion: cilium.io/v2alpha1
kind: CiliumLoadBalancerIPPool
metadata:
  name: ztna-ip-pool
spec:
  blocks:
    - start: 10.0.150.200
      stop: 10.0.150.210
```

## Troubleshooting

### Pods not starting

```bash
# Check pod events
kubectl describe pod -n ztna <pod-name>

# Check if image pull failed
kubectl get events -n ztna --field-selector reason=Failed

# Check if secrets exist
kubectl get secrets -n ztna
```

### LoadBalancer has no external IP

```bash
# Check Cilium L2 announcement status
cilium status

# Check if IP pool is configured
kubectl get ciliumloadbalancerippool -A

# Manual test: Announce IP from a node
# (Cilium should do this automatically)
```

### Connection timeout from macOS

```bash
# Verify UDP 4433 is reachable
nc -u -v 10.0.150.200 4433

# Check firewall on Pi nodes
sudo iptables -L -n | grep 4433

# Check Cilium network policy isn't blocking
kubectl get ciliumnetworkpolicy -n ztna
```

### QAD shows wrong IP

- Ensure `externalTrafficPolicy: Local` is set on Service
- This preserves client source IP for QAD to work correctly

## Cleanup

```bash
# Delete deployment
kubectl delete -k overlays/pi-home

# Delete namespace (removes everything)
kubectl delete namespace ztna

# Remove buildx builder (optional)
docker buildx rm ztna-builder
```

## Directory Structure

```
deploy/k8s/
├── README.md                 # This file
├── build-push.sh             # Multi-arch image builder
├── base/                     # Base Kustomize manifests
│   ├── kustomization.yaml
│   ├── namespace.yaml
│   ├── configmap.yaml
│   ├── secrets.yaml          # Placeholder secrets
│   ├── intermediate-server.yaml
│   ├── app-connector.yaml
│   └── echo-server.yaml
└── overlays/
    └── pi-home/              # Pi cluster overlay
        └── kustomization.yaml
```
