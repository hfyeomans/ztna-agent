# Task State: Cloud Deployment

**Task ID:** 006-cloud-deployment
**Status:** In Progress
**Branch:** `feature/006-cloud-deployment`
**Last Updated:** 2026-01-31

---

## Overview

Deploy Intermediate Server and App Connector to cloud infrastructure for NAT testing and production readiness. Enables testing Agent behavior behind real NAT environments.

**Primary purposes:**
1. Test Agent behavior behind real NAT environments
2. Validate QAD (QUIC Address Discovery) with actual public IPs
3. **Validate P2P hole punching with real NATs** (from Task 005)
4. Prepare infrastructure for production deployment

**Read first:** [`tasks/_context/README.md`](../_context/README.md)

---

## Current Phase: Post-Cloud Deployment Tasks

> **Status (2026-01-31):** Tasks 1-5 complete. All protocol tasks done.
> 1. ~~AWS E2E validation~~ COMPLETE
> 2. ~~Config file mechanism~~ COMPLETE
> 3. ~~IP→Service routing~~ COMPLETE
> 4. ~~TCP support~~ COMPLETE
> 5. ~~ICMP support~~ COMPLETE
> 6. ~~Documentation updates~~ COMPLETE

---

## Phase 1 - Pi k8s Deployment ✅ COMPLETE

### Prerequisites
- [x] Task 004 complete (E2E Relay Testing - local validation) ✅
- [x] Task 005 complete (P2P Hole Punching - protocol implementation) ✅
- [x] Task 005a complete (Swift Agent Integration) ✅
- [x] Phase 0: Docker NAT simulation validated ✅
- [x] Pi k8s cluster access confirmed ✅

### What's Done
- Task planning documentation created
- P2P validation phase added (Phase 7) with NAT testing matrix
- Task 004 merged to master
- Task 005 merged to master (P2P protocol complete - 79 tests)
- Task 005a merged to master (Swift Agent integration)
- Feature branch created: `feature/006-cloud-deployment`
- Research updated with NAT testing requirements
- Cloud provider analysis completed (Vultr/DigitalOcean recommended)
- **P2P fixed port (4434) implemented in app-connector** ✅
- **Phase 0: Docker NAT Simulation completed** ✅
  - Created `deploy/docker-nat-sim/` environment
  - NAT gateway containers with iptables MASQUERADE
  - End-to-end relay test successful
  - Agent observed at 172.20.0.2 (NATted), Connector at 172.20.0.3 (NATted)
  - UDP tunnel echo working through relay
- **Phase 1: Pi k8s Deployment completed** ✅
  - Created `deploy/k8s/` Kustomize structure (base + overlays)
  - Built multi-arch images (arm64) and pushed to Docker Hub
  - Configured Cilium L2 announcements for LoadBalancer
  - Deployed intermediate-server, app-connector, echo-server to Pi cluster
  - **All components verified working:**
    - intermediate-server: Running, accepting QUIC connections
    - app-connector: Running, registers for 'echo-service', receives QAD (30s idle timeout expected)
    - echo-server: Running
    - LoadBalancer: 10.0.150.205:4433/UDP accessible from macOS
  - **macOS → k8s QUIC connection verified:**
    - Local app-connector connects to k8s intermediate-server
    - Successfully registers as connector
    - QAD returns observed address
  - Created comprehensive skill documentation (`deploy/k8s/k8s-deploy-skill.md`)

### Phase 1.1: macOS E2E Test ✅ COMPLETE

**Test Date:** 2026-01-25

**Test Setup:**
- macOS ZtnaAgent app (built from /tmp/ZtnaAgent-build)
- Extension pointing to k8s LoadBalancer: 10.0.150.205:4433
- VPN profile "ZTNA Agent" configured and connected

**Test Results:**
```
✅ macOS VPN tunnel established (utun6, 100.64.0.1)
✅ Routes configured (10.100.0.1/32 → utun6)
✅ QUIC connection to intermediate-server (via Cilium L2 LoadBalancer)
✅ QAD received (observed: 10.0.0.22:55625 - SNAT'd k8s node IP)
✅ Packets intercepted and tunneled through QUIC DATAGRAM
✅ Intermediate server received 84-byte relay data (ICMP ping)
⚠️  "No destination for relay" - expected (no service ID routing for 10.100.0.1)
```

**Key Findings:**
1. **VPN on macOS 26+ works** - No system dialog popup needed; shows "Connected" in Settings
2. **SNAT with externalTrafficPolicy: Cluster** - macOS appears as 10.0.0.22 (k8s node IP) to intermediate
3. **30-second idle timeout** - QUIC connection closes after 30s without traffic
4. **Service-based routing** - MVP routes by service ID, not destination IP
5. **84-byte payload** - ICMP ping (56 data + 8 ICMP header + 20 IP header overhead)

**Next Steps (Full E2E with echo-service):**
- Configure macOS agent to send traffic to echo-service destination
- Or modify routing to forward packets to registered app-connector

### Phase 1.2: Agent Registration ✅ COMPLETE

**Date:** 2026-01-25

**Implementation:**
- Added `agent_register(agent, service_id)` FFI function to Rust core
- Registration sends DATAGRAM: `[0x10, len, service_id_bytes]`
- Updated bridging header with `agent_register` declaration
- Swift calls registration after QUIC connection established
- Service ID hardcoded to "echo-service" (matches k8s app-connector)
- 81 tests passing in packet_processor crate

**Files Changed:**
- `core/packet_processor/src/lib.rs` - Added REG_TYPE_AGENT, Agent::register(), agent_register FFI
- `ios-macos/Shared/PacketProcessor-Bridging-Header.h` - Added agent_register declaration
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift` - Calls registerForService() after connect

### Phase 1.3: Agent Registration Verification ✅ COMPLETE

**Date:** 2026-01-25

**Test Results:**
```
k8s intermediate-server logs:
[2026-01-25T20:27:45Z INFO] Registration: Agent for service 'echo-service' (conn=8faf58ba...)
[2026-01-25T20:27:45Z INFO] Registering Agent targeting service 'echo-service'
```

**Verified:**
- ✅ Agent connects to k8s intermediate-server (10.0.150.205:4433)
- ✅ Agent receives QAD (observed: 10.0.0.22 - SNAT'd k8s node IP)
- ✅ Agent sends registration DATAGRAM with service ID 'echo-service'
- ✅ Intermediate-server recognizes Agent registration

**Discovered (Previously Thought to be Gap):**
- ~~⚠️ Full E2E relay blocked: Tunneled IP packets (DATAGRAM) lack service context~~
- **RESOLVED:** The routing logic was already implemented (implicit single-service-per-connection model)
- The issue was **timing** - Agent and Connector weren't connected simultaneously

### Phase 1.4: Connector Keepalive ✅ COMPLETE

**Date:** 2026-01-25

**Problem:** App-connector disconnected after 30s idle timeout, causing CrashLoopBackOff in k8s

**Solution:** Added QUIC keepalive mechanism
- `KEEPALIVE_INTERVAL_SECS = 10` constant in app-connector/src/main.rs
- `maybe_send_keepalive()` method calls `conn.send_ack_eliciting()` (QUIC PING)
- Called in main loop after `process_timeouts()`
- Connector now stays connected indefinitely (tested 20+ minutes)

**k8s Fix:** Added kustomize patch to override entrypoint (skip gosu)
```yaml
- patch: |-
    - op: add
      path: /spec/template/spec/containers/0/command
      value: ["/usr/local/bin/app-connector"]
  target:
    kind: Deployment
    name: app-connector
```

### Phase 1.5: Full E2E Relay Test ✅ COMPLETE

**Date:** 2026-01-25 21:02 UTC (Pi k8s)

### Phase 4: AWS EC2 Deployment ✅ COMPLETE

**Date:** 2026-01-25

**Infrastructure:**
- EC2 Instance: i-021d9b1765cb49ca7
- Elastic IP: 3.128.36.92
- Private IP (Tailscale VPC): 10.0.2.126
- Region: us-east-2

**Components Deployed:**
- intermediate-server (systemd service on port 4433)
- app-connector (systemd service, registers for 'echo-service')
- echo-server (UDP Python server on port 9999)

**Configuration Files Created:**
- `/etc/systemd/system/ztna-intermediate.service`
- `/etc/systemd/system/ztna-connector.service`
- `/etc/systemd/system/echo-server.service`
- `/home/ubuntu/ztna-agent/echo-server-udp.py`

**Skill Documentation:** `deploy/aws/aws-deploy-skill.md`

**Note:** Single EC2 deployment is stepping stone. Full P2P testing requires
Agent behind NAT connecting to cloud Intermediate (different network topology).

### Phase 4.1: AWS E2E Validation ✅ COMPLETE

**Date:** 2026-01-26

**Test Setup:**
- macOS Agent (SwiftUI VPN client) behind home NAT (108.7.224.33)
- AWS intermediate-server (3.128.36.92:4433)
- AWS app-connector (registered for 'echo-service')

**Issues Discovered & Resolved:**

1. **Tailscale Interference** - Disabled Tailscale on macOS to prevent asymmetric routing
2. **AWS Source/Destination Checking** - Disabled on EC2 for proper UDP response routing
3. **IPv6 Preference** - macOS preferred IPv6; added `NWProtocolIP.Options.version = .v4` to force IPv4
4. **⚠️ CRITICAL: Hardcoded Server IP in `agent_recv`** - Was set to Pi k8s IP (10.0.150.205) instead of AWS (3.128.36.92). QUIC stack rejected packets because source IP didn't match!

**Fix Applied:**
```swift
// PacketTunnelProvider.swift handleReceivedPacket()
// BEFORE (broken):
var ipBytes: [UInt8] = [10, 0, 150, 205]  // Pi k8s
// AFTER (working):
var ipBytes: [UInt8] = [3, 128, 36, 92]   // AWS
```

**Results - FULL E2E SUCCESS:**
```
QUIC connection established
Registered for service 'echo-service'
Keepalive timer started (interval: 10s)
QAD observed address: <client-ip>:59393
```

**Data Flow Verified:**
```
macOS (behind NAT) → Internet → AWS Intermediate Server
       ↓                              ↓
   108.7.224.33                  3.128.36.92
```

**Test Setup:**
- macOS Agent connected to k8s intermediate-server (10.0.150.205:4433)
- k8s app-connector registered for 'echo-service' (keepalive active)
- k8s echo-server running (UDP 9999)
- VPN routes 10.100.0.1/32 → utun6

**Test Command:**
```bash
echo "ZTNA-TEST" | nc -u -w1 10.100.0.1 9999
```

**Results - FULL E2E SUCCESS:**
```
[21:02:13Z] Received 38 bytes to relay from aa7443... (Agent)
[21:02:13Z] Found destination e8780... for aa7443...
[21:02:13Z] Relayed 38 bytes from aa7443... to e8780... (→ Connector)
[21:02:13Z] Received 38 bytes to relay from e8780... (echo response)
[21:02:13Z] Found destination 176b5... for e8780...
[21:02:13Z] Relayed 38 bytes from e8780... to 176b5... (→ Agent)
```

**Data Flow Verified:**
```
macOS Agent → Intermediate → Connector → echo-server
                                              ↓
macOS Agent ← Intermediate ← Connector ← echo response
```

**Key Notes:**
- Only UDP traffic supported (Connector drops ICMP/TCP)
- Service-based routing works: Agent(echo-service) ↔ Connector(echo-service)
- macOS Agent still has 30s timeout - needs keepalive (future task)

### Phase 4.2: Config File Mechanism (Task #2) COMPLETE

**Date:** 2026-01-31

**Implementation Summary:**
All three components now support dynamic configuration instead of hardcoded values.

**macOS Agent (Swift):**
- `ContentView.swift`: Added `serverHost`, `serverPort`, `serviceId` to VPNManager with UserDefaults persistence
- Config passed to extension via `NETunnelProviderProtocol.providerConfiguration` dictionary
- UI fields for editing config (disabled when connected)
- `PacketTunnelProvider.swift`: `loadConfiguration()` reads from `providerConfiguration` at tunnel start
- `parseIPv4()` derives `serverIPBytes` from host string (single source of truth, eliminates dual-hardcoding bug)

**App Connector (Rust):**
- JSON config file support (`--config` flag or default paths `/etc/ztna/connector.json`, `connector.json`)
- Config structs: `ConnectorConfig`, `IntermediateServerConfig`, `ServiceConfig`, `P2PConfig`
- CLI args override config file values (backwards compatible)

**Intermediate Server (Rust):**
- JSON config file support (`--config` flag or default paths `/etc/ztna/intermediate.json`, `intermediate.json`)
- Config struct: `ServerConfig` (port, bind_addr, external_ip, cert_path, key_path)
- Named flags: `--port`, `--cert`, `--key`, `--bind`, `--external-ip`
- Backwards compatible with legacy positional args

**Example configs:** `deploy/config/{connector,intermediate,agent}.json`

**Files Changed:**
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift`
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`
- `app-connector/Cargo.toml` + `app-connector/src/main.rs`
- `intermediate-server/Cargo.toml` + `intermediate-server/src/main.rs`
- `deploy/config/connector.json` (new)
- `deploy/config/intermediate.json` (new)
- `deploy/config/agent.json` (new)

### Phase 4.3: IP→Service Routing (Task #3) COMPLETE

**Date:** 2026-01-31

**Architecture:** 0x2F Service-Routed Datagram Protocol

**Protocol Format:**
```
[0x2F, service_id_len, service_id_bytes..., ip_packet...]
```

**Changes:**
- `intermediate-server/src/registry.rs`: `agent_targets` changed from `HashMap<ConnectionId, String>` to `HashMap<ConnectionId, HashSet<String>>`. Added `find_agent_for_service()`. Agent can now register for multiple services.
- `intermediate-server/src/main.rs`: Added 0x2F handler in `process_datagrams()` and `relay_service_datagram()` method. Strips wrapper before forwarding to Connector.
- `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`: Added `ServiceConfig` struct, route table `[UInt32: String]`, `extractDestIPv4()`, `sendRoutedDatagram()`. Wraps outgoing packets with 0x2F header when route table populated. Registers for all configured services.
- `ios-macos/ZtnaAgent/ZtnaAgent/ContentView.swift`: Passes `services` array in providerConfiguration.
- `deploy/config/agent.json`: Updated with services array example.

**Backward Compatibility:** Non-0x2F datagrams still use implicit single-service routing.

**Tests:** 14 registry tests (including 2 new: `test_multi_service_agent`, `test_find_agent_for_service`) + 1 integration test + 81 packet_processor tests all pass.

### Phase 4.4: TCP Support in App Connector (Task #4) COMPLETE

**Date:** 2026-01-31

**Architecture:** Userspace TCP proxy with session tracking

**How it works:**
```
Agent (macOS kernel TCP) → TUN → PacketTunnelProvider → QUIC DATAGRAM →
Intermediate → Connector → [TCP proxy] → Backend TcpStream → Response →
[Build TCP/IP packet] → QUIC DATAGRAM → Agent → TUN → macOS kernel
```

**Session lifecycle:**
1. SYN received → open `TcpStream::connect_timeout()` to backend, send SYN-ACK
2. ACK received → mark session established
3. PSH|ACK received → `stream.write(payload)`, send ACK with consumed bytes
4. FIN received → close stream, send FIN-ACK
5. Backend sends data → `stream.read()` polled each loop iteration, build PSH|ACK packet
6. Backend closes → send FIN to agent
7. Error → send RST

**Changes:**
- `app-connector/src/main.rs`:
  - Added `TcpSession` struct tracking stream, seq/ack numbers, flow 4-tuple, state
  - Added `tcp_sessions: HashMap<(Ipv4Addr, u16, Ipv4Addr, u16), TcpSession>` to Connector
  - Added `handle_tcp_packet()` - dispatches SYN/ACK/FIN/RST handling
  - Added `send_ip_packet()` - sends constructed IP packets via QUIC DATAGRAM
  - Added `process_tcp_sessions()` - polls all backend TcpStreams for return data
  - Added `build_tcp_packet()` - constructs TCP/IP packet with proper checksum
  - Added `tcp_checksum()` - TCP checksum with pseudo-header
  - Modified `forward_to_local()` - dispatches protocol 6 to TCP handler
  - TCP sessions cleaned up on idle timeout (120s) in `process_timeouts()`
  - Non-blocking TcpStream with TCP_NODELAY for low latency
  - Max TCP payload per datagram: 1310 bytes (1350 - 20 IP - 20 TCP)

**Constants added:** `TCP_FIN`, `TCP_SYN`, `TCP_RST`, `TCP_PSH`, `TCP_ACK`, `MAX_TCP_PAYLOAD`, `TCP_SESSION_TIMEOUT_SECS`

**Tests:** 16 unit tests pass (5 new: `test_tcp_flags`, `test_build_tcp_packet_syn_ack`, `test_build_tcp_packet_with_data`, `test_tcp_checksum_validity`, `test_max_tcp_payload_fits_datagram`)

### Phase 4.5: ICMP Support in App Connector (Task #5) COMPLETE

**Date:** 2026-01-31

**Architecture:** Connector-local Echo Reply (no backend forwarding)

The Connector acts as the virtual IP endpoint for ICMP. When the Agent sends `ping 10.100.0.1`, the raw ICMP Echo Request arrives via QUIC DATAGRAM. The Connector parses it, constructs an Echo Reply with swapped source/destination IPs, and sends it back through the tunnel. The macOS kernel's ICMP stack sees the reply and reports the ping as successful.

**Changes:**
- `app-connector/src/main.rs`:
  - Added `handle_icmp_packet()` - parses Echo Request (type 8, code 0), calls `build_icmp_reply()`
  - Added `build_icmp_reply()` - constructs IP+ICMP Echo Reply packet, preserves identifier/sequence/data
  - Added `icmp_checksum()` - standard one's complement checksum for ICMP
  - Modified `forward_to_local()` - dispatches protocol 1 to ICMP handler

**Tests:** 18 unit tests pass (2 new: `test_build_icmp_reply`, `test_icmp_checksum_validity`)

### Phase 4.6: Documentation Updates COMPLETE

**Date:** 2026-01-31

**Updated documentation across 4 files:**

1. **`tasks/_context/components.md`** - Updated component capabilities (TCP/ICMP/0x2F/config), rewrote Service Registration & Routing Protocol section, updated deferred items, phase status table, test counts
2. **`tasks/_context/README.md`** - Updated architecture diagrams, deferred items (TCP/ICMP/Config marked done), cloud deployment architecture, test counts, registration protocol glossary
3. **`docs/architecture.md`** - Added 0x2F Service-Routed Datagram Protocol section, Split-Tunnel Routing section, updated Agent/Connector responsibilities, config-driven service definitions
4. **`tasks/_context/testing-guide.md`** - Added AWS Cloud Comprehensive Demo (5-terminal setup), updated all unit test counts (86→114, grand total 147→175), added config file reference, documented return-path DATAGRAM→TUN gap

### Critical Finding: Return-Path Gap

**Discovery:** The Agent cannot complete the inbound path for response packets.

**Outgoing (working):** macOS App → TUN → PacketTunnelProvider.readPackets() → agent_send_datagram() → QUIC → Intermediate → Connector → backend

**Incoming (missing):** Connector → Intermediate → Agent QUIC connection → ??? → packetFlow.writePackets() → TUN → macOS kernel

**What's needed:**
1. `agent_recv_datagram()` FFI function in Rust core to extract received DATAGRAMs from QUIC
2. Swift code in PacketTunnelProvider to poll for incoming DATAGRAMs and call `packetFlow.writePackets()` to inject into TUN
3. Concurrent receive loop that doesn't block the existing outgoing send loop

**Impact:** Blocks `ping 10.100.0.1`, `curl 10.100.0.2:8080`, and any application-level response delivery. The Connector and Intermediate sides are complete.

**Detailed documentation:** See `tasks/_context/testing-guide.md` → "Return-Path Gap" section.

### What's Next
1. ~~Test macOS ZtnaAgent app connecting to Pi k8s intermediate-server~~ DONE
2. ~~Implement Agent registration for relay routing~~ DONE
3. ~~Verify registration appears in k8s logs~~ DONE
4. ~~Test full E2E relay~~ DONE
5. ~~Deploy to AWS for public IP testing~~ DONE (Phase 4 complete)
6. ~~Remove hard-coded 1.1.1.1 from codebase~~ DONE
7. ~~Document dynamic configuration requirements in plan.md~~ DONE
8. ~~Implement Config File Mechanism (Task #2)~~ DONE
9. ~~Implement IP→Service Routing (Task #3)~~ DONE
10. ~~Add TCP Support to App Connector (Task #4)~~ DONE
11. ~~Add ICMP Support (Task #5)~~ DONE
12. ~~Documentation updates~~ DONE (Phase 4.6)
13. **Implement Agent return-path** (agent_recv_datagram FFI + packetFlow.writePackets)
14. Test P2P hole punching with home NAT → cloud setup

---

## Decisions Made

| Question | Decision | Notes |
|----------|----------|-------|
| AWS VPC | Create NEW VPC | Dedicated ZTNA testing environment |
| P2P Listen Port | Fixed port **4434** | Predictable firewall rules |
| TLS Certificates | Self-signed (MVP) | Domain + Let's Encrypt for later |
| Home k8s Cluster | Pi cluster available | 10.0.150.101-108 |

---

## Home MVP Infrastructure

**Raspberry Pi Kubernetes Cluster:**

| Role | IP Address | Notes |
|------|------------|-------|
| Control Plane | 10.0.150.101 | kubectl access confirmed |
| Worker 1 | 10.0.150.102 | |
| Worker 2 | 10.0.150.103 | |
| Worker 3 | 10.0.150.104 | |
| Worker 4 | 10.0.150.105 | |
| Worker 5 | 10.0.150.106 | |
| Worker 6 | 10.0.150.107 | |
| Worker 7 | 10.0.150.108 | |

**Purpose:** True NAT-to-NAT hole punching validation (both Agent and Connector behind home NAT)

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Task 004 (E2E Testing) | ✅ Complete | Local testing passed (61+ tests) |
| Task 005 (P2P Protocol) | ✅ Complete | Protocol implementation (79 tests) |
| Task 005a (Swift Integration) | ✅ Complete | macOS Agent wired up |
| AWS CLI | ✅ Configured | Access confirmed |
| DigitalOcean API | ✅ Available | API key ready |
| Pi k8s Cluster | ✅ Available | kubectl access confirmed |

---

## Deployment Components

### Pi k8s Cluster (Phase 1 - Complete)

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Pi k8s (LoadBalancer 10.0.150.205:4433) | ✅ Running |
| App Connector | Pi k8s (ClusterIP, registers for 'echo-service') | ✅ Running |
| Echo Server | Pi k8s (ClusterIP, test service) | ✅ Running |
| TLS Certificates | Self-signed (manual secret creation) | ✅ Done |
| Cilium L2 | LoadBalancer IP announcements | ✅ Working |

**Key Configuration:**
- LoadBalancer: `externalTrafficPolicy: Cluster` (required for Cilium L2)
- Docker Hub images: `hyeomans/ztna-{intermediate-server,app-connector,echo-server}:latest`
- Skill guide: `deploy/k8s/k8s-deploy-skill.md`

### Cloud VM (Phase 2 - Pending)

| Component | Target | Status |
|-----------|--------|--------|
| Intermediate Server | Cloud VM (public IP) | 🔲 |
| App Connector | Cloud VM (same or separate) | 🔲 |
| TLS Certificates | Let's Encrypt or self-signed | 🔲 |
| Firewall Rules | UDP 4433 inbound | 🔲 |

---

## Critical Testing Insight

> **IMPORTANT:** Cloud VMs have **direct public IPs** - they are NOT behind NAT.
> To test hole punching, the **Agent must be behind real NAT** (home network, mobile hotspot, etc.)

### What Cloud Deployment Tests

| Test Type | Cloud-Only | Cloud + Home NAT |
|-----------|------------|------------------|
| DATAGRAM relay | ✅ Yes | ✅ Yes |
| QAD public IP discovery | ✅ Yes | ✅ Yes |
| Cross-internet latency | ✅ Yes | ✅ Yes |
| **P2P hole punching** | ❌ No* | ✅ Yes |
| **NAT type behavior** | ❌ No* | ✅ Yes |

*Both cloud peers have direct public IPs - no NAT to punch through.

### Minimum Test Topology for P2P

```
┌─────────────────┐                    ┌─────────────────────────┐
│  Home Network   │                    │     Cloud VM            │
│  (Behind NAT)   │                    │  (Direct Public IP)     │
│                 │                    │                         │
│  ┌───────────┐  │                    │  Intermediate Server    │
│  │   Agent   │──┼──► Home Router ───►│       + Connector       │
│  │  (macOS)  │  │       NAT          │                         │
│  └───────────┘  │                    └─────────────────────────┘
└─────────────────┘
```

---

## P2P Validation Scope (From Task 005)

> Task 005 implements P2P protocol locally. This task validates it with real NATs.

### Testable Only with Cloud Deployment + Real NAT

| Feature | Local Testing | Cloud Testing |
|---------|---------------|---------------|
| NAT hole punching | ❌ Not possible | ✅ Real NAT behavior |
| Reflexive candidates | ❌ Returns 127.0.0.1 | ✅ Actual public IP |
| NAT type detection | ❌ No NAT to detect | ✅ Various NAT types |
| Cross-network latency | ❌ Always localhost | ✅ Real network paths |

### NAT Types to Test

| NAT Type | Hole Punching | Priority | Common Location |
|----------|---------------|----------|-----------------|
| Full Cone | Easy | P1 | Most home routers |
| Restricted Cone | Medium | P1 | Some home routers |
| Port Restricted | Medium | P1 | Some enterprise |
| Symmetric | Hard (relay fallback) | P2 | Carrier-grade, enterprise |

### Key P2P Validation Items

- Address exchange via Intermediate (real NAT)
- Simultaneous UDP open (hole punch timing)
- Direct QUIC connection after hole punch
- Path selection (direct vs relay RTT comparison)
- Fallback to relay when hole punching fails
- Connection priority order (Direct LAN → Direct WAN → Relay)

---

## Session Resume Instructions

1. Read `tasks/_context/README.md` for project context
2. Read this file for task state
3. Check `todo.md` for current progress
4. Ensure on branch: `feature/006-cloud-deployment`
5. Continue with next unchecked item in `todo.md`
