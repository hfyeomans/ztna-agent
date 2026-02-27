# Task 015: Oracle Quick Fixes — Research

**Description:** Research findings from Oracle Review 01 triage, identifying which findings are resolved, which are quick fixes, and which belong in larger tasks.

**Purpose:** Provide full context for each finding so implementation can proceed without re-investigating.

---

## Source

All findings originate from `oracle-review-01.md`, a Codex Oracle review (gpt-5.3-codex, xhigh reasoning) covering Tasks 001-005. The review was conducted pre-Task 007 (security hardening).

## Triage Methodology

1. Initial assessment by Claude against current codebase (post-Task 007)
2. Oracle verification (gpt-5.3-codex, xhigh) corrected several assessments
3. Final classification into: Fixed, Quick Fix (this task), Deferred (target tasks)

---

## Oracle-Verified Triage Results

### Confirmed Fixed (3 findings)

| # | Severity | Finding | Fixed By | Evidence |
|---|----------|---------|----------|----------|
| 1 | Critical | TLS cert verification disabled | Task 007 | `verify_peer(true)` default + CA cert loading. `intermediate-server/src/main.rs:351`, `app-connector/src/main.rs:560` |
| 4 | High | Stateless retry missing | Task 007 | AEAD retry tokens via `quiche::retry()`. `intermediate-server/src/main.rs:670-704` |
| 12 | Medium | Service ID length truncation | Task 007 | Bounds check before `u8` cast. `app-connector/src/main.rs:2087` |

### Conditionally Fixed (2 findings — deferred for hardening)

| # | Severity | Finding | Status | Target |
|---|----------|---------|--------|--------|
| 2 | Critical | No auth for service registration | Conditionally fixed — requires `--require-client-cert` flag; SAN-less certs still allowed for backward compat | Task 009 |
| 9 | Medium | DATAGRAM size mismatch (1350 vs ~1307) | Constants aligned at 1350 across all crates, but effective writable limit risk remains | Task 011 |

### Quick Fixes — This Task (4 findings)

#### Finding 6 (High): IPv6 QAD Panic — Remote DoS

- **Location:** `intermediate-server/src/qad.rs:51-53`
- **Current code:** `panic!("IPv6 QAD not yet implemented")`
- **Risk:** Any IPv6 client or dual-stack host crashes the server
- **Fix:** Replace `panic!()` with `log::warn!()` + return `Result::Err` (or empty Vec). Oracle recommends changing `build_observed_address()` to return `Option<Vec<u8>>` or `Result` rather than silently emitting empty datagrams.
- **Call-site:** `intermediate-server/src/main.rs:871` — `send_qad()` already handles errors

#### Finding 8 (Medium): Predictable P2P Identifiers

- **Location:** `core/packet_processor/src/p2p/signaling.rs:311-323` (`generate_session_id`)
- **Location:** `core/packet_processor/src/p2p/connectivity.rs:132-143` (`generate_transaction_id`)
- **Current code:** `SystemTime::now().as_nanos() ^ (pid << 32)` and timestamp bytes
- **Risk:** Predictable on same machine — time is observable, PID is enumerable. Enables session spoofing/replay.
- **Fix:** Replace with `ring::rand::SystemRandom` (already a dependency in `core/packet_processor/Cargo.toml`)
- **Note:** `ring = "0.17"` is already in Cargo.toml

#### Finding 11 (Medium): Legacy FFI Dead Code

- **Location:** `core/packet_processor/src/lib.rs:2118-2132` (`process_packet` function)
- **Location:** `core/packet_processor/src/lib.rs:2143-2146` (test `test_process_packet`)
- **Location:** `ios-macos/Shared/PacketProcessor-Bridging-Header.h:19` (C declaration)
- **Current code:** Always returns `PacketAction::Forward` regardless of input
- **Risk:** None functional, but dead code that misleads and violates project convention
- **Verification:** Grepped `ios-macos/` — no Swift callers exist. Only the bridging header declares it.
- **Fix:** Remove function, test, and bridging header declaration
- **Related docs to update:** `docs/architecture_design.md:73`, `docs/walkthrough.md:26`

#### Finding 15 (Low): UDP Length Sanity — Zero-Length Payload

- **Location:** `app-connector/src/main.rs:1265-1270`
- **Current code:** `let payload_len = udp_len.saturating_sub(8);` — when `udp_len < 8`, produces zero-length payload instead of dropping
- **Risk:** Malformed UDP packets (header claims < 8 bytes) are forwarded as empty payloads
- **Fix:** Add explicit check: if `udp_len < 8`, log warning and return early

### Deferred to Target Tasks (7 findings)

| # | Severity | Finding | Target | Why Not Quick Fix |
|---|----------|---------|--------|-------------------|
| 2 | Critical | Registration auth (conditional) | 009 | Needs policy decision on backward compat for SAN-less certs |
| 3 | High | Signaling session hijack | 009 | Oracle confirmed NOT fixed — `CandidateAnswer` accepted from any conn with matching session_id. Needs ownership/role validation architecture |
| 5 | High | Cross-tenant connector routing | 009 | "First flow wins" needs per-agent flow isolation — architectural change |
| 7 | High | Local UDP injection | 008 | Source addr validation needs design decision on what constitutes valid sources |
| 9 | Medium | DATAGRAM size mismatch | 011 | Effective limit investigation needed across QUIC negotiation |
| 10 | Medium | Interface enumeration endian bug | 011 | Oracle says `to_ne_bytes()` is likely **correct** here; original finding may be wrong. Needs deeper investigation, not a blind fix |
| 13 | Low | Hot-path per-packet allocations | 011 | Buffer reuse refactor across multiple hot paths, not one-liner |
| 14 | Low | Local socket recv buffer allocation | 008 | Small refactor to reuse `self.recv_buf`, needs borrow-scope work |

---

## Key Oracle Corrections

1. **Finding 3 (signaling hijack):** I initially marked as fixed. Oracle confirmed it is **NOT fixed** — sessions are keyed by session_id but `CandidateAnswer` is still accepted from any connection that supplies the matching session_id without ownership verification.

2. **Finding 10 (endian bug):** I initially marked as quick fix (`to_ne_bytes` → `to_be_bytes`). Oracle says **`to_ne_bytes()` is likely correct** because `sin_addr.s_addr` on macOS/BSD may already be in host byte order depending on the API path. Needs investigation, not a blind swap.

3. **Finding 9 (DATAGRAM mismatch):** I initially marked as fixed. Oracle notes constants are aligned but the **effective writable limit** (~1307 observed in tests) vs the 1350 constant still poses a boundary risk.

4. **Finding 15 (UDP length):** I initially marked as fixed. Oracle correctly identified that `saturating_sub(8)` on `udp_len < 8` produces a zero-length payload that gets forwarded, rather than being dropped as malformed.
