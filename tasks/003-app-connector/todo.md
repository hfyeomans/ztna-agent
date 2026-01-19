# TODO: App Connector

**Task ID:** 003-app-connector
**Branch:** `feature/003-app-connector`
**Depends On:** Task 002 (Intermediate Server)

---

## Prerequisites

- [ ] Task 002 (Intermediate Server) complete and merged
- [ ] Create feature branch: `git checkout -b feature/003-app-connector`

---

## Phase 1: Project Setup

- [ ] Create `app-connector/` crate
- [ ] Add dependencies (quiche, tokio, socket2, log, clap)
- [ ] Create CLI argument parsing
- [ ] Set up logging infrastructure

---

## Phase 2: QUIC Client

- [ ] Create quiche client configuration
- [ ] Implement UDP socket for QUIC transport
- [ ] Connect to Intermediate System
- [ ] Handle QUIC handshake
- [ ] Enable DATAGRAM support
- [ ] Log connection events

---

## Phase 3: Service Registration

- [ ] Define registration message format
- [ ] Send registration after handshake
- [ ] Include service_id in registration
- [ ] Handle registration confirmation

---

## Phase 4: QAD Handling

- [ ] Receive OBSERVED_ADDRESS from Intermediate
- [ ] Parse and store observed address
- [ ] Log observed address for debugging

---

## Phase 5: DATAGRAM Processing

- [ ] Receive DATAGRAMs from QUIC connection
- [ ] Parse encapsulated IP packet
- [ ] Extract protocol, ports, payload
- [ ] Determine forwarding destination

---

## Phase 6: Local Forwarding

- [ ] Implement UDP forwarding to local service
- [ ] Implement TCP connection to local service
- [ ] Handle ICMP (ping) if needed
- [ ] Capture response from local service

---

## Phase 7: Return Traffic

- [ ] Re-encapsulate response in IP packet format
- [ ] Send as QUIC DATAGRAM to Intermediate
- [ ] Handle flow control

---

## Phase 8: Testing

- [ ] Test with Intermediate (Task 002)
- [ ] Test with Agent (Task 001)
- [ ] Verify end-to-end ping works
- [ ] Test with HTTP service (curl through tunnel)

---

## Phase 9: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] Configuration file support
- [ ] Multiple service registration
- [ ] Health check for local service
- [ ] Metrics/statistics
