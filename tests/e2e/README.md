# E2E Relay Testing

**Task ID:** 004-e2e-relay-testing
**Last Updated:** 2026-01-19

---

## Overview

This directory contains end-to-end tests for the ZTNA relay infrastructure. The tests validate that the relay components work correctly together.

---

## Architecture

### Full ZTNA Relay Path (Production)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PRODUCTION RELAY PATH                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    QUIC/DATAGRAM     ┌────────────────────┐               │
│  │              │ ──────────────────►  │                    │               │
│  │    Agent     │     (port 4433)      │   Intermediate     │               │
│  │   (macOS)    │ ◄──────────────────  │      Server        │               │
│  │              │    QUIC/DATAGRAM     │                    │               │
│  └──────────────┘                      └─────────┬──────────┘               │
│        ▲                                         │                          │
│        │                                         │ QUIC/DATAGRAM            │
│        │ Virtual                                 ▼                          │
│        │ Interface                      ┌────────────────────┐              │
│        │ (utun)                         │                    │              │
│        │                                │   App Connector    │              │
│  ┌─────┴────────┐                       │                    │              │
│  │              │                       └─────────┬──────────┘              │
│  │  User App    │                                 │                          │
│  │  (browser,   │                                 │ UDP                      │
│  │   curl, etc) │                                 ▼                          │
│  │              │                       ┌────────────────────┐              │
│  └──────────────┘                       │                    │              │
│                                         │   Local Service    │              │
│                                         │   (protected app)  │              │
│                                         │                    │              │
│                                         └────────────────────┘              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow Detail

```
User App                Agent              Intermediate         Connector        Service
   │                      │                     │                   │               │
   │──── IP Packet ──────►│                     │                   │               │
   │                      │                     │                   │               │
   │               [Capture via utun]           │                   │               │
   │                      │                     │                   │               │
   │                      │── QUIC DATAGRAM ───►│                   │               │
   │                      │   [0x00][payload]   │                   │               │
   │                      │                     │                   │               │
   │                      │                     │── QUIC DATAGRAM ─►│               │
   │                      │                     │   [relay data]    │               │
   │                      │                     │                   │               │
   │                      │                     │                   │── UDP Pkt ───►│
   │                      │                     │                   │               │
   │                      │                     │                   │◄── UDP Pkt ───│
   │                      │                     │                   │               │
   │                      │                     │◄── QUIC DATAGRAM ─│               │
   │                      │                     │                   │               │
   │                      │◄── QUIC DATAGRAM ───│                   │               │
   │                      │                     │                   │               │
   │◄─── IP Packet ───────│                     │                   │               │
   │                      │                     │                   │               │
```

---

## Current Test Coverage

### What IS Being Tested (Phase 1 - Infrastructure Validation)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CURRENT TEST COVERAGE (Phase 1)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Tests verify that components START and STAY RUNNING:                       │
│                                                                              │
│      ┌────────────────────┐                                                 │
│      │   Intermediate     │  ✓ Process starts                               │
│      │      Server        │  ✓ Binds to port 4433                           │
│      │    (port 4433)     │  ✓ No crash/panic                               │
│      └────────────────────┘                                                 │
│                                                                              │
│      ┌────────────────────┐                                                 │
│      │   App Connector    │  ✓ Process starts                               │
│      │                    │  ✓ Connects to Intermediate                     │
│      │                    │  ✓ No crash/panic                               │
│      └────────────────────┘                                                 │
│                                                                              │
│  Tests also verify echo server directly (NOT through relay):                │
│                                                                              │
│      ┌────────┐           ┌────────────────────┐                            │
│      │   nc   │──────────►│   Echo Server      │  ✓ Responds to UDP         │
│      │ (test) │◄──────────│    (port 9999)     │  ✓ Various payload sizes   │
│      └────────┘  DIRECT   └────────────────────┘  ✓ Pattern integrity       │
│                  UDP                                                         │
│                                                                              │
│  ⚠️  NOTE: This path BYPASSES the QUIC relay entirely!                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### What is NOT Being Tested (Gap Analysis)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TEST GAPS (Not Yet Covered)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ❌ QUIC Connection Establishment                                           │
│     - TLS handshake                                                         │
│     - ALPN negotiation (b"ztna-v1")                                         │
│     - Certificate validation                                                │
│                                                                              │
│  ❌ QUIC DATAGRAM Relay                                                     │
│     - Data flowing through Intermediate                                     │
│     - Connector receiving relayed datagrams                                 │
│     - MAX_DATAGRAM_SIZE (1350) enforcement by QUIC layer                    │
│                                                                              │
│  ❌ Connector Registration Protocol                                         │
│     - Registration message format [0x11][len][service_id]                   │
│     - Service routing                                                       │
│                                                                              │
│  ❌ QAD (QUIC Address Discovery)                                            │
│     - Observed address reporting                                            │
│     - NAT traversal preparation                                             │
│                                                                              │
│  ❌ End-to-End Through Relay                                                │
│     - Complete path: Client → Intermediate → Connector → Service            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Test Components

### Component Startup Order

```
    ┌─────────────────────────────────────────────────────────────────────┐
    │                                                                     │
    │   1. Echo Server (port 9999)                                       │
    │      └── Provides UDP echo service for testing                     │
    │                                                                     │
    │   2. Intermediate Server (port 4433)                               │
    │      └── QUIC server that relays datagrams                         │
    │                                                                     │
    │   3. App Connector                                                  │
    │      └── Connects to Intermediate, forwards to Echo Server         │
    │                                                                     │
    └─────────────────────────────────────────────────────────────────────┘
```

### Port Assignments

| Component           | Port | Protocol | Purpose                        |
|---------------------|------|----------|--------------------------------|
| Intermediate Server | 4433 | QUIC/UDP | Relay server (TLS over UDP)    |
| Echo Server         | 9999 | UDP      | Test service (echoes payload)  |
| Connector           | N/A  | QUIC     | Client to Intermediate         |

---

## Running Tests

### Quick Start

```bash
# From project root
cd tests/e2e

# Run all tests (builds components first)
./run-mvp.sh

# Run with pre-built binaries
./run-mvp.sh --skip-build

# Run specific scenario
./run-mvp.sh --skip-build --scenario udp-connectivity

# Keep components running after tests (for debugging)
./run-mvp.sh --skip-build --keep
```

### Manual Component Testing

```bash
# Start components individually for debugging
source lib/common.sh
setup_directories
start_echo_server 9999
start_intermediate certs/cert.pem certs/key.pem
start_connector

# Check component status
check_component_running "intermediate"
check_component_running "connector"
check_component_running "echo"

# View logs
cat artifacts/logs/intermediate-server.log
cat artifacts/logs/app-connector.log
cat artifacts/logs/echo-server.log

# Stop everything
stop_all_components
```

---

## Test Scenarios

### Phase 1: Infrastructure Validation (Current)

| Test | Description | Status |
|------|-------------|--------|
| `udp-connectivity.sh` | Component health checks | ✅ 5 tests |
| `udp-echo.sh` | Direct echo server tests | ✅ 4 tests |
| `udp-boundary.sh` | Payload size tests | ✅ 5 tests |

### Phase 2: Protocol Validation (Planned)

| Test | Description | Status |
|------|-------------|--------|
| ALPN validation | Verify `ztna-v1` negotiation | 🔲 Needs QUIC client |
| Registration format | Test `[0x11][len][service_id]` | 🔲 Needs QUIC client |
| DATAGRAM relay | Data through Intermediate | 🔲 Needs QUIC client |

---

## What's Needed for True Relay Testing

To test the actual relay path, we need a **QUIC test client** that can:

1. Establish QUIC connection to Intermediate Server (port 4433)
2. Negotiate ALPN `b"ztna-v1"`
3. Send QUIC DATAGRAMs with test payloads
4. Receive relayed responses

### Options

1. **Rust QUIC Client** - Small binary using `quiche` crate
2. **Use Agent Component** - Requires iOS Simulator / macOS entitlements
3. **Protocol-level tests** - Mock QUIC at lower level

The Agent component (iOS/macOS Network Extension) cannot be easily run outside
the simulator/device due to entitlement requirements, hence the need for a
dedicated test client.

---

## Directory Structure

```
tests/e2e/
├── README.md                 # This file
├── run-mvp.sh               # Main test orchestrator
├── lib/
│   └── common.sh            # Shared functions (zsh)
├── scenarios/
│   ├── udp-connectivity.sh  # Component health tests
│   ├── udp-echo.sh          # Echo functionality tests
│   └── udp-boundary.sh      # Payload size tests
├── config/
│   └── env.local            # Environment configuration
├── fixtures/
│   └── echo-server/         # Rust UDP echo server
│       ├── Cargo.toml
│       └── src/main.rs
└── artifacts/               # Generated at runtime
    ├── logs/                # Component logs
    └── metrics/             # Test metrics (JSON)
```

---

## Troubleshooting

### Port Already in Use

```bash
# Find and kill processes on specific ports
lsof -i :4433
lsof -i :9999
pkill -f "intermediate-server"
pkill -f "app-connector"
pkill -f "udp-echo"
```

### Component Fails to Start

1. Check logs in `artifacts/logs/`
2. Verify certificates exist in `certs/`
3. Ensure no other processes on required ports

### Tests Pass but Relay Not Tested

This is expected for Phase 1. Current tests validate:
- Components can start and run
- Echo server works (directly)
- Infrastructure is healthy

Actual relay testing requires Phase 2+ with a QUIC test client.

---

## Related Documentation

- [Task State](../../tasks/004-e2e-relay-testing/state.md)
- [Task TODO](../../tasks/004-e2e-relay-testing/todo.md)
- [Project Context](../../tasks/_context/README.md)
- [Component Architecture](../../tasks/_context/components.md)
