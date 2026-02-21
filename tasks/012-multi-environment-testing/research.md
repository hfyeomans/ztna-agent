# Research: Multi-Environment Testing

**Task ID:** 012-multi-environment-testing
**Status:** Not Started
**Priority:** P3
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Purpose

Research deployment and testing across diverse environments — DigitalOcean, multi-region, symmetric NAT, mobile hotspot, and extended stability testing.

---

## Research Areas

### DigitalOcean Deployment
- Droplet provisioning (s-1vcpu-1gb, nyc1)
- Firewall rules (doctl compute firewall)
- Compare latency: DO NYC vs AWS us-east-2
- Cost comparison for always-on deployment

### Symmetric NAT / CGNAT
- Identify symmetric NAT test environments
- Mobile hotspot (T-Mobile, Verizon) — typically CGNAT
- Corporate VPN with symmetric NAT
- Impact on P2P hole punching success rate
- Relay-only graceful degradation verification

### Mobile Hotspot Testing
- iPhone personal hotspot (CGNAT)
- Android tethering
- WiFi calling network interactions
- Battery impact of QUIC keepalive on cellular

### Extended Stability Testing
- 1-hour sustained P2P ping test
- 24-hour Connector uptime test
- Memory leak detection over extended runs
- Connection recovery after sleep/wake cycles

### Throughput Benchmarks
- iperf3 through QUIC DATAGRAM tunnel
- P2P vs relay throughput comparison
- Impact of packet size on throughput
- Comparison with WireGuard/OpenVPN

### Multi-Region Deployment
- AWS us-east-2 + DO NYC (or AWS us-west-2)
- Cross-region relay latency
- Geographic failover testing
- Agent connecting to nearest Intermediate

---

## References

- Current deployment: AWS us-east-2 (single region) + Pi k8s (home LAN)
- MVP stability: 10-min test (600/600, 0% loss)
- MVP latency: P2P 32.6ms, Relay 76ms
- Phase 2 (DO) was deferred in Task 006 todo.md
- pystun3 for NAT classification
