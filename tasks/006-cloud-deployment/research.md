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

## References

- [DigitalOcean API](https://docs.digitalocean.com/reference/api/)
- [AWS Lightsail CLI](https://docs.aws.amazon.com/cli/latest/reference/lightsail/)
- [Vultr API](https://www.vultr.com/api/)
- [GCP Compute Engine](https://cloud.google.com/compute/docs)
- [Let's Encrypt Documentation](https://letsencrypt.org/docs/)
- [Terraform Cloud Providers](https://registry.terraform.io/browse/providers)
