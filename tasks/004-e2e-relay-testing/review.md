# Review Notes: Phase 1.5 E2E Relay Testing

Date: 2026-01-19

Focus:
- App Connector QUIC handshake flush and local socket polling fixes
- QUIC test client IP/UDP packet construction
- Protocol correctness and test coverage gaps

Key risks:
- Return traffic uses first flow entry only; concurrent flows can be misrouted.
- IPv4 header validation accepts IHL < 5 and ignores total length checks.
- QUIC send path drops packets on UDP WouldBlock without retry buffering.

Phase 2 recommendations:
- Add ALPN mismatch, DATAGRAM boundary, and malformed IP/UDP tests.
- Add multi-flow routing test to validate correct flow selection.
