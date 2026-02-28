# ZTNA Agent Deployment

This directory contains all deployment tooling for the ZTNA Agent system. Choose the approach that fits your environment.

## Deployment Options

| Method | Directory | Best For |
|--------|-----------|----------|
| **Terraform + Ansible** | `terraform/` + `ansible/` | Production AWS (new infra from scratch) |
| **Ansible only** | `ansible/` | Deploying to existing EC2 instances |
| **Docker (production)** | `docker/` | Container-based deployments, CI image builds |
| **Docker NAT Sim** | `docker-nat-sim/` | Local NAT/P2P testing |
| **Kubernetes** | `k8s/` | k8s clusters (Pi home lab, cloud k8s) |
| **Manual AWS** | `aws/` | Reference scripts for manual EC2 setup |

## Quick Start

### Option A: Terraform + Ansible (Full Automation)

Provision AWS infrastructure with Terraform, then deploy services with Ansible.

```bash
# 1. Provision infrastructure
cd deploy/terraform
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your values
terraform init
terraform plan
terraform apply

# 2. Update Ansible inventory with Terraform output
terraform output public_ip
# Update deploy/ansible/inventory.ini with the IP

# 3. Deploy services
cd ../ansible
ansible-playbook playbook.yml
```

### Option B: Ansible Only (Existing Server)

Deploy to an existing Ubuntu server (e.g., the current AWS EC2 instance).

```bash
cd deploy/ansible

# Edit inventory.ini with your server details
# Then deploy:
ansible-playbook playbook.yml

# Rebuild and redeploy binaries only:
ansible-playbook playbook.yml --tags build -e force_build=true

# Restart services only:
ansible-playbook playbook.yml --tags services
```

### Option C: Docker Images

Build production container images for the Intermediate Server and App Connector.

```bash
# Build from repo root
docker build -f deploy/docker/Dockerfile.intermediate -t ztna-intermediate .
docker build -f deploy/docker/Dockerfile.connector -t ztna-connector .

# Run intermediate server
docker run -d --name ztna-intermediate \
  -p 4433:4433/udp \
  -v /path/to/certs:/etc/ztna/certs:ro \
  ztna-intermediate

# Run app connector
docker run -d --name ztna-connector \
  ztna-connector \
    --server <intermediate-host>:4433 \
    --service echo-service \
    --forward <backend-host>:8080
```

### Option D: Kubernetes

See `k8s/README.md` for Kustomize-based deployment to k8s clusters.

```bash
# Build and push multi-arch images
./deploy/k8s/build-push.sh

# Deploy
kubectl apply -k deploy/k8s/base/
```

## Directory Layout

```
deploy/
  ansible/                  # Ansible playbook and roles
    playbook.yml            #   Main playbook
    inventory.ini           #   Host inventory
    ansible.cfg             #   Ansible configuration
    roles/ztna/             #   ZTNA deployment role
      tasks/main.yml        #     Build, deploy, configure
      templates/            #     Jinja2 systemd unit templates
      handlers/main.yml     #     Service restart handlers
  terraform/                # Terraform IaC for AWS
    main.tf                 #   Provider configuration
    variables.tf            #   Input variables
    ec2.tf                  #   VPC, SG, EC2, EIP resources
    outputs.tf              #   Useful outputs (IP, IDs)
    user_data.sh.tftpl      #   EC2 bootstrap script template
    terraform.tfvars.example#   Example variable values
  docker/                   # Production Dockerfiles
    Dockerfile.intermediate #   Intermediate Server image
    Dockerfile.connector    #   App Connector image
  docker-nat-sim/           # Docker Compose NAT simulation (testing)
  k8s/                      # Kubernetes manifests (Kustomize)
  aws/                      # Manual AWS setup scripts
  config/                   # JSON configuration templates
```

## Architecture

All deployment methods target the same binary architecture:

```
                    Internet
                       |
              UDP 4433 | (QUIC)
                       v
            +-----------------------+
            | Intermediate Server   |  <-- Public-facing relay
            | (QUIC relay, mTLS)    |
            +-----------+-----------+
                        |
                  localhost/cluster
                        |
            +-----------+-----------+
            | App Connector         |  <-- Service mesh agent
            | (echo-service)        |
            +-----------+-----------+
                        |
                TCP localhost:8080
                        |
            +-----------+-----------+
            | Echo Server (Python)  |  <-- Backend service
            +-----------------------+
```

## Important Notes

- **Same-commit rule**: When deploying from a feature branch, ALWAYS deploy Intermediate Server and App Connector from the same commit. Mismatched binaries cause QUIC handshake failures.
- **TLS certificates**: Must be provisioned separately. Use `deploy/aws/setup-certbot.sh` for Let's Encrypt via Route53, or provide your own.
- **Secrets**: Never commit `.tfvars`, SSH keys, or certificate files. Use Ansible Vault or environment variables for sensitive values.
- **Firewall**: All methods configure UFW or security groups to allow UDP 4433 (QUIC) and TCP 22 (SSH).
