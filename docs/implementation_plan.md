# Zero Trust Network Access (ZTNA) Agent - Research & Architecture Plan

## Goal Description
Design a Zero Trust Network Access system consisting of an Endpoint Agent (kernel-level packet interception), an Intermediate System (QUIC Address Discovery/Signaling), and an Application Connector. Create an MVP "Hello World" driver on macOS to demonstrate raw packet interception.

## User Review Required
> [!IMPORTANT]
> **Kernel vs. System Extensions**: On macOS, Kernel Extensions (kexts) are deprecated and difficult to distribute. We will target **Network Extensions (System Extensions)** which run in userspace but provide packet-level access (e.g., `NEPacketTunnelProvider` or `NEFilterPacketProvider`). This is the modern, supported approach.
>
> **Language Choice**: User has strictly requested **Rust** or **C++**. We will remove Go from consideration. We will evaluate `quinn` (Rust) and `quiche` (Rust/C++ binding) for the agent.

## Research Areas

### 1. macOS Packet Interception
- **Network Extension Framework**: Focus on `NEPacketTunnelProvider` (for VPN-like tunneling).
- **Raw Sockets**: Not viable for seamless redirection in modern macOS; System Extensions are required.

### 2. QUIC & Address Discovery
- **Libraries**:
    - **Rust (Pure)**: `quinn`, `s2n-quic`.
    - **Rust/C++ (Cloudflare)**: `quiche`.
- **Address Discovery**: Investigate `quiche` support for `ADD_ADDRESS` and NAT traversal compared to `quinn`/`iroh`.

### 3. Architecture Components
- **Endpoint Agent**: Intercepts traffic, encapsulates in QUIC.
- **Intermediate System**: Publicly accessible, handles initial handshake coordination (hole punching).
- **App Connector**: Docker container, decapsulates QUIC, forwards to target app.

## Proposed Strategy for MVP ("Hello World")
1.  **Mechanism**: `NEPacketTunnelProvider` (System Extension).
2.  **Language**: Swift for OS interaction, **Rust (via FFI)** for packet logic.
3.  **Features**:
    - Install and Start the Extension.
    - Create a virtual network interface.
    - Intercept all traffic to specific test IP (e.g., `1.1.1.1`).
    - Pass packet to Rust static lib.
    - Rust decodes header and logs it.

## Plan Steps
### Phase 1: Project Setup
1.  Create Xcode Project (SwiftUI App + Network Extension Target).
2.  Initialize Rust crate (`packet_processor`).
3.  Configure FFI (Swift bridging header).

### Phase 2: Implementation
4.  Implement basic `NEPacketTunnelProvider` lifecycle.
5.  Implement Rust logic to parse IP headers (using `etherparse`).
6.  Wire up `readPackets` loop to call Rust function.

## Verification Plan
### Automated
- Rust Unit Tests: Verify `etherparse` correctly identifies packets.

### Manual Verification
- **Install**: Run app, approve in System Settings.
- **Traffic**: Run `ping 1.1.1.1`.
- **Verify**: Check Console.app for logs from the Rust core ("Intercepted ICMP packet...").
