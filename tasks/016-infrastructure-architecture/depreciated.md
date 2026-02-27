# Task 016: Infrastructure Architecture — Deprecated/Legacy Code

**Description:** Code removed or replaced during this task.

**Purpose:** Record what was removed and why, per project convention.

---

## Planned Deprecations

### Single-EC2 Deployment (Task 006 Legacy)

**Status:** To be replaced

**What:** All ZTNA components (intermediate-server, app-connector, echo-server) running as systemd services on a single EC2 instance (3.128.36.92, t3.micro).

**Why:** Single point of failure, no isolation between public-facing relay and internal connector, can't scale independently. Appropriate for MVP/demo but not for production.

**Replacement:** Distributed architecture with components on separate infrastructure (ECS/Fargate or dedicated EC2 instances).

**Files affected:**
- `deploy/aws/aws-deploy-skill.md` — will need major rewrite
- systemd service files on EC2 — will be replaced by ECS task definitions or new systemd configs
