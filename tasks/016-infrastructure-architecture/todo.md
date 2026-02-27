# Todo: Infrastructure Architecture

**Description:** Implementation checklist for infrastructure separation.

**Purpose:** Track granular progress through each phase.

---

## Phase 0: Research & Planning

- [x] Document current single-EC2 architecture
- [x] Define target distributed architecture
- [x] Create task files (research, plan, state, todo, placeholder, depreciated)
- [ ] Oracle review: Docker vs bare EC2 for Intermediate Server
- [ ] Incorporate Oracle feedback into plan
- [ ] Finalize key decisions (tech stack, IaC tool, test backend)

## Phase 1: Containerization

- [ ] Create Dockerfile for intermediate-server (multi-stage Rust build)
- [ ] Create Dockerfile for app-connector (multi-stage Rust build)
- [ ] Create docker-compose.yml for local testing
- [ ] GitHub Actions workflow: build images → push to ECR
- [ ] Test containers locally (verify QUIC UDP works through Docker networking)

## Phase 2: AWS Infrastructure (IaC)

- [ ] Choose IaC tool (Terraform recommended)
- [ ] VPC / subnets / security groups
- [ ] ECS cluster + services (or EC2 — per Oracle)
- [ ] NLB for Intermediate Server (UDP :4433, Elastic IP)
- [ ] ALB for Admin Panel (HTTPS :443)
- [ ] ECR repositories
- [ ] IAM roles and policies
- [ ] CloudWatch log groups

## Phase 3: Component Deployment

- [ ] Deploy Intermediate Server (public-facing)
- [ ] Deploy App Connector (private, outbound QUIC)
- [ ] Deploy test backend (web server / QuakeJS)
- [ ] E2E verification: macOS Agent → Intermediate → Connector → Backend

## Phase 4: Admin Panel (MVP)

- [ ] Choose tech stack
- [ ] REST API: health endpoints, policy CRUD
- [ ] Web frontend: dashboard
- [ ] Deploy on ECS/Fargate behind ALB
- [ ] Connect to Intermediate Server for status/policy push

## Phase 5: Validation & Cutover

- [ ] E2E test suite with distributed deployment
- [ ] Performance comparison vs single-EC2
- [ ] Update deploy/ documentation and skills
- [ ] Tear down old single-EC2 deployment
