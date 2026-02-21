# Kubernetes Deployment Skill Guide

A comprehensive guide covering all commands, architecture, testing, and lessons learned for deploying ZTNA on a Pi k8s cluster with Cilium.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Prerequisites](#prerequisites)
3. [Cluster Verification](#cluster-verification)
4. [Cilium L2 Configuration](#cilium-l2-configuration)
5. [Building Multi-Arch Images](#building-multi-arch-images)
6. [TLS Certificate Management](#tls-certificate-management)
7. [Deployment Commands](#deployment-commands)
8. [Troubleshooting](#troubleshooting)
9. [Lessons Learned](#lessons-learned)

---

## Architecture Overview

### Network Topology

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Pi k8s Cluster Deployment                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐                ┌─────────────────────────────────────────┐ │
│  │   macOS     │                │           Pi k8s Cluster                 │ │
│  │   Agent     │                │           (10.0.150.101-108)            │ │
│  │             │                │                                          │ │
│  │  (Home      │◄──── QUIC ────►│  ┌─────────────────────────────────────┐ │ │
│  │   Network)  │   UDP:4433     │  │    Intermediate Server              │ │ │
│  │             │                │  │    LoadBalancer: 10.0.150.200:4433  │ │ │
│  └─────────────┘                │  │    (Cilium L2 announcement)         │ │ │
│                                  │  └─────────────────────────────────────┘ │ │
│                                  │                    │                      │ │
│                                  │                    │ ClusterIP            │ │
│                                  │                    ▼                      │ │
│                                  │  ┌─────────────────────────────────────┐ │ │
│                                  │  │    App Connector                    │ │ │
│                                  │  │    (Pod-to-Pod communication)       │ │ │
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

### Component Summary

| Component | Service Type | Port | Purpose |
|-----------|--------------|------|---------|
| intermediate-server | LoadBalancer | 4433/UDP | QUIC relay, QAD, signaling |
| app-connector | ClusterIP | 4434/UDP | Connects to intermediate, forwards traffic |
| echo-server | ClusterIP | 9999/UDP | Test service for E2E validation |

### Kustomize Directory Structure

```
deploy/k8s/
├── README.md                 # Main documentation
├── k8s-deploy-skill.md       # This file
├── build-push.sh             # Multi-arch image builder
├── check-cluster.sh          # Cluster prerequisite checker
├── base/                     # Kustomize base manifests
│   ├── kustomization.yaml
│   ├── namespace.yaml
│   ├── configmap.yaml
│   ├── secrets.yaml          # Placeholder (create manually)
│   ├── intermediate-server.yaml
│   ├── app-connector.yaml
│   └── echo-server.yaml
└── overlays/
    └── pi-home/              # Pi cluster overlay
        ├── kustomization.yaml
        └── cilium-l2.yaml    # Cilium L2 config (cluster-scoped)
```

---

## Prerequisites

### Required Tools

```bash
# Verify tools are installed
docker --version          # Docker with buildx support
docker buildx version     # Multi-arch build capability
kubectl version           # Kubernetes CLI
kustomize version         # Or use kubectl kustomize
openssl version           # For certificate generation
```

### Cluster Requirements

- Kubernetes 1.28+ (k3s v1.34.3 tested)
- Cilium CNI with L2 announcement support (v1.18.5 tested)
- arm64 nodes (Raspberry Pi 4/5)
- Network access from macOS to cluster LoadBalancer IP

---

## Cluster Verification

### Check Kubectl Access

```bash
# Verify cluster connectivity
kubectl cluster-info

# Expected output:
# Kubernetes control plane is running at https://10.0.150.101:6443
```

### Check Node Architecture

```bash
# List nodes with architecture
kubectl get nodes -o custom-columns='NAME:.metadata.name,ARCH:.status.nodeInfo.architecture,OS:.status.nodeInfo.osImage'

# Expected: arm64 for Pi 4/5
```

### Check Cilium Status

```bash
# Verify Cilium is running
kubectl get pods -n kube-system -l k8s-app=cilium

# Check Cilium version
kubectl get pods -n kube-system -l k8s-app=cilium -o jsonpath='{.items[0].spec.containers[0].image}'
```

### Run Full Cluster Check

```bash
./deploy/k8s/check-cluster.sh
```

This script verifies:
- kubectl access
- Node architecture (arm64)
- Cilium installation
- L2 announcement policy CRDs
- LoadBalancer IP pool CRDs
- Existing LoadBalancer services

---

## Cilium L2 Configuration

### Check Existing L2 Configuration

```bash
# Check L2 announcement policies
kubectl get ciliuml2announcementpolicy -A

# Check LoadBalancer IP pools
kubectl get ciliumloadbalancerippool -A
```

### Create L2 Announcement Policy

```yaml
# cilium-l2.yaml
apiVersion: cilium.io/v2alpha1
kind: CiliumL2AnnouncementPolicy
metadata:
  name: ztna-l2-policy
spec:
  loadBalancerIPs: true
  interfaces:
    - ^eth0$  # Regex for eth0 interface
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
    - start: "10.0.150.200"
      stop: "10.0.150.210"
```

### Enable L2 Announcements in Cilium (Required!)

**CRITICAL**: Having the CRDs is not enough - L2 announcements must be **enabled** in Cilium itself via Helm:

```bash
# Check if L2 is currently enabled
kubectl get cm cilium-config -n kube-system -o yaml | grep -i l2

# If "enable-l2-announcements" is missing or "false", enable it:
helm upgrade cilium cilium/cilium -n kube-system \
  --reuse-values \
  --set l2announcements.enabled=true \
  --set l2announcements.leaseDuration=3s \
  --set l2announcements.leaseRenewDeadline=1s \
  --set l2announcements.leaseRetryPeriod=200ms \
  --set devices='{eth0}' \
  --set externalIPs.enabled=true

# Restart Cilium daemonset to apply changes
kubectl rollout restart ds/cilium -n kube-system
kubectl rollout status -n kube-system ds/cilium --timeout=120s
```

### Apply L2 Configuration

```bash
# Apply cluster-scoped resources directly (not via kustomize)
kubectl apply -f deploy/k8s/overlays/pi-home/cilium-l2.yaml

# Verify IP pool
kubectl get ciliumloadbalancerippool
# Expected: DISABLED=false, IPS AVAILABLE=11

# Verify L2 lease is acquired
kubectl get leases -n kube-system | grep l2announce
# Expected: cilium-l2announce-ztna-intermediate-server with a holder node
```

### Verify L2 Announcement is Active

```bash
# Check which node holds the L2 lease
kubectl get lease cilium-l2announce-ztna-intermediate-server -n kube-system \
  -o jsonpath='{.spec.holderIdentity}'

# Check L2 announcement table on lease holder
LEASE_NODE=$(kubectl get lease cilium-l2announce-ztna-intermediate-server -n kube-system -o jsonpath='{.spec.holderIdentity}')
kubectl -n kube-system exec $(kubectl get pods -n kube-system -l k8s-app=cilium --field-selector spec.nodeName=$LEASE_NODE -o name | head -1) \
  -- cilium-dbg shell -- db/show l2-announce

# Expected output:
# IP             NetworkInterface
# 10.0.150.205   eth0
```

### Change LoadBalancer IP

```bash
# Patch service with new IP from the pool
kubectl patch svc intermediate-server -n ztna \
  -p '{"metadata":{"annotations":{"io.cilium/lb-ipam-ips":"10.0.150.205"}}}'

# Verify new IP
kubectl get svc -n ztna intermediate-server
```

### Key Insight: Cluster-Scoped Resources

**IMPORTANT**: CiliumL2AnnouncementPolicy and CiliumLoadBalancerIPPool are **cluster-scoped** (no namespace).

If you include them in a namespace-scoped kustomization, kustomize will add `namespace: ztna` which will be ignored but causes confusion. Better to apply them separately:

```bash
# Apply Cilium resources separately
kubectl apply -f deploy/k8s/overlays/pi-home/cilium-l2.yaml

# Then apply ZTNA components
kubectl apply -k deploy/k8s/overlays/pi-home
```

---

## Building Multi-Arch Images

### Docker Buildx Setup

```bash
# Check buildx is available
docker buildx version

# Create a builder for multi-arch
docker buildx create --name ztna-builder --use --bootstrap

# List builders
docker buildx ls
```

### Login to Container Registry

```bash
# Docker Hub
docker login docker.io -u YOUR_USERNAME

# GitHub Container Registry (alternative)
echo $GITHUB_TOKEN | docker login ghcr.io -u YOUR_USERNAME --password-stdin
```

### Build and Push Images

```bash
# Build all images for arm64 and push to Docker Hub
./deploy/k8s/build-push.sh --arm64-only

# Build specific component
./deploy/k8s/build-push.sh --arm64-only intermediate

# Build without pushing (local testing)
./deploy/k8s/build-push.sh --no-push --arm64-only

# Build for multiple platforms
./deploy/k8s/build-push.sh  # Builds linux/arm64,linux/amd64
```

### Build Script Options

| Option | Description |
|--------|-------------|
| `--no-push` | Build only, don't push to registry |
| `--arm64-only` | Build only for arm64 (faster) |
| `--tag TAG` | Use specific tag (default: latest) |
| `--registry REG` | Use specific registry (default: docker.io) |
| `--owner OWNER` | Use specific owner/username |

### Image Names

Default images built:
- `docker.io/hyeomans/ztna-intermediate-server:latest`
- `docker.io/hyeomans/ztna-app-connector:latest`
- `docker.io/hyeomans/ztna-echo-server:latest`

### Making Docker Hub Repos Public

After pushing, images are **private by default** on Docker Hub. To make them pullable:

1. Go to https://hub.docker.com/r/YOUR_USERNAME/ztna-intermediate-server
2. Settings → Visibility → Make Public
3. Repeat for other repos

---

## TLS Certificate Management

### Generate Self-Signed Certificates

```bash
# Intermediate server certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout intermediate-key.pem \
  -out intermediate-cert.pem \
  -days 365 -nodes \
  -subj "/CN=ztna-intermediate"

# Connector certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout connector-key.pem \
  -out connector-cert.pem \
  -days 365 -nodes \
  -subj "/CN=ztna-connector"
```

### Create Kubernetes Secrets

```bash
# Create namespace first
kubectl create namespace ztna

# Create TLS secrets
kubectl create secret tls ztna-intermediate-tls -n ztna \
  --cert=intermediate-cert.pem \
  --key=intermediate-key.pem

kubectl create secret tls ztna-connector-tls -n ztna \
  --cert=connector-cert.pem \
  --key=connector-key.pem

# Verify secrets
kubectl get secrets -n ztna
```

### Clean Up Local Cert Files

```bash
rm -f intermediate-*.pem connector-*.pem
```

---

## Deployment Commands

### Preview Deployment

```bash
# See what will be deployed without applying
kubectl kustomize deploy/k8s/overlays/pi-home
```

### Deploy Components

```bash
# Apply Cilium L2 config first (cluster-scoped)
kubectl apply -f deploy/k8s/overlays/pi-home/cilium-l2.yaml

# Apply ZTNA components
kubectl apply -k deploy/k8s/overlays/pi-home
```

### Check Deployment Status

```bash
# Watch pods
kubectl get pods -n ztna -w

# Check services (LoadBalancer should get external IP)
kubectl get svc -n ztna

# Detailed pod status
kubectl describe pod -n ztna -l app.kubernetes.io/name=intermediate-server
```

### View Logs

```bash
# Intermediate server logs
kubectl logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# App connector logs
kubectl logs -n ztna -l app.kubernetes.io/name=app-connector -f

# Echo server logs
kubectl logs -n ztna -l app.kubernetes.io/name=echo-server -f

# All ZTNA logs
kubectl logs -n ztna -l app.kubernetes.io/part-of=ztna-system -f --prefix
```

### Restart Deployments

```bash
# Restart all deployments (useful after image update)
kubectl rollout restart deployment -n ztna intermediate-server app-connector echo-server

# Watch rollout status
kubectl rollout status deployment -n ztna intermediate-server
```

### Delete Deployment

```bash
# Delete ZTNA components (keeps namespace and secrets)
kubectl delete -k deploy/k8s/overlays/pi-home

# Delete everything including namespace
kubectl delete namespace ztna

# Delete Cilium L2 resources
kubectl delete -f deploy/k8s/overlays/pi-home/cilium-l2.yaml
```

---

## Troubleshooting

### Image Pull Errors

**Symptom**: `ErrImagePull` or `ImagePullBackOff`

**Common Causes**:
1. Image doesn't exist on registry
2. Repository is private
3. Image not built for arm64

**Solutions**:

```bash
# Check image exists
docker manifest inspect docker.io/hyeomans/ztna-intermediate-server:latest

# Make Docker Hub repos public (see Build section)

# Or create image pull secret
kubectl create secret docker-registry dockerhub-secret \
  -n ztna \
  --docker-server=docker.io \
  --docker-username=YOUR_USERNAME \
  --docker-password=YOUR_TOKEN

# Then add to deployment spec:
# spec.template.spec.imagePullSecrets:
#   - name: dockerhub-secret
```

### LoadBalancer No External IP

**Symptom**: Service shows `<pending>` for EXTERNAL-IP

**Check Cilium L2**:

```bash
# Verify L2 policy exists
kubectl get ciliuml2announcementpolicy

# Verify IP pool has available IPs
kubectl get ciliumloadbalancerippool
# IPS AVAILABLE should be > 0

# Check Cilium status
kubectl -n kube-system exec -it ds/cilium -- cilium status
```

### Pod CrashLoopBackOff

**Check logs**:

```bash
kubectl logs -n ztna -l app.kubernetes.io/name=intermediate-server --previous

# Check events
kubectl get events -n ztna --sort-by='.lastTimestamp'
```

### Connection Refused from macOS

**Check network path**:

```bash
# From macOS, test UDP connectivity
nc -u -v -z 10.0.150.205 4433

# Check if service has endpoints
kubectl get endpoints -n ztna intermediate-server

# Check pod is running
kubectl get pods -n ztna -o wide
```

### Destination Host Unreachable from macOS

**Symptom**: Ping/UDP shows "Destination Host Unreachable" despite ARP working

**Root Cause**: Usually `externalTrafficPolicy: Local` with L2 announcements where lease holder ≠ pod node

**Diagnosis**:

```bash
# 1. Check ARP is working (MAC should resolve to a Pi node)
arp -an | grep "10.0.150.205"

# 2. Check which node holds L2 lease
kubectl get lease cilium-l2announce-ztna-intermediate-server -n kube-system \
  -o jsonpath='{.spec.holderIdentity}'

# 3. Check which node has the pod
kubectl get pods -n ztna -l app.kubernetes.io/name=intermediate-server -o wide

# 4. If different nodes → traffic policy is the issue
kubectl get svc -n ztna intermediate-server \
  -o jsonpath='{.spec.externalTrafficPolicy}'
```

**Fix**:

```bash
# Change to Cluster policy
kubectl patch svc -n ztna intermediate-server \
  -p '{"spec":{"externalTrafficPolicy":"Cluster"}}'
```

### L2 Announcements Not Working

**Symptom**: LoadBalancer has IP but not reachable, ARP fails

**Check L2 is enabled in Cilium**:

```bash
kubectl get cm cilium-config -n kube-system -o yaml | grep -i l2

# If "enable-l2-announcements: false", enable via Helm (see Cilium L2 Configuration section)
```

**Check BPF L2 responder map**:

```bash
# Get node holding lease
LEASE_NODE=$(kubectl get lease cilium-l2announce-ztna-intermediate-server -n kube-system -o jsonpath='{.spec.holderIdentity}')

# Check L2 responder map on that node
kubectl exec -n kube-system $(kubectl get pods -n kube-system -l k8s-app=cilium --field-selector spec.nodeName=$LEASE_NODE -o name | head -1) \
  -- bpftool map dump pinned /sys/fs/bpf/tc/globals/cilium_l2_responder_v4
```

---

## Lessons Learned

### 1. Cilium L2 Resources are Cluster-Scoped

CiliumL2AnnouncementPolicy and CiliumLoadBalancerIPPool don't have namespaces. Apply them separately from the namespaced resources to avoid confusion:

```bash
# Good: Apply separately
kubectl apply -f cilium-l2.yaml
kubectl apply -k overlays/pi-home

# Bad: Including in kustomization adds namespace metadata that gets ignored
```

### 2. Docker Hub Images are Private by Default

When you push to Docker Hub, repos are created as **private**. You must manually make them public via the web UI, or create imagePullSecrets.

### 3. kustomize commonLabels is Deprecated

Use `labels` instead of `commonLabels`:

```yaml
# Deprecated
commonLabels:
  app: myapp

# Recommended
labels:
  - pairs:
      app: myapp
    includeSelectors: false
```

### 4. Create TLS Secrets Before Kustomize Apply

Kustomize base includes placeholder secrets. Better to:
1. Remove secrets.yaml from base resources
2. Create secrets manually before applying
3. This avoids overwriting real secrets with placeholders

### 5. Pi Cluster Uses eth0 for Physical Network

When configuring Cilium L2 interfaces, Pi nodes use `eth0` for the physical network. Use regex `^eth0$` in the L2 announcement policy.

### 6. externalTrafficPolicy: Cluster Required for L2 Announcements

**CRITICAL**: With Cilium L2 announcements, the node holding the L2 lease (responding to ARP) may NOT be the node running the pod.

With `externalTrafficPolicy: Local`:
- Traffic arrives at the L2 lease holder node
- If the pod isn't on that node, traffic is **dropped**
- Results in "Destination Host Unreachable" errors

**Solution**: Use `Cluster` policy:

```yaml
spec:
  externalTrafficPolicy: Cluster
```

**Trade-off**: Client source IP is NATted (affects QAD observed addresses). The intermediate server will see Cilium's internal IP (e.g., 10.0.0.22) instead of the actual client IP.

**Debugging this issue**:
```bash
# Find which node has the L2 lease
kubectl get lease cilium-l2announce-ztna-intermediate-server -n kube-system \
  -o jsonpath='{.spec.holderIdentity}'

# Find which node runs the pod
kubectl get pods -n ztna -l app.kubernetes.io/name=intermediate-server -o wide

# If different nodes → externalTrafficPolicy: Local won't work
```

### 7. Multi-Arch Builds Need Buildx

Standard `docker build` only builds for the host architecture. Use `docker buildx build --platform linux/arm64` for Pi images from a Mac.

### 8. k3s Uses Contained instead of Docker

k3s nodes don't have Docker - they use containerd. This means:
- Can't use `docker pull` on nodes for debugging
- Use `crictl` instead for container operations
- Image pulls go through containerd, not Docker daemon

### 9. LoadBalancer IPs Must Be on Same Subnet as Nodes

For L2 announcements to work, the LoadBalancer IP pool must be on the **same subnet** as the k8s nodes:

```
Nodes: 10.0.150.101-108
IP Pool: 10.0.150.200-210  ← Same /24 subnet
```

**Why**: L2 announcements use ARP, which only works within a broadcast domain (same subnet). The node responds to ARP requests for the LoadBalancer IP using its own MAC address.

**Router Configuration**: The router (10.0.150.1) does NOT need any special configuration:
- L2/ARP happens directly between client (Mac) and k8s node
- Router is not involved in L2 traffic forwarding
- No static routes or NAT rules needed

### 10. Debugging Cilium L2 BPF State

To verify L2 announcements are correctly programmed in Cilium's BPF maps:

```bash
# Check L2 responder map (shows IPs Cilium will respond to ARP for)
kubectl exec -n kube-system <cilium-pod> -- \
  bpftool map dump pinned /sys/fs/bpf/tc/globals/cilium_l2_responder_v4

# Key format: IP address in hex (e.g., 0a 00 96 cd = 10.0.150.205)
# Value: 01 = enabled

# Check LB BPF map (shows service-to-backend mappings)
kubectl exec -n kube-system <cilium-pod> -- \
  cilium-dbg bpf lb list | grep "10.0.150"
```

### 11. QUIC Keepalive Required for Long-Running Connections

**Problem**: QUIC has a 30-second idle timeout. Pods without traffic disconnect and restart.

**Symptom**: app-connector pod shows `CrashLoopBackOff` with logs showing connection closed after 30s.

**Solution**: Implement QUIC keepalive (PING frames) in long-running components:

```rust
// In app-connector/src/main.rs
const KEEPALIVE_INTERVAL_SECS: u64 = 10;  // Less than half of idle timeout

fn maybe_send_keepalive(&mut self) {
    if self.last_keepalive.elapsed().as_secs() >= KEEPALIVE_INTERVAL_SECS {
        if let Some(ref mut conn) = self.intermediate_conn {
            if conn.is_established() {
                let _ = conn.send_ack_eliciting();  // Sends QUIC PING
            }
        }
        self.last_keepalive = Instant::now();
    }
}
```

### 12. Docker Entrypoint vs K8s SecurityContext

**Problem**: Docker images that use `gosu` or `su` to drop privileges fail in k8s with "operation not permitted".

**Root Cause**: k8s `securityContext` already runs containers as non-root (UID 1000). Using `gosu` inside the container tries to switch users, which is not permitted.

**Symptom**:
```
app-connector   0/1     CrashLoopBackOff   3 (25s ago)
Logs: "operation not permitted"
```

**Solution**: Override the entrypoint via kustomize patch to skip the privilege-dropping script:

```yaml
# In overlays/pi-home/kustomization.yaml
patches:
  - patch: |-
      - op: add
        path: /spec/template/spec/containers/0/command
        value: ["/usr/local/bin/app-connector"]
    target:
      kind: Deployment
      name: app-connector
```

**Debugging**:
```bash
# Check if container is trying to switch users
kubectl logs -n ztna deployment/app-connector

# Verify securityContext in deployment
kubectl get deployment app-connector -n ztna -o jsonpath='{.spec.template.spec.securityContext}'
```

### 13. Keep macOS Agent Config in Sync with K8s LoadBalancer IP

**Problem**: macOS Agent has hardcoded server IP that must match the k8s LoadBalancer.

**Files to keep in sync**:
1. `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`:
   ```swift
   private let serverHost = "10.0.150.205"  // Must match k8s
   ```
2. `deploy/k8s/overlays/pi-home/kustomization.yaml`:
   ```yaml
   - op: replace
     path: /metadata/annotations/io.cilium~1lb-ipam-ips
     value: "10.0.150.205"  # Must match Swift
   ```

**Symptom if mismatched**: VPN shows "connected" but no QUIC connection. Check intermediate logs - no new connections.

**⚠️ TECHNICAL DEBT**: This hardcoding approach doesn't scale for cloud deployment. Before deploying to DigitalOcean/AWS:
- macOS Agent should read server address from configuration (UserDefaults, plist, or MDM profile)
- Each environment needs its own kustomize overlay with correct IPs
- See `tasks/006-cloud-deployment/todo.md` Phase 1.3 for full plan

### 14. Building and Pushing Updated Images After Code Changes

After modifying component code (e.g., adding keepalive to app-connector):

```bash
# 1. Build and push updated image
cd /path/to/ztna-agent
./deploy/k8s/build-push.sh --arm64-only app-connector

# 2. Or build manually
docker buildx build --platform linux/arm64 \
  -t docker.io/hyeomans/ztna-app-connector:latest \
  -f deploy/docker-nat-sim/Dockerfile.connector \
  --push .

# 3. Force k8s to pull new image
kubectl rollout restart deployment app-connector -n ztna

# 4. Watch pod restart
kubectl get pods -n ztna -w
```

---

## Quick Reference Commands

```bash
# === Cluster Status ===
kubectl get nodes -o wide
kubectl get pods -n ztna
kubectl get svc -n ztna
kubectl get events -n ztna --sort-by='.lastTimestamp'

# === Cilium Status ===
kubectl get ciliuml2announcementpolicy
kubectl get ciliumloadbalancerippool

# === Image Management ===
./deploy/k8s/build-push.sh --arm64-only
kubectl rollout restart deployment -n ztna --all

# === Logs ===
kubectl logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# === Cleanup ===
kubectl delete namespace ztna
kubectl delete -f deploy/k8s/overlays/pi-home/cilium-l2.yaml
```

---

## macOS ZtnaAgent E2E Testing

### E2E Test Setup

1. **Configure Extension with k8s IP:**
   ```swift
   // In PacketTunnelProvider.swift
   private let serverHost = "10.0.150.205"  // k8s LoadBalancer IP
   private let serverPort: UInt16 = 4433
   ```

2. **Clean and rebuild macOS app:**
   ```bash
   # Clean old cached Extension (critical!)
   rm -rf ~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*

   # Rebuild
   xcodebuild -project ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj \
       -scheme ZtnaAgent -configuration Debug \
       -derivedDataPath /tmp/ZtnaAgent-build build
   ```

3. **Launch and connect:**
   ```bash
   open /tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app --args --auto-start
   ```

### Verify E2E Connection

```bash
# Check VPN tunnel is up
ifconfig utun6
# Expected: inet 100.64.0.1

# Check UDP connection to k8s
netstat -an | grep "10.0.150.205.4433"

# Check k8s intermediate logs
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server --tail=20
# Expected: "New connection from 10.0.0.XX:XXXXX"
```

### E2E Test Results (Phase 1.5 - FULL SUCCESS)

| Test | Result | Notes |
|------|--------|-------|
| VPN tunnel creation | ✅ | No popup dialog on macOS 26+ |
| QUIC handshake | ✅ | Connects to LoadBalancer IP |
| QAD (address discovery) | ✅ | Observed address returned (SNAT'd) |
| Packet tunneling | ✅ | DATAGRAMs sent through tunnel |
| Agent registration | ✅ | Agent registers for 'echo-service' |
| Connector registration | ✅ | Connector registers for 'echo-service' |
| **Relay routing** | ✅ | **Full E2E relay working!** |
| **Return traffic** | ✅ | **Echo response returns to Agent** |

### E2E Test Procedure

```bash
# 1. Reconnect VPN to establish fresh QUIC connection
networksetup -disconnectpppoeservice "ZTNA Agent"
sleep 2
networksetup -connectpppoeservice "ZTNA Agent"
sleep 3

# 2. Verify Agent registration
kubectl logs -n ztna deployment/intermediate-server --tail=10 | grep Agent
# Expected: "Registration: Agent for service 'echo-service'"

# 3. Send UDP test traffic (route goes through VPN tunnel)
# 10.100.0.0/24 is the ZTNA virtual service range (routed through tunnel)
# 10.100.0.1 = echo-service (UDP 9999)
echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999

# 4. Check relay logs
kubectl logs -n ztna deployment/intermediate-server --tail=15 | grep -E "relay|destination"
# Expected:
#   "Received 38 bytes to relay from ..."
#   "Found destination ... for ..."
#   "Relayed 38 bytes from ... to ..."
```

### E2E Success Log Sample (2026-01-25)

```
[21:02:13Z] Received 38 bytes to relay from aa7443... (Agent)
[21:02:13Z] Found destination e8780... for aa7443...
[21:02:13Z] Relayed 38 bytes from aa7443... to e8780... (→ Connector)
[21:02:13Z] Received 38 bytes to relay from e8780... (echo response)
[21:02:13Z] Found destination 176b5... for e8780...
[21:02:13Z] Relayed 38 bytes from e8780... to 176b5... (→ Agent)
```

### Known Issues (Updated 2026-01-25)

1. **Extension caching**: macOS caches the Extension binary path. After code changes, you MUST:
   - Delete `~/Library/Developer/Xcode/DerivedData/ZtnaAgent-*`
   - Rebuild to `/tmp/ZtnaAgent-build`
   - Stop and restart VPN

2. **~~macOS Agent 30-second idle timeout~~**: ✅ **FIXED** (2026-01-25)
   - Agent now sends keepalive PING every 10 seconds
   - Connection stays alive indefinitely without traffic
   - Rebuild required to get the fix: `/Applications/ZtnaAgent.app`

3. **SNAT hides client IP**: With `externalTrafficPolicy: Cluster`, intermediate sees k8s node IP (10.0.0.22) not the real client IP.

4. **UDP only**: Connector currently only forwards UDP traffic. TCP/ICMP packets are dropped.
   - Connector extracts UDP payload from IP packet
   - Forwards UDP payload to echo-server
   - TCP support is future work

### Routing Architecture (Implicit Single-Service-Per-Connection)

The relay routing uses an implicit model:
```
1. Agent registers: "I want to reach service 'echo-service'" (DATAGRAM 0x10)
2. Connector registers: "I handle service 'echo-service'" (DATAGRAM 0x11)
3. When Agent sends DATAGRAM (IP packet):
   - Intermediate looks up Agent's registered service
   - Finds Connector for that service
   - Forwards DATAGRAM to Connector
4. Return traffic follows reverse path
```

**No per-packet service ID needed** - the connection registration determines routing.

---

---

## Human Demo Guide: Multi-Terminal E2E Demo

This section provides copy-paste commands for running a live demo across multiple terminals. Perfect for showing the ZTNA stack to others.

### Prerequisites

1. **Pi k8s cluster running** with ZTNA deployed
2. **macOS Agent app** built and installed at `/Applications/ZtnaAgent.app`
3. **Terminal app** (or iTerm2 for better multi-pane support)

### Terminal Layout (4 Terminals)

```
┌─────────────────────────────────────┬─────────────────────────────────────┐
│ Terminal 1: K8s Intermediate Logs   │ Terminal 2: K8s Connector Logs      │
│ (Relay hub - see connections)       │ (Service handler)                   │
├─────────────────────────────────────┼─────────────────────────────────────┤
│ Terminal 3: K8s Echo Server Logs    │ Terminal 4: macOS Commands          │
│ (Backend service)                   │ (VPN control + test traffic)        │
└─────────────────────────────────────┴─────────────────────────────────────┘
```

### Step-by-Step Demo Commands

**Terminal 1 - Intermediate Server Logs (Start First)**
```bash
# Watch the relay hub - shows all connections and relayed traffic
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f --tail=50
```
*Look for: "New connection", "Registration", "Relayed X bytes"*

---

**Terminal 2 - App Connector Logs**
```bash
# Watch the service handler - shows registration and forwarding
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=app-connector -f --tail=50
```
*Look for: "Registered as Connector for 'echo-service'", "QAD: Observed address"*

---

**Terminal 3 - Echo Server Logs**
```bash
# Watch the backend service - shows received UDP packets
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=echo-server -f --tail=50
```
*Look for: "Received X bytes", "Echoing back"*

---

**Terminal 4 - macOS Commands (Run Demo)**
```bash
# === PREPARATION ===
# Check VPN status
scutil --nc list | grep ZTNA

# === START THE DEMO ===
# 1. Disconnect any existing VPN connection
networksetup -disconnectpppoeservice "ZTNA Agent" 2>/dev/null || true

# 2. Wait for clean state
sleep 2

# 3. Connect VPN (starts QUIC connection to k8s)
networksetup -connectpppoeservice "ZTNA Agent"

# 4. Wait for connection establishment
sleep 3

# 5. Verify VPN tunnel is up
ifconfig utun6 | grep inet
# Expected: inet 100.64.0.1 --> 100.64.0.1 netmask 0xffffffff

# 6. Verify UDP socket to k8s
netstat -an | grep "10.0.150.205.4433"
# Expected: udp connection entry

# === SEND TEST TRAFFIC ===
# 7. Send UDP through the tunnel (routed via VPN to echo-service)
echo "ZTNA-DEMO-$(date +%H%M%S)" | nc -u -w2 10.100.0.1 9999

# 8. Send multiple test packets
for i in 1 2 3; do
  echo "ZTNA-TEST-$i" | nc -u -w1 10.100.0.1 9999
  sleep 1
done

# === CLEANUP ===
# 9. Disconnect VPN when done
networksetup -disconnectpppoeservice "ZTNA Agent"
```

### What to Point Out During Demo

**When VPN connects (Terminal 1 shows):**
```
[INFO] New connection from 10.0.0.22:XXXXX (scid=...)
[INFO] Sent QAD to ... (observed: 10.0.0.22:XXXXX)
[INFO] Registration: Agent for service 'echo-service' from ...
```

**When test traffic is sent (Terminal 1 shows):**
```
[INFO] Received 43 bytes to relay from aa7443... (Agent)
[INFO] Found destination e8780... for aa7443...
[INFO] Relayed 43 bytes from aa7443... to e8780... (→ Connector)
[INFO] Received 43 bytes to relay from e8780... (echo response)
[INFO] Relayed 43 bytes from e8780... to 176b5... (→ Agent)
```

**Terminal 2 shows:**
```
[INFO] Forwarding 15 bytes UDP to echo-server:9999
[INFO] Received response: 15 bytes from echo-server
```

**Terminal 3 shows:**
```
Received 15 bytes: ZTNA-DEMO-163025
Echoing back...
```

### Quick Status Check Commands

```bash
# === K8s Status ===
# All pods running?
kubectl --context k8s1 get pods -n ztna -o wide

# LoadBalancer IP assigned?
kubectl --context k8s1 get svc -n ztna intermediate-server

# Recent events?
kubectl --context k8s1 get events -n ztna --sort-by='.lastTimestamp' | tail -10

# === Network Verification ===
# Can reach LoadBalancer?
nc -u -v -z 10.0.150.205 4433

# ARP working?
arp -an | grep "10.0.150.205"

# === macOS Agent Status ===
# VPN configuration exists?
scutil --nc list | grep -i ztna

# Extension process running?
pgrep -fl Extension | grep -i ztna

# System logs (last 1 minute)
log show --last 1m --predicate 'subsystem CONTAINS "ztna"' --info
```

### Troubleshooting During Demo

| Symptom | Quick Fix |
|---------|-----------|
| VPN won't connect | `networksetup -disconnectpppoeservice "ZTNA Agent"` then retry |
| No traffic in logs | Check `ifconfig utun6` - tunnel might not be up |
| "Destination unreachable" | Verify k8s pods are Running: `kubectl get pods -n ztna` |
| 30s timeout disconnect | **Expected** - send traffic or wait for keepalive (if enabled) |
| Connector CrashLoop | **Expected** without traffic - it reconnects automatically |

### Demo Reset (Clean Slate)

```bash
# 1. Stop macOS VPN
networksetup -disconnectpppoeservice "ZTNA Agent" 2>/dev/null

# 2. Restart k8s deployments (forces fresh connections)
kubectl --context k8s1 rollout restart deployment -n ztna --all

# 3. Wait for pods to be ready
kubectl --context k8s1 wait --for=condition=ready pod -l app.kubernetes.io/part-of=ztna-system -n ztna --timeout=60s

# 4. Verify all running
kubectl --context k8s1 get pods -n ztna
```

### Extended Demo: Keepalive Verification

After the macOS Agent keepalive was implemented (2026-01-25), verify it works:

```bash
# Terminal 1: Watch intermediate logs
kubectl --context k8s1 logs -n ztna -l app.kubernetes.io/name=intermediate-server -f

# Terminal 4: Connect and wait
networksetup -connectpppoeservice "ZTNA Agent"

# Wait 45+ seconds (should stay connected past 30s idle timeout)
# Watch Terminal 1 for keepalive PING activity every 10 seconds

# Verify still connected after 60 seconds
sleep 60
scutil --nc status "ZTNA Agent" | grep -i status
# Should show: Connected
```

---

## P2P Hole Punching Status

> **Current Status:** Protocol IMPLEMENTED, Real NAT Testing NOT YET DONE

### What's Complete

| Component | Status | Details |
|-----------|--------|---------|
| P2P Protocol (Task 005) | ✅ Done | 79 unit tests, signaling, connectivity checks |
| Docker NAT Simulation | ✅ Done | Local testing with simulated NAT |
| Relay Path (E2E) | ✅ Done | macOS → k8s Intermediate → Connector → Echo |
| macOS Agent Keepalive | ✅ Done | 10s PING interval prevents idle timeout |

### What's NOT Done (Phase 6)

| Test | Status | Notes |
|------|--------|-------|
| Real NAT hole punching | ❌ Pending | Requires Agent behind real NAT, Connector on different network |
| Direct path verification | ❌ Pending | Need to verify "Path selected: DIRECT" in logs |
| NAT classification | ❌ Pending | Run pystun3, document NAT type |
| Hairpin NAT test | ❌ Pending | Both behind same NAT (home router) |
| Fallback test | ❌ Pending | Block direct, verify relay fallback |

### Why P2P Testing is Deferred

Current topology has both Agent and Connector on the same home network:
- macOS Agent: 10.0.150.x (home WiFi)
- K8s Cluster: 10.0.150.101-108 (home LAN)
- Same NAT: Both behind home router

**For real P2P hole punching**, we need:
1. Agent behind home NAT
2. Connector on **different** network (cloud VM, mobile hotspot, or different location)
3. Intermediate Server accessible to both

### Next Steps for P2P Testing

1. Deploy Intermediate Server to cloud (DigitalOcean/AWS)
2. Deploy App Connector to cloud (same VM for simplicity)
3. Keep macOS Agent behind home NAT
4. Test P2P candidate exchange and direct path establishment
5. Verify direct connection bypasses relay

---

## Related Documentation

- [Main README](./README.md) - Deployment quick start
- [Task 006 Context](../../tasks/006-cloud-deployment/) - Original task documentation
- [ZTNA Architecture](../../docs/architecture.md) - System design
- [Cilium L2 Docs](https://docs.cilium.io/en/stable/network/l2-announcements/) - Official Cilium L2 guide
