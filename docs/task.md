# Task: Zero Trust Network Access (ZTNA) with QUIC and Packet Interception

- [ ] Research Phase <!-- id: 0 -->
    - [/] Research macOS Network Extension framework (NEFilterPacketProvider, NEPacketTunnelProvider) for raw packet access. <!-- id: 1 -->
    - [x] Research macOS Network Extension framework (NEFilterPacketProvider, NEPacketTunnelProvider) for raw packet access. <!-- id: 1 -->
    - [x] Research QUIC libraries (Rust: `quinn`, `quiche`) and support for Address Discovery. <!-- id: 2 -->
    - [ ] Investigate existing open-source ZTNA/VPN projects for reference (e.g., WireGuard, nebula). <!-- id: 3 -->
- [x] Architecture Design <!-- id: 4 -->
    - [x] Draft High-Level Architecture Document (Agent, Intermediate, Connector). <!-- id: 5 -->
    - [x] detailed design for QUIC tunneling and NAT traversal strategies. <!-- id: 6 -->
- [ ] MVP Planning <!-- id: 7 -->
    - [x] Define "Hello World" scope for packet interception on macOS. <!-- id: 8 -->
    - [x] specific implementation plan for the macOS Endpoint Agent. <!-- id: 9 -->
- [x] MVP Implementation <!-- id: 10 -->
    - [x] Initialize project repository. <!-- id: 11 -->
    - [x] Implement macOS Network Extension for packet interception. <!-- id: 12 -->
    - [x] Demonstrate packet logging/dropping (Hello World). <!-- id: 13 -->
