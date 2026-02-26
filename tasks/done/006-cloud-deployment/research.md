# Research: Cloud Deployment

**Task ID:** 006-cloud-deployment

---

## Purpose

Document research findings, cloud provider evaluation, deployment strategies, and best practices for deploying ZTNA components to cloud infrastructure.

---

## Cloud Provider Comparison

### DigitalOcean

**Pros:**
- Simple, developer-friendly interface
- Fast provisioning (< 1 minute)
- Predictable pricing
- Good documentation
- Easy firewall management

**Cons:**
- Limited free tier
- Fewer regions than AWS/GCP
- Less enterprise features

**Pricing (as of 2026):**
- Basic Droplet: $4-6/month (1 vCPU, 512MB-1GB RAM)
- Static IP: Included
- Bandwidth: 500GB-1TB included

**QUIC/UDP Support:**
- Full UDP support
- No special configuration needed
- Firewall supports UDP rules

---

### AWS Lightsail

**Pros:**
- AWS ecosystem integration
- Predictable pricing (unlike EC2)
- Free tier for first month
- Static IP included

**Cons:**
- AWS account complexity
- Billing can be confusing
- Less flexible than EC2

**Pricing:**
- Basic: $3.50-5/month (1 vCPU, 512MB-1GB RAM)
- Static IP: Free if attached
- Bandwidth: 1TB included

**QUIC/UDP Support:**
- Full UDP support
- Security groups support UDP rules

---

### Vultr

**Pros:**
- Very cheap options
- Global data centers (32 locations)
- Fast SSD storage
- No contracts

**Cons:**
- Smaller community
- Documentation less comprehensive
- Occasional stock issues in regions

**Pricing:**
- Cloud Compute: $2.50-5/month (1 vCPU, 512MB-1GB RAM)
- Static IP: Free
- Bandwidth: 500GB-1TB included

**QUIC/UDP Support:**
- Full UDP support
- Firewall supports UDP rules

---

### GCP Compute Engine

**Pros:**
- $300 free credit (90 days)
- f1-micro always free tier
- Excellent networking
- Premium network tier available

**Cons:**
- Complex pricing
- Steep learning curve
- Free tier has limitations

**Pricing:**
- f1-micro: Free (0.2 vCPU, 0.6GB RAM) - limited
- e2-micro: ~$5/month (2 vCPU shared, 1GB RAM)
- Static IP: $0.01/hour when unattached

**QUIC/UDP Support:**
- Full UDP support
- VPC firewall supports UDP rules

---

## Recommendation

**For MVP Development:** DigitalOcean or Vultr
- Simple setup
- Low cost
- Good for testing

**For Production:** AWS Lightsail or GCP
- Better reliability
- More enterprise features
- Better for scaling

---

## Deployment Architecture Options

### Single VM (Recommended for MVP)

```
┌────────────────────────────────────────┐
│         Cloud VM (Public IP)           │
│                                         │
│  Intermediate Server (port 4433)       │
│           │                             │
│           ▼                             │
│  App Connector (localhost)             │
│           │                             │
│           ▼                             │
│  Test Service (localhost:8080)         │
└────────────────────────────────────────┘
```

**Pros:**
- Simple deployment
- No internal networking complexity
- Lowest cost
- Easier debugging

**Cons:**
- Single point of failure
- Not production-like

---

### Separate VMs (Production-like)

```
┌──────────────────┐    ┌──────────────────┐
│  Intermediate    │    │   Connector      │
│  VM (Public)     │◄──►│   VM (Private)   │
│                  │    │                  │
│  Port 4433       │    │  Test Service    │
└──────────────────┘    └──────────────────┘
```

**Pros:**
- More realistic
- Security isolation
- Scalability options

**Cons:**
- Higher cost
- More complex networking
- VPC configuration needed

---

## TLS Certificate Options

### Self-Signed (MVP)

```bash
# Generate self-signed certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout key.pem -out cert.pem \
  -days 365 -nodes \
  -subj "/CN=ztna-intermediate"
```

**Pros:**
- No domain required
- Quick setup
- Works for testing

**Cons:**
- Agent must skip verification or trust cert
- Not suitable for production
- Browser warnings if used for web

---

### Let's Encrypt (Production)

```bash
# Requires domain pointing to VM IP
certbot certonly --standalone -d intermediate.yourdomain.com
```

**Pros:**
- Free, trusted certificates
- Automatic renewal
- Production-ready

**Cons:**
- Requires domain name
- DNS setup required
- Renewal automation needed

---

## Firewall Configuration

### Required Rules

| Direction | Protocol | Port | Source | Purpose |
|-----------|----------|------|--------|---------|
| Inbound | UDP | 4433 | 0.0.0.0/0 | QUIC connections |
| Inbound | TCP | 22 | Admin IPs | SSH management |
| Outbound | All | All | 0.0.0.0/0 | Internet access |

### Example: DigitalOcean

```bash
doctl compute firewall create \
  --name ztna-firewall \
  --inbound-rules "protocol:udp,ports:4433,address:0.0.0.0/0" \
  --inbound-rules "protocol:tcp,ports:22,address:YOUR_IP/32" \
  --outbound-rules "protocol:tcp,ports:all,address:0.0.0.0/0" \
  --outbound-rules "protocol:udp,ports:all,address:0.0.0.0/0"
```

### Example: AWS Security Group

```bash
aws ec2 create-security-group \
  --group-name ztna-sg \
  --description "ZTNA Intermediate Server"

aws ec2 authorize-security-group-ingress \
  --group-name ztna-sg \
  --protocol udp \
  --port 4433 \
  --cidr 0.0.0.0/0

aws ec2 authorize-security-group-ingress \
  --group-name ztna-sg \
  --protocol tcp \
  --port 22 \
  --cidr YOUR_IP/32
```

---

## NAT Testing Scenarios

### Home NAT (Most Common)

- Router performs NAT
- Usually "Full Cone" or "Restricted Cone"
- Hole punching typically works

### Carrier-Grade NAT (CGNAT)

- ISP performs additional NAT layer
- Common on mobile networks
- May have strict port filtering
- Hole punching more difficult

### Enterprise NAT/Firewall

- Often "Symmetric NAT"
- May block UDP entirely
- Hole punching unlikely to work
- Relay essential

### Testing Tools

```bash
# Check NAT type (requires STUN server)
# Our QAD achieves similar result

# Check if UDP port is reachable
nc -u -v CLOUD_IP 4433

# Test latency
ping CLOUD_IP

# Packet capture
tcpdump -i any udp port 4433
```

---

## Performance Considerations

### Expected Latency

| Route | Estimated RTT |
|-------|---------------|
| Same region | 10-30ms |
| Cross-country | 50-100ms |
| Intercontinental | 150-300ms |

### Bandwidth

- QUIC DATAGRAM: MAX_DATAGRAM_SIZE = 1350 bytes
- Overhead: QUIC headers ~20-50 bytes per datagram
- Effective throughput depends on network conditions

### Optimization

- Choose region close to primary users
- Consider multiple relay servers for global deployment
- Monitor packet loss and jitter

---

## Monitoring & Observability

### Basic Monitoring

```bash
# Check service status
systemctl status intermediate-server
systemctl status app-connector

# View logs
journalctl -u intermediate-server -f
journalctl -u app-connector -f

# Network stats
ss -ulnp | grep 4433
netstat -an | grep 4433
```

### Advanced Monitoring (Future)

- Prometheus metrics endpoint
- Grafana dashboards
- AlertManager for notifications
- CloudWatch / Stackdriver integration

---

---

## Configuration Scalability (From Task 004 Oracle Review)

> **Note:** Recommendations from Phase 2 E2E testing Oracle review (2026-01-19)

When migrating to cloud services, plan to address these hard-coded values:

### Current Hard-Coded Values to Replace

| Value | Current Location | Migration Strategy |
|-------|------------------|-------------------|
| `test-service` | protocol-validation.sh | Environment variable `$SERVICE_ID` |
| `127.0.0.1:4433` | Multiple scripts | Config file or env var |
| DATAGRAM sizes | quic-client, test scripts | Query `dgram_max_writable_len()` |
| Cert paths | common.sh, testing-guide | Single canonical `$CERT_DIR` |

### Recommended Configuration Architecture

```
# Environment-based (development)
INTERMEDIATE_HOST=127.0.0.1
INTERMEDIATE_PORT=4433
SERVICE_ID=test-service
CERT_DIR=/path/to/certs

# Config file (production)
config.toml:
[relay]
host = "relay.example.com"
port = 4433

[service]
id = "production-service"
token = "..."  # From secrets manager

[tls]
cert = "/etc/ztna/cert.pem"
key = "/etc/ztna/key.pem"
```

### Production Scalability Requirements

1. **Service Discovery**
   - DNS-based relay endpoint resolution
   - Health check integration
   - Failover support

2. **Certificate Management**
   - Let's Encrypt with auto-renewal
   - Or cloud KMS (AWS ACM, GCP Certificate Manager)
   - Certificate rotation without downtime

3. **Multi-Tenant Service IDs**
   - Namespaced service IDs: `tenant-id/service-name`
   - Token-based authentication per tenant
   - Rate limiting per service ID

4. **Dynamic DATAGRAM Sizing**
   - Use `dgram_max_writable_len()` after handshake
   - Adapt to MTU/network conditions
   - Log effective limits for monitoring

---

## Critical Insight: NAT Testing Requirements

> **IMPORTANT:** Cloud VMs have **direct public IPs** - they are NOT behind NAT. Testing hole punching requires at least ONE peer to be behind real NAT.

### What This Means

1. **Deploying to cloud alone doesn't test NAT traversal**
   - Cloud Intermediate Server: Direct public IP (no NAT)
   - Cloud App Connector: Direct public IP (no NAT)
   - Both can communicate directly without hole punching

2. **To test real hole punching, the Agent must be behind NAT**
   - Home network (most common NAT - Full Cone or Restricted Cone)
   - Mobile hotspot (Carrier-Grade NAT - more restrictive)
   - Corporate network (Symmetric NAT - hardest)

3. **Cloud-only testing validates relay, not hole punching**
   - DATAGRAM relay: ✅ Testable (cloud-to-cloud)
   - QAD public IP discovery: ✅ Testable
   - P2P hole punching: ❌ Requires real NAT

### Minimum Viable Test Topology for Hole Punching

```
┌─────────────────────┐                    ┌─────────────────────────────┐
│   Home Network      │                    │        Cloud VM             │
│   (Behind NAT)      │                    │   (Direct Public IP)        │
│                     │                    │                             │
│  ┌───────────────┐  │                    │  ┌───────────────────────┐  │
│  │    Agent      │  │                    │  │  Intermediate Server  │  │
│  │   (macOS)     │──┼───► Home Router ──►│  │      + App Connector  │  │
│  │               │  │       NAT          │  └───────────────────────┘  │
│  └───────────────┘  │                    │                             │
└─────────────────────┘                    └─────────────────────────────┘

What's being tested:
- QAD discovers Agent's public IP (home NAT external address)
- Connector sees Agent behind NAT
- P2P candidates exchanged, hole punch attempted
- Fallback to relay if hole punch fails
```

---

## Same-Network Hole Punching Testing

### Can We Test on the Same LAN?

**Short Answer:** No meaningful NAT hole punching testing on the same LAN.

**Why:**
- Both peers on the same LAN see each other's private IPs directly
- No NAT translation involved
- [Hairpin NAT](https://bford.info/pub/net/p2pnat/) (reaching external IP from inside) requires specific router support (rare)
- Most home routers don't support hairpin NAT well

### Docker NAT Simulation

**Can Docker containers with separate subnets simulate NAT scenarios?**

**Yes, this works!** The [arXiv paper "Implementing NAT Hole Punching with QUIC"](https://arxiv.org/abs/2408.01791) describes:

```bash
# Example Docker NAT simulation setup
# Create two separate networks with NAT

# Network A: 192.168.0.0/24 (ClientA behind NAT-A)
# Network B: 192.168.1.0/24 (ClientB behind NAT-B)
# Each network has iptables MASQUERADE rule

# This simulates two different LANs behind different NATs
```

**Pros:**
- Validates signaling protocol and timing
- Tests address exchange flow
- Reproducible environment

**Cons:**
- Synthetic NAT behavior (iptables != real router)
- May not reflect real-world NAT quirks
- Port mapping behavior may differ

**Recommendation:** Use Docker simulation for protocol validation, but test with real NATs for production validation.

---

## Cloudflare Workers Limitations

### Is Cloudflare Workers Suitable for UDP Hole Punching?

**Short Answer:** No, Cloudflare Workers is not suitable for UDP/QUIC hole punching testing.

### Limitations

1. **UDP Support is Limited**
   - [Cloudflare Socket Workers](https://blog.cloudflare.com/introducing-socket-workers/) (UDP/TCP socket API) still in development
   - Current Workers are HTTP/WebSocket focused
   - No raw socket control for hole punching

2. **Dedicated IPv4 Required**
   - [UDP requires dedicated IPv4 addresses](https://fly.io/docs/networking/services/) ($2-5/month extra)
   - Shared IPs don't support UDP
   - Added cost and complexity

3. **Anycast Complications**
   - Cloudflare uses Anycast routing globally
   - UDP packets may route to different edge servers
   - Breaks stateful hole punching (need consistent endpoint)

4. **Proxy Architecture**
   - Workers sit behind Cloudflare's proxy layer
   - Can't control outbound source port (critical for hole punching)
   - No direct socket access

### Better Alternatives

| Option | UDP Support | Hole Punching | Recommendation |
|--------|-------------|---------------|----------------|
| **Cloudflare Workers** | Limited | ❌ Not suitable | Don't use |
| **Fly.io** | Requires dedicated IPv4 | ⚠️ Possible with config | Not recommended |
| **Traditional VPS** | Full | ✅ Works | **Use this** |

---

## AWS NAT Behavior Analysis

### Why AWS Can Be Problematic for Hole Punching

**Short Answer:** Not symmetric NAT - it's security groups + NAT Gateway complexity.

### AWS VPC NAT Behavior

[AWS VPC uses 1:1 NAT](http://www.somic.org/2009/11/02/punching-udp-holes-in-amazon-ec2/) that preserves source ports (similar to Full Cone NAT). This *should* work for hole punching.

**Real Issues:**

1. **Security Groups Block by Default**
   - Must explicitly allow UDP ingress from `0.0.0.0/0` (any source)
   - Default: deny all inbound
   - Easy to misconfigure

2. **NAT Gateway = Symmetric-like Behavior**
   - If instances are in *private* subnets using NAT Gateway
   - NAT Gateway may assign different ports for different destinations
   - This breaks hole punching
   - **Solution:** Use instances with direct public IPs (not NAT Gateway)

3. **Same-VPC Private IP Issues**
   - Instances in same VPC using private IPs doesn't work without security group rules
   - [Reported issue](http://www.somic.org/2009/11/02/punching-udp-holes-in-amazon-ec2/): same-region private IP communication fails without explicit rules

### AWS Configuration for Hole Punching

```bash
# CRITICAL: Security group must allow UDP from any source
aws ec2 authorize-security-group-ingress \
  --group-id sg-xxx \
  --protocol udp \
  --port 4433 \
  --cidr 0.0.0.0/0  # Must be 0.0.0.0/0, not specific IP

# Use instance with PUBLIC IP (not behind NAT Gateway)
# VPC public subnet, auto-assign public IP
```

### AWS Summary

| Configuration | Hole Punching Works? | Notes |
|---------------|----------------------|-------|
| EC2 with public IP, open security group | ✅ Yes | Recommended |
| EC2 behind NAT Gateway | ⚠️ Maybe not | Port mapping may vary |
| EC2 with restrictive security group | ❌ No | Must allow 0.0.0.0/0 UDP |
| Lightsail with open firewall | ✅ Yes | Simpler than EC2 |

---

## Detailed Cloud Provider Comparison for Hole Punching

### Provider Ranking for NAT Traversal Testing

| Rank | Provider | NAT Type | UDP Support | Ease of Setup | Cost | Notes |
|------|----------|----------|-------------|---------------|------|-------|
| 1 | **Vultr** ⭐ | Direct public IP | Full | Easy | $2.50-5/mo | Cheapest, 32 regions |
| 2 | **DigitalOcean** ⭐ | Direct public IP | Full | Very Easy | $4-6/mo | Best docs, simple firewall |
| 3 | **Hetzner** | Direct public IP | Full | Easy | €4-5/mo | Cheapest in EU |
| 4 | **Linode** | Direct public IP | Full | Easy | $5/mo | Akamai backbone |
| 5 | **GCP** | VPC-based | Full | Complex | Free-$5/mo | Free tier, complex config |
| 6 | **AWS Lightsail** | Direct public IP | Full | Medium | $3.50-5/mo | Simpler than EC2 |
| 7 | **AWS EC2** | Security groups | Full | Complex | Varies | Flexible but complex |
| 8 | **Fly.io** | Behind proxy | Limited | Medium | Varies | Needs dedicated IPv4 |

### Recommendation

**For Task 006 MVP:** **Vultr** or **DigitalOcean**

**Why:**
- Direct public IPs (no NAT on cloud side)
- Simple firewall configuration
- Cheap ($2.50-6/month)
- Fast provisioning (< 60 seconds)
- No complex VPC/security group configuration

---

## Fly.io Deep Dive

### Why Fly.io is Not Recommended for This Task

1. **UDP Requires Special Configuration**
   - Must listen on `fly-global-services` address
   - Requires dedicated IPv4 ($2/month extra)
   - [Anycast may not work properly with UDP](https://community.fly.io/t/anycast-not-applicable-with-udp/20526)

2. **Proxy Layer Complications**
   - Fly Proxy sits between internet and your app
   - May interfere with NAT traversal
   - Source port control uncertain

3. **When Fly.io Makes Sense**
   - Edge deployment (low latency to users)
   - HTTP/WebSocket workloads
   - NOT raw UDP applications

---

## Alternative Testing Approaches

### 1. Docker NAT Simulation (Local)

**Pros:** Free, reproducible, no cloud costs
**Cons:** Synthetic NAT behavior

```bash
# Setup two Docker networks with NAT
# Useful for validating protocol before cloud deployment
```

### 2. Tailscale/ZeroTier NAT Tester

**Pros:** Real NAT data from global users
**Cons:** Not automated

[Tailscale reports 90%+ success rate](https://tailscale.com/blog/nat-traversal-improvements-pt-1) for NAT traversal in typical conditions.

### 3. punch-check Tool

[GitHub: delthas/punch-check](https://github.com/delthas/punch-check) - A simple tool to check whether your router supports UDP hole-punching and additional NAT properties.

---

## References

- [DigitalOcean API](https://docs.digitalocean.com/reference/api/)
- [AWS Lightsail CLI](https://docs.aws.amazon.com/cli/latest/reference/lightsail/)
- [Vultr API](https://www.vultr.com/api/)
- [GCP Compute Engine](https://cloud.google.com/compute/docs)
- [Let's Encrypt Documentation](https://letsencrypt.org/docs/)
- [Terraform Cloud Providers](https://registry.terraform.io/browse/providers)

### NAT Traversal Research

- [UDP Hole Punching - Wikipedia](https://en.wikipedia.org/wiki/UDP_hole_punching)
- [Peer-to-Peer Communication Across NATs - Bryan Ford](https://bford.info/pub/net/p2pnat/)
- [Implementing NAT Hole Punching with QUIC - arXiv](https://arxiv.org/abs/2408.01791)
- [Tailscale NAT Traversal Improvements](https://tailscale.com/blog/nat-traversal-improvements-pt-1)
- [ZeroTier: The State of NAT Traversal](https://www.zerotier.com/blog/the-state-of-nat-traversal/)
- [Decentralized Hole Punching - Protocol Labs](https://research.protocol.ai/publications/decentralized-hole-punching/seemann2022.pdf)
- [AWS EC2 Hole Punching](http://www.somic.org/2009/11/02/punching-udp-holes-in-amazon-ec2/)
- [Cloudflare Socket Workers](https://blog.cloudflare.com/introducing-socket-workers/)
- [Fly.io UDP Documentation](https://fly.io/docs/networking/udp-and-tcp/)
