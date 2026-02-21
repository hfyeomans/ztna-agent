# TODO: Multi-Environment Testing

**Task ID:** 012-multi-environment-testing
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Track implementation and testing tasks for multi-environment validation.

---

## Phase 1: DigitalOcean Deployment

- [ ] Configure doctl CLI (`doctl auth init`)
- [ ] Create Droplet (Ubuntu 24.04, s-1vcpu-1gb, nyc1)
- [ ] Configure firewall (UDP 4433, 4434, TCP 22)
- [ ] Deploy Intermediate + Connector + echo-server
- [ ] Test from macOS Agent
- [ ] Compare latency with AWS deployment
- [ ] Document results

## Phase 2: NAT Diversity Testing

- [ ] Run pystun3 from home network (document NAT type)
- [ ] Test from iPhone personal hotspot (CGNAT)
- [ ] Test from coffee shop WiFi
- [ ] Test from Android tethering (if available)
- [ ] Document P2P success/failure per NAT type
- [ ] Verify relay fallback works for all NAT types

## Phase 3: Extended Stability

- [ ] 1-hour P2P sustained ping (3600 packets, 0% loss target)
- [ ] 24-hour Connector uptime test
- [ ] Memory profiling (valgrind/heaptrack on Intermediate + Connector)
- [ ] macOS Agent sleep/wake recovery cycles (10 cycles)
- [ ] Document any memory growth or connection degradation

## Phase 4: Throughput Benchmarks

- [ ] Set up iperf3 through ZTNA tunnel
- [ ] Measure P2P throughput (Mbps)
- [ ] Measure relay throughput (Mbps)
- [ ] Compare with WireGuard baseline
- [ ] Test with different packet sizes (64, 512, 1024, MTU)
- [ ] Document throughput ceiling and bottleneck analysis

## Phase 5: Multi-Region

- [ ] Deploy to second region (DO NYC or AWS us-west-2)
- [ ] Test cross-region relay latency
- [ ] Test Agent connecting to nearest Intermediate
- [ ] Geographic failover: primary down → secondary takeover
