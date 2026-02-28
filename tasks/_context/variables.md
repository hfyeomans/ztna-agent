# ZTNA Infrastructure Variables Dictionary

**Purpose:** Single reference for all shell variables used across demo runbooks, testing guides, and deployment docs.
**Canonical definitions:** `docs/demo-runbook.md` § Configuration and `tasks/_context/testing-guide.md` § Configuration.
**Last Updated:** 2026-02-28

---

## AWS Infrastructure

| Variable | Current Value | Description |
|----------|---------------|-------------|
| `ZTNA_SSH_KEY` | `~/.ssh/hfymba.aws.pem` | Path to AWS EC2 SSH private key |
| `ZTNA_SSH_HOST` | `ubuntu@10.0.2.126` | SSH user@host (private/Tailscale IP — SSH access only) |
| `ZTNA_SSH` | `ssh -i $ZTNA_SSH_KEY $ZTNA_SSH_HOST` | Shorthand SSH command |
| `ZTNA_PUBLIC_IP` | `3.128.36.92` | AWS Elastic IP (public — used for QUIC connections) |
| `ZTNA_QUIC_PORT` | `4433` | Intermediate Server QUIC listen port |

## Metrics & Health

| Variable | Current Value | Description |
|----------|---------------|-------------|
| `ZTNA_INTERMEDIATE_BIND` | `10.0.2.126` | Address the Intermediate binds to (metrics bind here too) |
| `ZTNA_INTERMEDIATE_METRICS_PORT` | `9090` | Intermediate metrics/health HTTP port |
| `ZTNA_CONNECTOR_METRICS_PORT` | `9091` | Connector (echo-service) metrics/health HTTP port |
| `ZTNA_CONNECTOR_WEB_METRICS_PORT` | `9092` | Connector (web-app) metrics/health HTTP port |

**Note:** Intermediate metrics bind to `ZTNA_INTERMEDIATE_BIND`, not `0.0.0.0`. Connector metrics bind to `0.0.0.0` (accessible via `localhost`).

## Virtual Service IPs

| Variable | Current Value | Description |
|----------|---------------|-------------|
| `ZTNA_ECHO_VIRTUAL_IP` | `10.100.0.1` | Virtual IP for echo-service (routed through QUIC tunnel) |
| `ZTNA_WEB_VIRTUAL_IP` | `10.100.0.2` | Virtual IP for web-app (routed through QUIC tunnel) |
| `ZTNA_WEB_PORT` | `8080` | HTTP port for web-app backend |

**Note:** These IPs exist only inside the split-tunnel (10.100.0.0/24 → utun). They are not real network addresses.

## k8s Cluster (Pi)

| Variable | Current Value | Description |
|----------|---------------|-------------|
| `K8S_LB_HOST` | `10.0.150.205` | k8s LoadBalancer IP (Cilium L2, Pi cluster) |
| `K8S_LB_PORT` | `4433` | k8s Intermediate Server service port |

**Note:** Pi cluster is a separate deployment environment from AWS. These variables apply only to k8s demo scenarios.

---

## Where Variables Are Used

| File | Has Config Section? | Uses Variables? |
|------|--------------------|-----------------|
| `docs/demo-runbook.md` | Yes (canonical) | All commands parameterized |
| `tasks/_context/testing-guide.md` | Yes (mirrors runbook) | All commands parameterized |
| `tasks/_context/README.md` | No (references runbook) | All commands parameterized |
| `tasks/_context/components.md` | No (references runbook) | Descriptive only, cross-references runbook |

## Quick Setup

Copy-paste this block before running any demo or test commands:

```bash
export ZTNA_SSH_KEY="~/.ssh/hfymba.aws.pem"
export ZTNA_SSH_HOST="ubuntu@10.0.2.126"
export ZTNA_SSH="ssh -i $ZTNA_SSH_KEY $ZTNA_SSH_HOST"
export ZTNA_PUBLIC_IP="3.128.36.92"
export ZTNA_QUIC_PORT="4433"
export K8S_LB_HOST="10.0.150.205"
export K8S_LB_PORT="4433"
export ZTNA_ECHO_VIRTUAL_IP="10.100.0.1"
export ZTNA_WEB_VIRTUAL_IP="10.100.0.2"
export ZTNA_WEB_PORT="8080"
export ZTNA_INTERMEDIATE_BIND="10.0.2.126"
export ZTNA_INTERMEDIATE_METRICS_PORT="9090"
export ZTNA_CONNECTOR_METRICS_PORT="9091"
export ZTNA_CONNECTOR_WEB_METRICS_PORT="9092"
```

## Updating After Infrastructure Changes

If infrastructure changes (new EC2, new IP, new key):

1. Update the values in this file
2. Update `docs/demo-runbook.md` § Configuration
3. Update `tasks/_context/testing-guide.md` § Configuration
4. Re-export the variables in your shell
