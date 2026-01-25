# Docker NAT Simulation Environment

This directory contains a Docker-based NAT simulation environment for testing P2P hole punching in the ZTNA agent project.

## Overview

The simulation creates three isolated Docker networks that mimic real-world NAT scenarios:

```
+-----------------------------------------------------------------------+
|                          Docker Host                                   |
+-----------------------------------------------------------------------+
|                                                                        |
|  ztna-public (172.20.0.0/24) - "Internet" (no NAT)                    |
|  +-- intermediate-server (172.20.0.10:4433)                           |
|  +-- nat-agent (172.20.0.2) - Agent's public interface                |
|  +-- nat-connector (172.20.0.3) - Connector's public interface        |
|                                                                        |
|  ztna-agent-lan (172.21.0.0/24) - Agent's private network             |
|  +-- quic-client (172.21.0.10) - Agent simulator, behind NAT          |
|  +-- nat-agent (172.21.0.1) - NAT gateway to public                   |
|                                                                        |
|  ztna-connector-lan (172.22.0.0/24) - Connector's private network     |
|  +-- app-connector (172.22.0.10) - App Connector, behind NAT          |
|  +-- echo-server (172.22.0.20:9999) - Local service to forward to     |
|  +-- nat-connector (172.22.0.1) - NAT gateway to public               |
|                                                                        |
+-----------------------------------------------------------------------+
```

## Quick Start

### Prerequisites

- Docker and Docker Compose installed
- TLS certificates in `../../certs/` (cert.pem, key.pem, connector-cert.pem, connector-key.pem)

### Run the Full Test Suite

```bash
# Build and run all tests
./test-nat-simulation.sh --build

# Run tests (using cached images)
./test-nat-simulation.sh

# Run with verbose output
./test-nat-simulation.sh --verbose

# Clean up and rebuild everything
./test-nat-simulation.sh --clean --build
```

### Manual Testing

```bash
# Start core services
docker compose up -d

# Run a one-off test client
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --send-udp "Hello from NAT!" \
    --dst 172.22.0.20:9999 \
    --wait 5000

# View logs
docker compose logs -f

# Stop everything
docker compose down
```

## Components

### Intermediate Server (172.20.0.10:4433)

The QUIC rendezvous server on the "public internet". Both the Agent (quic-client) and App Connector connect to this server to:

1. Discover their public IP:port (NAT-mapped address)
2. Exchange peer information for direct P2P connections
3. Relay traffic when direct connection fails

### App Connector (172.22.0.10)

Runs behind NAT on the Connector LAN. It:

1. Connects to the Intermediate Server through nat-connector
2. Registers as a Connector for "test-service"
3. Listens for incoming P2P connections from Agents
4. Forwards received traffic to the echo-server (172.22.0.20:9999)

### QUIC Test Client (172.21.0.10)

Simulates an Agent behind NAT on the Agent LAN. It:

1. Connects to the Intermediate Server through nat-agent
2. Registers as an Agent targeting "test-service"
3. Attempts P2P hole punching to reach the App Connector
4. Sends test traffic and measures RTT

### NAT Gateways (nat-agent, nat-connector)

Alpine containers running iptables NAT rules. They simulate home router NAT behavior using MASQUERADE.

### Echo Server (172.22.0.20:9999)

Simple UDP echo server that the App Connector forwards traffic to. Used to verify end-to-end connectivity.

## NAT Types

The simulation supports different NAT behaviors via `setup-nat.sh`:

| NAT Type | Description | P2P Difficulty |
|----------|-------------|----------------|
| `full-cone` | Endpoint-Independent Mapping | Easy |
| `restricted` | Address-Restricted Cone | Medium |
| `port-restrict` | Port-Restricted Cone (default) | Medium |
| `symmetric` | Different mapping per destination | Hard (requires TURN) |

To change NAT type:

```bash
# Example: Configure symmetric NAT on the agent gateway
docker exec -it ztna-nat-agent /bin/sh
# Inside container:
./setup-nat.sh symmetric
```

## Debug Mode

Start debug containers for manual network inspection:

```bash
./test-nat-simulation.sh --debug

# Connect to debug containers
docker exec -it ztna-debug-agent bash    # On Agent LAN
docker exec -it ztna-debug-connector bash # On Connector LAN
docker exec -it ztna-debug-public bash    # On Public network
```

Debug containers include `netshoot` tools: tcpdump, nmap, curl, netcat, etc.

### Useful Debug Commands

```bash
# Capture QUIC traffic on public network
docker exec ztna-debug-public tcpdump -i eth0 -n port 4433

# Check NAT conntrack entries
docker exec ztna-nat-agent cat /proc/net/nf_conntrack | grep udp

# Test UDP connectivity through NAT
docker exec ztna-debug-agent nc -u 172.20.0.10 4433

# Trace route through NAT
docker exec ztna-debug-agent traceroute 172.20.0.10
```

## Test Scenarios

### 1. Basic Relay Test

Verify traffic flows through the Intermediate Server relay:

```bash
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --send-udp "relay test" \
    --dst 172.22.0.20:9999 \
    --wait 5000
```

### 2. RTT Measurement

Measure round-trip latency through the system:

```bash
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --measure-rtt \
    --rtt-count 100 \
    --payload-size 64 \
    --dst 172.22.0.20:9999
```

### 3. Burst Load Test

Send rapid bursts of packets:

```bash
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --burst 50 \
    --payload-size 100 \
    --dst 172.22.0.20:9999
```

### 4. Echo Verification

Verify data integrity through the tunnel:

```bash
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --payload-size 256 \
    --payload-pattern random \
    --repeat 10 \
    --verify-echo \
    --dst 172.22.0.20:9999
```

## Troubleshooting

### Containers Won't Start

```bash
# Check for port conflicts
lsof -i :4433

# View build errors
docker compose build --no-cache 2>&1 | less

# Check container status
docker compose ps -a
```

### No Response from Echo Server

1. Verify echo server is running:
   ```bash
   docker logs ztna-echo-server
   ```

2. Test direct connectivity (bypassing NAT):
   ```bash
   docker exec ztna-app-connector nc -u 172.22.0.20 9999
   ```

3. Check app-connector forwarding:
   ```bash
   docker logs ztna-app-connector 2>&1 | grep -i forward
   ```

### NAT Traversal Failing

1. Check NAT rules are applied:
   ```bash
   docker exec ztna-nat-agent iptables -t nat -L -n -v
   ```

2. Monitor conntrack:
   ```bash
   docker exec ztna-nat-agent cat /proc/net/nf_conntrack
   ```

3. Capture traffic at NAT gateway:
   ```bash
   docker exec ztna-nat-agent tcpdump -i eth0 -n udp
   ```

### Connection Timeouts

- Increase `--wait` parameter for slow environments
- Check intermediate server logs: `docker logs ztna-intermediate`
- Verify all containers are on correct networks: `docker network inspect ztna-public`

## File Structure

```
deploy/docker-nat-sim/
+-- Dockerfile.intermediate    # Intermediate Server image
+-- Dockerfile.connector       # App Connector image
+-- Dockerfile.quic-client     # QUIC Test Client image
+-- Dockerfile.echo-server     # UDP Echo Server image
+-- docker-compose.yml         # Orchestration with NAT networks
+-- setup-nat.sh              # Configurable NAT rules script
+-- test-nat-simulation.sh    # Automated test runner
+-- README.md                 # This file
```

## Integration with CI/CD

The test script returns exit codes suitable for CI integration:

```yaml
# Example GitHub Actions job
nat-simulation-test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Run NAT simulation tests
      run: |
        cd deploy/docker-nat-sim
        ./test-nat-simulation.sh --build
```

## Related Documentation

- [ZTNA Architecture](../../docs/)
- [E2E Testing Guide](../../tests/e2e/README.md)
- [Intermediate Server](../../intermediate-server/)
- [App Connector](../../app-connector/)
