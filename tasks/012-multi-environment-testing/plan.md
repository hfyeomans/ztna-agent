# Plan: Multi-Environment Testing

**Task ID:** 012-multi-environment-testing
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Plan deployment and testing across diverse network environments to validate ZTNA robustness beyond home NAT + AWS.

---

## Phases (To Be Defined)

### Phase 1: DigitalOcean Deployment
- Provision DO droplet with ZTNA stack
- Deploy Intermediate + Connector + test services
- Compare latency and performance with AWS

### Phase 2: NAT Diversity Testing
- Classify NAT types with pystun3
- Test from mobile hotspot (CGNAT)
- Test from coffee shop WiFi
- Document P2P success rate per NAT type

### Phase 3: Extended Stability
- 1-hour P2P sustained test
- 24-hour Connector uptime
- Memory leak profiling (valgrind/heaptrack)
- Sleep/wake recovery cycles

### Phase 4: Throughput Benchmarks
- iperf3 through tunnel
- Compare with WireGuard baseline
- Document throughput ceiling and bottleneck

### Phase 5: Multi-Region
- Deploy to second cloud region
- Test cross-region relay latency
- Geographic failover

---

## Success Criteria

- [ ] ZTNA deployed and tested on DigitalOcean
- [ ] P2P success rate documented for 3+ NAT types
- [ ] 1-hour stability: 0% loss
- [ ] Throughput benchmarks published
- [ ] Multi-region relay tested
