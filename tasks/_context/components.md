# Component Status & Dependencies

**Last Updated:** 2026-01-19

---

## Component Status

### 001: Agent QUIC Client ✅ COMPLETE

**Location:** `core/packet_processor/` + `ios-macos/ZtnaAgent/Extension/`

| Milestone | Status | Commit |
|-----------|--------|--------|
| Phase 1: Rust QUIC Client | ✅ Done | `958ce3f` |
| Phase 1.5: Code Quality | ✅ Done | `229448b` |
| Phase 2: Swift UDP Integration | ✅ Done | `286df2a` |

**Capabilities:**
- Creates QUIC connections via quiche
- Sends/receives QUIC DATAGRAMs
- Parses QAD OBSERVED_ADDRESS messages
- Tunnels intercepted IP packets
- Thread-safe state management

**Waiting on:** Intermediate Server (002) for testing

---

### 002: Intermediate Server ✅ COMPLETE

**Location:** `intermediate-server/`

**Capabilities:**
- QUIC server accepting connections (mio event loop)
- QAD: report observed address to clients (7-byte format)
- DATAGRAM relay between agent/connector pairs
- Client registry for routing (connection-based)
- Integration test (handshake + QAD verified)

**Critical Compatibility:**
- ALPN: `b"ztna-v1"` (matches Agent)
- QAD: DATAGRAM only, 7-byte IPv4 format

---

### 003: App Connector ✅ COMPLETE

**Location:** `app-connector/`

**Dependencies:** 002 (Intermediate Server)

| Milestone | Status | Commit |
|-----------|--------|--------|
| Phase 1: QUIC Client + UDP Forwarding | ✅ Done | `7ec1708` |

**Capabilities:**
- QUIC client via quiche (mio event loop, not tokio)
- Registers as Connector (0x11 protocol)
- Parses QAD OBSERVED_ADDRESS messages
- Decapsulates IPv4/UDP packets from DATAGRAMs
- Forwards UDP payload to configurable local service
- Constructs return IP/UDP packets with proper checksums
- Integration test (handshake + QAD + registration verified)

**Critical Compatibility:**
- ALPN: `b"ztna-v1"` (matches Agent/Intermediate)
- MAX_DATAGRAM_SIZE: 1350
- Registration: `[0x11][len][service_id]`
- QAD: 7-byte IPv4 format (0x01 + IP + port)

**Key Design Decisions:**
- **mio over tokio**: Matches Intermediate Server's sans-IO model
- **UDP-only for MVP**: TCP requires TUN/TAP or TCP state tracking (deferred)
- **No registration ACK**: Server doesn't acknowledge; treat as best-effort

**Deferred to Post-MVP:**
- TCP support (requires TUN/TAP)
- ICMP support
- Automatic reconnection
- Config file (TOML)

---

### 004: E2E Relay Testing 🔄 IN PROGRESS

**Location:** `tests/e2e/`

**Dependencies:** 002, 003

**Status:**

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Infrastructure | ✅ Done | 14 tests passing (component startup, direct echo) |
| Phase 1.5: QUIC Test Client | ✅ Done | IP/UDP packet construction, E2E relay VERIFIED |
| Phase 2: Protocol Validation | 🔲 Next | ALPN, registration, MAX_DATAGRAM_SIZE |
| Phase 3: Relay Validation | ✅ Done | Full relay path verified (Agent→Intermediate→Connector→Echo→back) |

**Capabilities Built:**
- Test framework (`lib/common.sh`) with component lifecycle
- UDP echo server fixture (`fixtures/echo-server/`)
- **QUIC test client** (`fixtures/quic-client/`) for sending DATAGRAMs
  - Agent registration (`--service <id>`)
  - IP/UDP packet construction (`--send-udp --dst ip:port`)
  - IPv4 header checksum calculation (RFC 1071)
- Test scenarios for connectivity, echo, boundary conditions
- Architecture documentation (`tests/e2e/README.md`)

**E2E Relay Verified (2026-01-19):**
```
QUIC Client → Intermediate → Connector → Echo Server → back
✅ Full round-trip: 42-byte IP/UDP packet, 14-byte payload echoed
```

**Bug Fixes Applied:**
- App Connector: Initial QUIC handshake not sent (added `send_pending()`)
- App Connector: Local socket not registered with mio poll (return traffic lost)

**Important Distinction:**
- Task 001 Agent = Production macOS NetworkExtension (intercepts system packets)
- QUIC Test Client = Test harness CLI (sends arbitrary DATAGRAMs from scripts)

**Capabilities Needed:**
- NAT testing (Intermediate on cloud)
- Latency measurement
- Failure scenario testing

---

### 005: P2P Hole Punching 🔲 NOT STARTED

**Location:** Updates to Agent + Connector

**Dependencies:** 002, 003, 004 (relay working first)

**Capabilities needed:**
- Address exchange via Intermediate
- Simultaneous open (hole punch)
- QUIC connection migration
- Path selection (prefer direct)
- Fallback to relay

---

### 006: Cloud Deployment 🔲 NOT STARTED

**Location:** Cloud infrastructure + deployment scripts

**Dependencies:** 004 (E2E Testing - local validation first)

**Purpose:**
- Deploy Intermediate Server and App Connector to cloud
- Enable NAT testing with real public IPs
- Validate QAD with actual network conditions
- Prepare infrastructure for production

**Deployment Targets:**
| Component | Target |
|-----------|--------|
| Intermediate Server | Cloud VM with public IP |
| App Connector | Cloud VM (same or separate) |
| Test Service | Cloud VM (localhost) |

**Capabilities needed:**
- Cloud VM provisioning (DigitalOcean/AWS/Vultr/GCP)
- TLS certificate management (self-signed or Let's Encrypt)
- Systemd service configuration
- Firewall rules (UDP 4433)
- Remote Agent testing (NAT traversal)

**Key Decisions (TBD):**
| Decision | Options | Status |
|----------|---------|--------|
| Cloud Provider | DO, AWS, Vultr, GCP | TBD |
| Deployment | Single VM vs Separate VMs | TBD |
| Certificates | Self-signed vs Let's Encrypt | TBD |
| Automation | Manual, Terraform, Ansible | TBD |

---

## Dependency Graph

```
                    ┌─────────────────────────┐
                    │  001: Agent Client      │
                    │  ✅ COMPLETE            │
                    └───────────┬─────────────┘
                                │
                                │ requires server to test
                                ▼
                    ┌─────────────────────────┐
                    │  002: Intermediate      │
                    │  Server                 │
                    │  ✅ COMPLETE            │
                    └───────────┬─────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
    ┌─────────────────────────┐   ┌─────────────────────────┐
    │  003: App Connector     │   │  004: E2E Testing       │
    │  ✅ COMPLETE            │   │  🔄 IN PROGRESS         │
    └───────────┬─────────────┘   └───────────┬─────────────┘
                │                             │
                └─────────────┬───────────────┘
                              │
                              │ relay working locally
                              ▼
                    ┌─────────────────────────┐
                    │  005: P2P Hole Punching │
                    │  🔲 NOT STARTED         │
                    │  ★ PRIMARY GOAL ★       │
                    └───────────┬─────────────┘
                                │
                                │ needs NAT testing
                                ▼
                    ┌─────────────────────────┐
                    │  006: Cloud Deployment  │
                    │  🔲 NOT STARTED         │
                    │  (NAT testing, prod)    │
                    └─────────────────────────┘
```

---

## Critical Path

**Shortest path to working relay (local):**
1. ✅ 001: Agent Client (done)
2. ✅ 002: Intermediate Server (done)
3. ✅ 003: App Connector (done)
4. 🔄 004: E2E Testing (relay VERIFIED, protocol validation next)

**Path to P2P (primary goal):**
- All of above + 005: P2P Hole Punching

**Path to production deployment:**
- All of above + 006: Cloud Deployment (NAT testing, production readiness)

---

## Inter-Component Communication

| From | To | Protocol | Port |
|------|----|----------|------|
| Agent | Intermediate | QUIC/UDP | 4433 |
| Connector | Intermediate | QUIC/UDP | 4433 |
| Agent | Connector (P2P) | QUIC/UDP | dynamic |
| Connector | Local App | TCP/UDP | configurable |

---

## Shared Code

| Module | Used By | Location |
|--------|---------|----------|
| QAD message format | Agent, Intermediate, Connector | TBD (shared crate) |
| QUIC config | All Rust components | TBD (shared crate) |
