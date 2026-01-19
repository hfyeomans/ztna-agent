# Task State: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Status:** Not Started
**Branch:** `feature/006-cloud-deployment`
**Last Updated:** 2026-01-19

---

## Overview

Deploy Intermediate Server and App Connector to cloud infrastructure for NAT testing and production readiness. Enables testing Agent behavior behind real NAT environments.

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Not Started

### Prerequisites
- [ ] Task 004 complete (E2E Relay Testing - local validation)
- [ ] Cloud provider account (AWS/GCP/DigitalOcean/Vultr)
- [ ] Domain name (optional, for TLS certificates)

### What's Done
- Task planning documentation created

### What's Next
1. Complete Task 004 (local E2E testing)
2. Choose cloud provider
3. Create feature branch: `git checkout -b feature/006-cloud-deployment`
4. Set up cloud infrastructure
5. Deploy and configure components
6. Test NAT traversal

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 004 (E2E Testing) | 🔲 Ready | Local testing must pass first |
| Task 005 (P2P) | 🔲 Not Started | Optional - cloud helps test hole punching |
| Cloud Account | 🔲 Not Configured | Need provider credentials |

---

## Deployment Components

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Cloud VM (public IP) | 🔲 |
| App Connector | Cloud VM (same or separate) | 🔲 |
| TLS Certificates | Let's Encrypt or self-signed | 🔲 |
| Firewall Rules | UDP 4433 inbound | 🔲 |

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/006-cloud-deployment`
5. Continue with next unchecked item in `todo.md`
