# State: Infrastructure Architecture

**Description:** Current state of the infrastructure architecture task.

**Purpose:** Track progress and context for session resumption.

---

## Current State: Not Started — Research Phase

**Last Updated:** 2026-02-27

### What's Done
- [x] Task created with 6 standard files
- [x] Research documented (current state, target architecture, decision points)
- [x] Plan drafted (5 phases, success criteria)
- [x] Oracle review requested (Docker vs bare EC2 for Intermediate Server)
- [x] Oracle feedback incorporated — Docker on EC2 with `--net=host` recommended
- [x] Shared _context/ files updated (README.md, components.md)

### Key Decisions Made
- Task 008 stays focused on software-level operations (reconnection, metrics, graceful shutdown)
- Task 016 handles infrastructure separation and admin panel
- Multi-tenancy is explicitly out of scope (future work)
- Pi k8s is for local/edge connector deployments, not the primary platform

### Key Decisions Pending
- Docker vs bare EC2 for Intermediate Server (awaiting Oracle)
- Admin Panel tech stack
- Test backend selection (QuakeJS, nginx, etc.)
- Task ordering: 008 first or 016 first?

### Context for Resume
- Created from user direction on 2026-02-27
- Intermediate Server is QAD + QUIC relay (NOT STUN/TURN)
- Current single-EC2 deployment documented in `deploy/aws/aws-deploy-skill.md`
- Existing k8s manifests in `deploy/k8s/`
- Existing Docker NAT sim in `deploy/docker-nat-sim/`
