# Research: Security Hardening

**Task ID:** 007-security-hardening
**Status:** Not Started
**Priority:** P1
**Depends On:** None (006 MVP complete)
**Branch:** (not yet created)
**Last Updated:** 2026-02-21

---

## Security Review Findings (2026-02-21)

Full security review performed on `feature/006-cloud-deployment` branch (15,900 lines of changes across 93 files). Findings below are organized by severity.

### Source

- Review scope: `git diff origin/master...HEAD` (code files only, not docs/tasks)
- Components reviewed: Rust (packet_processor, intermediate-server, app-connector), Swift (PacketTunnelProvider), Shell (deployment scripts), Docker/K8s configs, GitHub Actions

---

## Critical Findings

### C1. TLS Certificate Verification Disabled on ALL QUIC Connections

**Files:**
- `core/packet_processor/src/lib.rs:195`
- `app-connector/src/main.rs:415,440`
- `intermediate-server/src/main.rs:244`

**Description:** Every QUIC connection uses `config.verify_peer(false)` — Agent-to-Intermediate, Connector-to-Intermediate, and P2P. This completely disables TLS certificate verification, making the entire ZTNA system vulnerable to MITM attacks. An attacker on the network path can impersonate the Intermediate Server and read/modify all tunneled IP packets.

**Already tracked in:** `_context/README.md` Priority 1 table — "TLS Certificate Verification"

**Fix approach:**
- Enable `verify_peer(true)` on Agent and Connector (client side)
- Load server CA cert with `config.load_verify_locations_from_file()`
- Consider mTLS where clients present certificates too
- Let's Encrypt for production certs (DNS-01 challenge for UDP)

---

## High Findings

### H1. Unbounded Received Datagram Queue (OOM DoS)

**File:** `core/packet_processor/src/lib.rs:582`

**Description:** `self.received_datagrams` (`VecDeque<Vec<u8>>`) grows without limit. If Swift drains slowly or a malicious Connector floods datagrams, memory exhaustion crashes the Network Extension (kills VPN tunnel).

**Not previously tracked.**

**Fix:** Cap queue depth (e.g., `const MAX_RECV_QUEUE: usize = 1024`), drop excess datagrams with log warning.

### H2. No Authentication or Authorization on Service Registration

**Files:**
- `intermediate-server/src/main.rs:530-570` (`handle_registration`)
- `intermediate-server/src/registry.rs:44-78`

**Description:** Any QUIC client can register as Agent or Connector for any service ID by sending `0x10`/`0x11` datagram. No auth, no authorization. A rogue client can:
- Register as Connector for a service → hijack traffic
- New Connector registration silently replaces old one (`registry.rs:57`)

**Already tracked in:** `_context/README.md` Priority 1 table — "Client Authentication"

**Fix approach:** Signed registration tokens, mTLS with service ID in cert, or pre-shared key per service. At minimum, log warning on Connector replacement.

### H3. TCP Proxy Forwards Without Destination Validation + Blocking Connect

**File:** `app-connector/src/main.rs:1108-1200`

**Description:** Two issues:
1. TCP proxy connects to `self.forward_addr` for every SYN regardless of destination IP in tunneled packet. Combined with H2, creates SSRF-like risk.
2. Uses blocking `StdTcpStream::connect_timeout(500ms)` on the single-threaded mio event loop. SYN floods to unreachable backends stall all QUIC processing.

**Partially tracked:** Blocking connect noted in code as MVP limitation. Destination validation not tracked.

**Fix:**
- Validate `dst_ip` matches expected virtual service IP before proxying
- Migrate to non-blocking `mio::net::TcpStream::connect()`
- Add rate limiting on new TCP session creation per source

### H4. Placeholder TLS Secrets Committed to Repository

**File:** `deploy/k8s/base/secrets.yaml:1-36`

**Description:** Base64-encoded placeholder TLS certs/keys in the repository. Included in `kustomization.yaml` as base resource. If deployed without replacement, all traffic uses public key material.

**Already tracked in:** `placeholder.md` — "Self-signed development certificates"

**Fix:** Remove from base `kustomization.yaml` resources. Add pre-deploy validation script. Consider cert-manager or sealed-secrets.

---

## Medium Findings

### M1. Hardcoded AWS Public IP in Source Code

**Files:**
- `deploy/config/agent.json:3`
- `deploy/config/connector.json:3`
- `deploy/config/intermediate.json:4`
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift:47-48`
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift:99`

**Description:** AWS Elastic IP `3.128.36.92` hardcoded as default throughout codebase. Infrastructure reconnaissance data committed to repo.

**Fix:** Use `0.0.0.0` placeholder that forces explicit configuration. Move real IPs to `.env` files in `.gitignore`.

### M2. P2P Protocol Demux Based on First Byte is Fragile

**File:** `app-connector/src/main.rs:119-121`

**Description:** `is_p2p_control_packet` checks `(data[0] & 0xC0) == 0` — any packet with top 2 bits clear is treated as P2P control. QUIC short headers or random UDP could be misrouted. Malformed input passed to `bincode::deserialize`.

**Fix:** Add magic byte prefix to P2P control messages (e.g., `[0xZT, type]`) that can't collide with QUIC headers.

### M3. Service-Routed Datagram Has No Sender Authorization

**File:** `intermediate-server/src/main.rs:577-644`

**Description:** `relay_service_datagram` (0x2F handler) allows any connected client to route to any Connector by service ID. No check that sending Agent is registered for that service.

**Fix:** Verify source connection is registered Agent for the specified service before relaying.

### M4. Docker NAT Containers Have Excessive Capabilities

**File:** `deploy/docker-nat-sim/docker-compose.yml:83-85,130-132,179-180,206-207`

**Description:** `NET_ADMIN` and `NET_RAW` granted to non-gateway containers (app-connector, quic-client) that don't need them.

**Fix:** Remove from non-gateway containers. Use init container for route setup if needed. Mark as dev-only.

### M5. FFI `agent_set_local_addr` Assumes 4-Byte Buffer Without Length Parameter

**File:** `core/packet_processor/src/lib.rs:979-1006`

**Description:** Takes `*const u8` for IP address, calls `slice::from_raw_parts(ip, 4)` with no length validation. If Swift passes shorter buffer → UB.

**Fix:** Add `ip_len` parameter to FFI function and validate `ip_len >= 4` before dereferencing.

### M6. Keepalive Interception on 5-Byte Packets Could Swallow QUIC

**File:** `core/packet_processor/src/lib.rs:284-294`

**Description:** `recv()` intercepts raw UDP packets as P2P keepalives when `data.len() == 5 && (data[0] == 0x10 || data[0] == 0x11)`. A legitimate 5-byte QUIC stateless reset matching this pattern would be silently consumed.

**Fix:** Add distinctive magic prefix to keepalive messages, or verify upper bits don't match QUIC header patterns.

---

## Low Findings

### L1. Blocking TCP Connect on Event Loop Thread

**File:** `app-connector/src/main.rs:1150`

**Tracked as:** MVP limitation (code comment acknowledges post-MVP migration needed)

### L2. Config Files Reference Cert Paths Without Startup Validation

**Files:** `deploy/config/connector.json:18-19`, `deploy/config/intermediate.json:5-6`

**Fix:** Add startup validation for cert/key file existence with clear error messages.

### L3. `from_utf8_lossy` on Service IDs May Create Routing Collisions

**File:** `intermediate-server/src/main.rs:542,627`

**Description:** Invalid UTF-8 sequences silently replaced with U+FFFD, potentially mapping different byte sequences to the same service ID string.

**Fix:** Use strict `String::from_utf8()`, reject invalid service IDs.

### L4. `setup-nat.sh` Uses Unsanitized Env Vars in `/proc` Paths

**File:** `deploy/docker-nat-sim/setup-nat.sh:46-47`

**Fix:** Validate interface names match `^[a-zA-Z0-9]+$` before use.

### L5. Verify No Force-Unwrap on NWEndpoint.Port in Binding Paths

**File:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`

**Status:** Current code appears to use `guard let` — verify during implementation.

---

## Info Findings

### I1. Verbose Logging of Network Topology at `info` Level

**Files:** All server components

**Fix:** Reduce client addresses and routing decisions to `debug` level for production.

### I2. Docker Compose Mounts Host `certs/` Directory

**File:** `deploy/docker-nat-sim/docker-compose.yml:70,213`

**Status:** Read-only mount (correct). Ensure `certs/` is in `.gitignore`.

---

## Research Areas (Original — Still Relevant)

### TLS Certificate Management
- Let's Encrypt integration for Intermediate Server
- Certificate rotation without downtime
- ACME protocol with UDP-based services (DNS-01 challenge)
- Certificate pinning in macOS Agent

### Client Authentication
- Mutual TLS (mTLS) between Agent <-> Intermediate
- Client certificate provisioning and revocation
- Token-based authentication as alternative
- MDM certificate distribution for enterprise

### Rate Limiting
- Per-connection rate limits on Intermediate Server
- Registration flood protection
- DATAGRAM throughput limits
- DDoS mitigation for public-facing QUIC endpoint

### Protocol Hardening
- Stateless retry tokens (QUIC anti-amplification)
- Registration ACK (currently fire-and-forget)
- Connection ID rotation
- Address validation during handshake

---

## References

- Current TLS: self-signed certs in `certs/` directory
- Deferred from Task 006 Phase 3: TLS cert management
- Deferred from `_context/README.md`: Registration ACK, rate limiting
- QUIC RFC 9000 Section 8: Address Validation
- Let's Encrypt ACME: https://letsencrypt.org/docs/
- Security review: 2026-02-21 (this document)
