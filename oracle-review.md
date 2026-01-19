# Phase 2 E2E Relay Testing Review

## Findings

### Medium
- `tests/e2e/scenarios/protocol-validation.sh:150-154` uses a hard-coded service id (`test-service`) while other tests use `$SERVICE_ID`. This can cause the suite to validate a different service than the configured env and mask misconfigurations. Use `$SERVICE_ID` or `$TEST_SERVICE_ID` consistently.
- `tests/e2e/scenarios/protocol-validation.sh:81-143` hard-codes datagram boundary sizes based on one empirical limit. This is brittle if QUIC overhead changes (CID length/MTU/quiche versions) and can cause false failures. Consider deriving payload size from `dgram_max_writable_len()` in the client (`tests/e2e/fixtures/quic-client/src/main.rs:123-143`) and subtracting 28 bytes for IP/UDP headers, or accept overrides via env.

### Low
- `tests/e2e/scenarios/protocol-validation.sh:95-107` only asserts "DATAGRAM queued" for the boundary test; it doesn't verify the relay path works or that the server accepted it. A regression that drops outbound datagrams post-queue would still pass. Consider asserting `RECV:` or a log-based confirmation.
- `tests/e2e/lib/common.sh:271-280` uses `pkill -f` for cleanup, which can terminate unrelated `intermediate-server` or `app-connector` processes on the same host. Prefer tracking and killing only known PIDs or scoping the match with `$PROJECT_ROOT`.
- `tests/e2e/lib/common.sh:299-313` uses `nc -z -u` for readiness checks, which doesn't reliably confirm UDP services are ready (no handshake). Consider a probe/echo check or a log-based readiness signal.
- `tasks/_context/testing-guide.md:188-191` lists `start_intermediate_server` and `start_app_connector`, but the actual functions are `start_intermediate` and `start_connector` (`tests/e2e/lib/common.sh:175-229`). This will trip new users.
- `tasks/_context/testing-guide.md:32-34` references certs under `intermediate-server/certs` while scripts default to `certs/` (`tests/e2e/lib/common.sh:30`). Both paths exist but the guide should clarify which is canonical for E2E tests.

## Coverage Gaps / Suggested Tests

- `tests/e2e/scenarios/protocol-validation.sh:22-246` covers agent registration only. Add connector registration (`0x11`) validation (valid + malformed length), and unknown opcode handling.
- `tests/e2e/scenarios/protocol-validation.sh:81-246` doesn't test malformed IP/UDP headers (bad checksum, non-UDP protocol, length mismatch). These are common relay hardening cases.
- Add a negative test for zero-length service id and for overlong service id (>255) to ensure consistent rejection behavior.
- Add a test for multiple back-to-back datagrams (or interleaved send/recv) to surface send-queue or flow-map issues.

## Answers To Specific Questions

1. `quiche::connect()` followed by a manual send/flush is the correct sans-IO pattern. In this client, `connect()` calls `flush()` immediately (`tests/e2e/fixtures/quic-client/src/main.rs:375-398`) and subsequent loops call `flush()` after processing, which matches quiche's expectations.
2. Yes, `: $((var += 1))` is a safe fix for `set -e` with zsh. `((var++))` returns the old value as the exit status and will trip errexit when it evaluates to 0.
3. Yes, programmatic sizing is preferable. quiche exposes `dgram_max_writable_len()` after handshake; you can use that to compute a safe payload size and reduce flakiness across versions/MTU. A good place to integrate is the `--payload-size` path in `tests/e2e/fixtures/quic-client/src/main.rs:123-143`.
4. Missing edge cases include connector registration (`0x11`) validation, malformed IP/UDP headers, zero-length or oversized service IDs, and unknown opcode handling. Those are likely to surface parsing or dispatch bugs before production.

## Open Questions / Assumptions

- Do you want the protocol validation suite to assert end-to-end delivery for boundary tests (i.e., require `RECV:`), or are these intended as "client-side acceptance" only?
- Should the scripts treat `intermediate-server/certs` or top-level `certs/` as the single source of truth, or is dual-path support intentional?
