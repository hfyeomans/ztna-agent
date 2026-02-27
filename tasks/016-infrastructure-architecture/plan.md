# Plan: Infrastructure Architecture

**Description:** Plan for separating ZTNA components into independent, production-ready systems on AWS with an admin panel.

**Purpose:** Move from the single-EC2 MVP deployment to a proper distributed architecture where each component runs on its own infrastructure with appropriate isolation, scaling, and management.

---

## Task ID: 016-infrastructure-architecture
**Status:** Not Started
**Priority:** P2
**Depends On:** 007 ✅, 015 ✅
**Branch:** (not yet created)
**Last Updated:** 2026-02-27

---

## Scope

### In Scope
- Separate Intermediate Server, App Connector, and backend services onto independent infrastructure
- Containerize components (Docker images, CI builds)
- **Intermediate Server:** Docker on dedicated EC2 with host networking (`--net=host`) — Oracle recommended (see research.md)
- **App Connector:** ECS/Fargate (outbound QUIC, no inbound UDP needed) or k8s for on-prem/edge
- **Admin Panel:** ECS/Fargate behind ALB (standard web app pattern)
- Deploy a test backend (echo + HTTP smoke target, QuakeJS as demo layer) accessible through the tunnel
- Terraform for reproducible infrastructure (Oracle-recommended IaC)
- ZTNA client remains on local macOS

### Out of Scope
- Multi-tenancy for Intermediate Server (future — noted in research.md)
- Task 008 software-level operations (reconnection, metrics, graceful shutdown, UDP injection fix)
- Pi k8s changes (existing manifests continue to work)

### Multi-Tenancy Note
The Intermediate Server will eventually need multi-tenancy (per-tenant isolation, resource quotas, tenant-aware routing). This is NOT in scope for this task. Build a solid single-tenant distributed architecture first. Multi-tenancy will be layered on once the architecture is proven.

---

## Phases

### Phase 1: Containerization
- Create Dockerfiles for intermediate-server, app-connector
- Multi-stage builds (Rust builder → minimal runtime image)
- CI pipeline for building and pushing images (GitHub Actions → ECR)
- Test locally with docker-compose before deploying

### Phase 2: AWS Infrastructure (Terraform)
- Terraform modules for:
  - VPC / subnets / security groups (separate per component)
  - Dedicated EC2 for Intermediate Server (Elastic IP, host-networked Docker, UDP :4433)
  - ECS cluster + Fargate for App Connector, Admin Panel, test backends
  - ALB for Admin Panel (HTTPS :443)
  - ECR repositories for container images
  - IAM roles and policies
  - CloudWatch log groups

### Phase 3: Component Deployment
- Deploy Intermediate Server (public-facing, UDP :4433)
- Deploy App Connector (private, outbound QUIC to Intermediate)
- Deploy test backend (web server or QuakeJS, accessible through tunnel)
- Verify end-to-end: macOS Agent → Intermediate → Connector → Backend

### Phase 4: Admin Panel (MVP)
- REST API service for:
  - System health overview (component status, connection counts)
  - Policy definition (service registrations, allowed services)
  - Push policy to Intermediate Server and Connectors
- Web frontend (basic dashboard)
- Deploy on ECS/Fargate behind ALB

### Phase 5: Validation & Cutover
- E2E testing with distributed deployment
- Performance comparison vs single-EC2 baseline
- Tear down old single-EC2 deployment
- Update documentation and deploy skills

---

## Key Decisions — Resolved

1. **Intermediate Server packaging:** Docker on dedicated EC2 with `--net=host` (Oracle recommendation — best balance of CI/CD + bare-socket UDP performance)
2. **IaC tool:** Terraform (Oracle recommendation — mature modules for ECS/NLB/ALB/ECR/IAM/CloudWatch)
3. **Test backend:** Echo + HTTP/WebSocket smoke target first; QuakeJS as demo-only layer (Oracle recommendation)

## Key Decisions — Pending

1. **Admin Panel tech stack:** Rust (Axum/Actix) vs Node/TypeScript vs Go?
2. **Admin Panel control-plane contract:** Authz model, config versioning, rollback, ack/retry semantics for policy push
3. **Task ordering:** Task 008 (software ops) first, or Task 016 (infra architecture) first?

## Oracle-Identified Gaps (Must Address Before Execution)

1. Retry-token key management for rolling deploys / multi-instance (currently generated at process start, in-memory only)
2. Health-check/drain model for UDP service during deploys
3. App Connector placement should be topology-aware (near protected services)
4. Coordinate with Task 008 — minimal observability + graceful shutdown needed for infra cutover

---

## Success Criteria

- [ ] Each ZTNA component runs on independent infrastructure
- [ ] Intermediate Server has its own stable public IP
- [ ] App Connector runs separately, connects to Intermediate over the network (not localhost)
- [ ] Admin panel accessible for system health and policy management
- [ ] Test backend accessible through the ZTNA tunnel from macOS
- [ ] Infrastructure is reproducible via IaC (single command to deploy)
- [ ] Docker images built and pushed via CI
- [ ] E2E test passes with distributed deployment
