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

### Apply L2 Configuration

```bash
# Apply cluster-scoped resources directly (not via kustomize)
kubectl apply -f deploy/k8s/overlays/pi-home/cilium-l2.yaml

# Verify
kubectl get ciliumloadbalancerippool
# Expected: DISABLED=false, IPS AVAILABLE=11
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
nc -u -v 10.0.150.200 4433

# Check if service has endpoints
kubectl get endpoints -n ztna intermediate-server

# Check pod is running
kubectl get pods -n ztna -o wide
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

### 6. externalTrafficPolicy: Local for QAD

For QUIC Address Discovery (QAD) to work correctly, the LoadBalancer service must preserve client source IPs:

```yaml
spec:
  externalTrafficPolicy: Local
```

### 7. Multi-Arch Builds Need Buildx

Standard `docker build` only builds for the host architecture. Use `docker buildx build --platform linux/arm64` for Pi images from a Mac.

### 8. k3s Uses Contained instead of Docker

k3s nodes don't have Docker - they use containerd. This means:
- Can't use `docker pull` on nodes for debugging
- Use `crictl` instead for container operations
- Image pulls go through containerd, not Docker daemon

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

## Related Documentation

- [Main README](./README.md) - Deployment quick start
- [Task 006 Context](../../tasks/006-cloud-deployment/) - Original task documentation
- [ZTNA Architecture](../../docs/architecture.md) - System design
- [Cilium L2 Docs](https://docs.cilium.io/en/stable/network/l2-announcements/) - Official Cilium L2 guide
