# State: Multi-Environment Testing

**Task ID:** 012-multi-environment-testing
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track the current state of multi-environment testing.

---

## Current State

Not started. MVP testing limited to home NAT (EIF NAT) + AWS us-east-2.

### What Exists (from MVP)
- AWS EC2 deployment (us-east-2, Elastic IP 3.128.36.92)
- Pi k8s deployment (home LAN, 10.0.150.205)
- Docker NAT simulation (localhost)
- P2P tested from home router NAT (EIF/port-restricted cone NAT)
- 10-minute stability test (600/600, 0% loss)
- Performance: P2P 32.6ms, Relay 76ms

### NAT Types Tested
| Environment | NAT Type | P2P Result |
|-------------|----------|------------|
| Home router (EIF) | Port-restricted cone | P2P success |
| Docker NAT sim | Full cone (simulated) | P2P success |
| Mobile hotspot | Not yet tested | — |
| Coffee shop WiFi | Not yet tested | — |
| Corporate VPN | Not yet tested | — |

### What This Task Delivers
- DigitalOcean deployment with performance comparison
- NAT diversity testing (3+ NAT types)
- 1-hour+ stability tests
- Throughput benchmarks (iperf3)
- Multi-region deployment and testing

---

## Decisions Log

(No decisions yet — task not started)
