# Docker NAT Simulation - Troubleshooting Guide

## Build Issues

### Issue: "Cannot connect to Docker daemon"
**Symptom:**
```
Cannot connect to the Docker daemon at unix:///Users/hank/.docker/run/docker.sock.
Is the docker daemon running?
```

**Resolution:**
```bash
# Start Docker Desktop
open /Applications/Docker.app

# Wait 10-20 seconds for daemon to start
sleep 20

# Verify Docker is running
docker info
```

---

### Issue: "COPY failed: stat target/release/[binary]: no such file"
**Symptom:**
```
COPY failed: stat /build/intermediate-server/target/release/intermediate-server: file does not exist
```

**Cause:** Binary name mismatch between Cargo.toml and Dockerfile

**Resolution:**
```bash
# Check Cargo.toml for correct binary name
grep -A 2 '\[\[bin\]\]' intermediate-server/Cargo.toml

# Verify Dockerfile COPY path matches
grep 'COPY --from=builder.*target/release' deploy/docker-nat-sim/Dockerfile.intermediate
```

**Verified binaries (2026-01-25):**
- intermediate-server → `intermediate-server`
- app-connector → `app-connector`
- quic-client → `quic-test-client`
- echo-server → `udp-echo`

---

### Issue: Cargo dependency resolution hangs
**Symptom:**
```
Updating crates.io index
(hangs for several minutes)
```

**Cause:** Network issues, slow crates.io mirror, or large dependency tree

**Resolution:**
```bash
# Build with verbose output to see progress
docker compose build --progress=plain 2>&1 | tee build.log

# If stuck, cancel (Ctrl+C) and retry
docker compose build --no-cache
```

---

### Issue: OpenSSL linkage errors
**Symptom:**
```
error: linking with `cc` failed
= note: ld: library not found for -lssl
```

**Cause:** Missing libssl-dev in build stage

**Resolution:**
Verify Dockerfile includes:
```dockerfile
RUN apt-get install -y pkg-config libssl-dev
```

All current Dockerfiles include this. If error persists, rebuild from scratch:
```bash
docker compose build --no-cache --pull
```

---

## Runtime Issues

### Issue: Container immediately exits
**Symptom:**
```bash
docker compose ps
# Shows container status as "Exited (1)"
```

**Diagnosis:**
```bash
# Check exit logs
docker logs ztna-intermediate
docker logs ztna-app-connector

# Check for common errors:
# - Certificate file not found
# - Port already in use
# - Invalid command-line arguments
```

**Resolution:**
```bash
# Verify certificates are mounted
docker exec ztna-intermediate ls -la /etc/ztna/certs/

# Check port conflicts
lsof -i :4433

# Restart with fresh state
docker compose down
docker compose up -d
```

---

### Issue: "bind: address already in use"
**Symptom:**
```
Error starting userland proxy: listen udp4 0.0.0.0:4433: bind: address already in use
```

**Diagnosis:**
```bash
# Find process using port 4433
lsof -i :4433

# Example output:
# COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME
# quiche  12345 user    7u  IPv4 0x1234      0t0  UDP *:4433
```

**Resolution:**
```bash
# Option 1: Stop the conflicting process
kill -9 12345

# Option 2: Change port in docker-compose.yml
# Edit: ports: - "4434:4433/udp"

# Option 3: Use different port in test client
docker compose run --rm quic-client --server 172.20.0.10:4434 ...
```

---

### Issue: Certificate validation errors
**Symptom:**
```
Error: certificate verify failed
TLS handshake error: unable to get local issuer certificate
```

**Diagnosis:**
```bash
# Check certificates exist
ls -lh /Users/hank/dev/src/agent-driver/ztna-agent/certs/

# Verify certificate validity
openssl x509 -in certs/cert.pem -text -noout | grep -A 2 Validity

# Check if cert/key match
openssl x509 -in certs/cert.pem -noout -modulus | openssl md5
openssl rsa -in certs/key.pem -noout -modulus | openssl md5
# MD5 hashes should match
```

**Resolution:**
```bash
# Option 1: Regenerate certificates (if expired or mismatched)
cd certs/
./generate-certs.sh  # (if script exists)

# Option 2: Disable cert verification for testing only
# Add to client: --insecure (NOT RECOMMENDED for production)

# Option 3: Check file permissions
chmod 644 certs/cert.pem
chmod 644 certs/connector-cert.pem
chmod 600 certs/key.pem
chmod 600 certs/connector-key.pem
```

---

## Networking Issues

### Issue: NAT rules not applied
**Symptom:**
```bash
docker exec ztna-nat-agent iptables -t nat -L -n -v
# Output: Empty POSTROUTING chain
```

**Diagnosis:**
```bash
# Check if NET_ADMIN capability is granted
docker inspect ztna-nat-agent | grep -A 5 CapAdd

# Check if ip_forward is enabled
docker exec ztna-nat-agent sysctl net.ipv4.ip_forward
# Should return: net.ipv4.ip_forward = 1
```

**Resolution:**
```bash
# Restart NAT gateway with proper capabilities
docker compose restart nat-agent

# Verify rules are applied
docker exec ztna-nat-agent iptables -t nat -L POSTROUTING -n -v | grep MASQUERADE

# Expected output:
# Chain POSTROUTING (policy ACCEPT)
# target     prot opt source               destination
# MASQUERADE  all  --  172.21.0.0/24        0.0.0.0/0
```

---

### Issue: Echo server not responding
**Symptom:**
```
Error: timeout waiting for echo response
```

**Diagnosis:**
```bash
# Step 1: Verify echo server is running
docker logs ztna-echo-server

# Step 2: Test direct connectivity (bypass NAT)
docker exec ztna-app-connector nc -u -w 2 172.22.0.20 9999 <<< "test"

# Step 3: Check app-connector forwarding
docker logs ztna-app-connector 2>&1 | grep -i "forward\|relay"

# Step 4: Check NAT conntrack on connector side
docker exec ztna-nat-connector cat /proc/net/nf_conntrack | grep 9999
```

**Resolution:**
```bash
# If echo server not running
docker compose restart echo-server

# If forwarding not working, check connector config
docker exec ztna-app-connector env | grep ZTNA_FORWARD
# Should show: ZTNA_FORWARD_HOST=172.22.0.20, ZTNA_FORWARD_PORT=9999

# If still failing, capture traffic
docker exec ztna-nat-connector tcpdump -i eth1 -n udp port 9999 -c 20
```

---

### Issue: Intermediate server not receiving connections
**Symptom:**
```
Error: connection timeout to intermediate server
```

**Diagnosis:**
```bash
# Check intermediate server logs
docker logs ztna-intermediate 2>&1 | grep -i "listening\|error"

# Verify port is exposed
docker port ztna-intermediate
# Should show: 4433/udp -> 0.0.0.0:4433

# Test connectivity from agent network
docker exec ztna-debug-agent ping -c 3 172.20.0.10
```

**Resolution:**
```bash
# Restart intermediate server
docker compose restart intermediate-server

# Wait for startup
sleep 5

# Verify listening
docker logs ztna-intermediate 2>&1 | tail -20

# If not listening, check certificate paths
docker exec ztna-intermediate ls -la /etc/ztna/certs/
```

---

## NAT Traversal Issues

### Issue: Observed address shows private IP
**Symptom:**
```bash
docker logs ztna-quic-client 2>&1 | grep "observed"
# Shows: Observed address: 172.21.0.10:XXXXX (should be 172.20.0.2:XXXXX)
```

**Cause:** NAT not working, traffic not going through NAT gateway

**Diagnosis:**
```bash
# Check routing in quic-client container
docker exec ztna-quic-client ip route
# Default route should point to nat-agent (172.21.0.1)

# Check if MASQUERADE rule is working
docker exec ztna-nat-agent iptables -t nat -L POSTROUTING -n -v
# pkts and bytes columns should show non-zero values after traffic
```

**Resolution:**
```bash
# Restart NAT gateway
docker compose restart nat-agent

# Verify NAT is working with trace
docker exec ztna-nat-agent tcpdump -i eth0 -n udp port 4433 -c 10 &
docker compose run --rm quic-client --server 172.20.0.10:4433 --service test-service --wait 2000
# Check tcpdump shows source IP as 172.20.0.2, not 172.21.0.10
```

---

### Issue: Conntrack table empty
**Symptom:**
```bash
docker exec ztna-nat-agent cat /proc/net/nf_conntrack | grep udp | grep 4433
# No output (should show active connections)
```

**Cause:** No traffic passing through NAT, or iptables modules not loaded

**Resolution:**
```bash
# Verify iptables modules
docker exec ztna-nat-agent lsmod | grep nf_conntrack
docker exec ztna-nat-agent lsmod | grep nf_nat

# Rebuild NAT gateway from Alpine base
docker compose build --no-cache nat-agent
docker compose up -d nat-agent

# Test again
docker compose run --rm quic-client --server 172.20.0.10:4433 --service test-service --wait 2000
```

---

## Performance Issues

### Issue: High RTT (> 100ms)
**Symptom:**
```
Average RTT: 250ms
```

**Expected:** Docker local networking should have RTT < 10ms

**Diagnosis:**
```bash
# Check system load
docker stats --no-stream

# Check for CPU throttling
docker inspect ztna-intermediate | grep -A 5 NanoCpus

# Measure baseline network latency
docker exec ztna-debug-agent ping -c 10 172.20.0.10
```

**Resolution:**
```bash
# Increase Docker resources in Docker Desktop settings
# Preferences → Resources → CPUs (4+) and Memory (4GB+)

# Restart Docker Desktop
# Then rebuild and retest
```

---

### Issue: Packet loss
**Symptom:**
```
Packet loss: 15% (expected: 0%)
```

**Diagnosis:**
```bash
# Check for UDP buffer overruns
docker exec ztna-intermediate netstat -su | grep -i "overflow\|error"

# Monitor UDP drops in real-time
docker exec ztna-nat-agent tcpdump -i eth0 -n udp -c 100 -w /tmp/capture.pcap
# Download: docker cp ztna-nat-agent:/tmp/capture.pcap .
# Analyze with Wireshark
```

**Resolution:**
```bash
# Increase UDP buffer sizes (add to docker-compose.yml)
# sysctls:
#   - net.core.rmem_max=26214400
#   - net.core.wmem_max=26214400

# Or reduce test burst rate
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --burst 10 \
    --burst-delay 100  # Add delay between packets
```

---

## Debug Commands

### Capture QUIC traffic
```bash
# On public network
docker exec ztna-debug-public tcpdump -i eth0 -n port 4433 -w /tmp/quic.pcap

# Download and analyze
docker cp ztna-debug-public:/tmp/quic.pcap .
wireshark quic.pcap
```

### Trace route through NAT
```bash
# From agent network to intermediate server
docker exec ztna-debug-agent traceroute -U -p 4433 172.20.0.10

# Expected hops:
# 1. 172.21.0.1 (nat-agent)
# 2. 172.20.0.10 (intermediate-server)
```

### Monitor NAT conntrack in real-time
```bash
# Watch conntrack table update
docker exec ztna-nat-agent watch -n 1 'cat /proc/net/nf_conntrack | grep udp | grep 4433'
```

### Check QUIC connection state
```bash
# Enable debug logging
docker compose stop intermediate-server
docker compose up -d intermediate-server -e RUST_LOG=debug,quinn=trace

# Watch detailed QUIC logs
docker logs -f ztna-intermediate 2>&1 | grep -i "handshake\|stream\|connection"
```

### Interactive debugging
```bash
# Launch shell in running container
docker exec -it ztna-app-connector /bin/bash

# Or start debug container
docker compose --profile debug up -d debug-agent
docker exec -it ztna-debug-agent bash

# Tools available: tcpdump, nmap, curl, netcat, dig, traceroute, etc.
```

---

## Common Error Messages

| Error | Meaning | Resolution |
|-------|---------|------------|
| `connection refused` | Service not listening or firewall blocking | Check service logs, verify port is exposed |
| `no route to host` | Network routing issue | Verify container networks, check NAT rules |
| `certificate verify failed` | TLS certificate invalid | Regenerate certs or disable verification |
| `address already in use` | Port conflict | Kill conflicting process or change port |
| `operation not permitted` | Missing capabilities | Add NET_ADMIN, NET_RAW to container |
| `timeout` | Network unreachable or slow response | Check NAT rules, increase --wait parameter |

---

## Getting Help

1. **Check logs first:**
   ```bash
   docker compose logs -f
   ```

2. **Enable verbose logging:**
   ```bash
   docker compose up -d -e RUST_LOG=debug
   ```

3. **Use debug containers:**
   ```bash
   docker compose --profile debug up -d
   docker exec -it ztna-debug-agent bash
   ```

4. **Capture traffic:**
   ```bash
   docker exec ztna-nat-agent tcpdump -i eth0 -n -w /tmp/capture.pcap
   ```

5. **Check documentation:**
   - README.md - Environment overview
   - TEST_REPORT.md - Detailed test procedures
   - VERIFICATION_SUMMARY.md - Quick status check

---

**Last updated:** 2026-01-25
