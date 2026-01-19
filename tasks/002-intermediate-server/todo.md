# TODO: Intermediate Server

**Task ID:** 002-intermediate-server
**Branch:** `feature/002-intermediate-server`

---

## Setup

- [ ] Create feature branch: `git checkout -b feature/002-intermediate-server`
- [ ] Create `intermediate-server/` crate
- [ ] Add dependencies (quiche, tokio, mio, ring, log)
- [ ] Generate development certificates

---

## Phase 1: Basic QUIC Server

- [ ] Create main.rs with tokio runtime
- [ ] Bind UDP socket to 0.0.0.0:4433
- [ ] Create quiche::Config for server
- [ ] Enable DATAGRAM support in config
- [ ] Implement basic packet receive loop
- [ ] Handle QUIC handshake
- [ ] Log connection events

---

## Phase 2: QAD (QUIC Address Discovery)

- [ ] Extract source IP:Port from UDP packets
- [ ] Store observed address per connection
- [ ] Define OBSERVED_ADDRESS message format
- [ ] Send QAD message after handshake
- [ ] Test: Agent logs its observed address

---

## Phase 3: Client Registry

- [ ] Define ClientType enum (Agent, Connector)
- [ ] Create Client struct with connection state
- [ ] Implement client registration on connect
- [ ] Implement client cleanup on disconnect
- [ ] Add routing table for destination lookup

---

## Phase 4: DATAGRAM Relay

- [ ] Receive DATAGRAMs from connections
- [ ] Parse routing header
- [ ] Look up destination connection
- [ ] Forward DATAGRAM to destination
- [ ] Handle missing destination gracefully

---

## Phase 5: Integration Testing

- [ ] Test with Agent from Task 001
- [ ] Verify Agent connects successfully
- [ ] Verify Agent receives QAD message
- [ ] Test multiple concurrent connections
- [ ] Test clean disconnect/reconnect

---

## Phase 6: PR & Merge

- [ ] Update state.md with completion status
- [ ] Update `_context/components.md` status
- [ ] Push branch to origin
- [ ] Create PR for review
- [ ] Address review feedback
- [ ] Merge to master

---

## Stretch Goals (Optional)

- [ ] Add CLI for port configuration
- [ ] Add health check endpoint
- [ ] Add metrics/logging improvements
- [ ] Add connection timeout handling
