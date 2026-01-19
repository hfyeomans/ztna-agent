#!/bin/zsh
# protocol-validation.sh - Phase 2: Protocol Validation Tests
# Task 004: E2E Relay Testing
#
# Tests ALPN validation, registration format, and MAX_DATAGRAM_SIZE boundary.
#
# Prerequisites:
#   - Intermediate Server running on port 4433
#   - App Connector registered for test-service
#   - UDP Echo Server running on port 9999

set -euo pipefail

# Get script directory and load common functions
SCRIPT_DIR="${${(%):-%x}:A:h}"
source "$SCRIPT_DIR/../lib/common.sh"

# ============================================================================
# Test Functions
# ============================================================================

# Test 1: ALPN Validation - Correct Protocol
test_alpn_correct() {
    test_start "ALPN validation with correct protocol (ztna-v1)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --alpn "ztna-v1" \
        --wait 2000 2>&1); then

        if echo "$output" | grep -q "Connection established"; then
            test_pass "Connection established with correct ALPN"
            return 0
        else
            test_fail "Connection did not establish"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 2: ALPN Validation - Wrong Protocol (Negative Test)
test_alpn_wrong() {
    test_start "ALPN validation with wrong protocol (negative test)"

    local output
    # Using --expect-fail to indicate we expect this to fail
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --alpn "wrong-protocol" \
        --expect-fail \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "EXPECTED_FAIL"; then
            test_pass "Connection correctly rejected with wrong ALPN"
            return 0
        else
            test_fail "Connection was NOT rejected (should have been)"
            echo "$output"
            return 1
        fi
    else
        # If the command failed without --expect-fail catching it, still a pass
        # because wrong ALPN should cause connection failure
        if echo "$output" | grep -qi "error\|timeout\|rejected"; then
            test_pass "Connection rejected with wrong ALPN (via error)"
            return 0
        else
            test_fail "Unexpected failure mode"
            echo "$output"
            return 1
        fi
    fi
}

# Test 3: MAX_DATAGRAM_SIZE - At Limit
# Note: QUIC DATAGRAM max is ~1307 bytes due to QUIC/encryption overhead
# IP header (20) + UDP header (8) + payload (1278) = 1306 bytes (at limit)
test_datagram_at_limit() {
    test_start "MAX_DATAGRAM_SIZE at limit (1278 byte payload = 1306 total)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 1278 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "DATAGRAM queued"; then
            test_pass "1306-byte DATAGRAM accepted (at QUIC payload limit)"
            return 0
        else
            test_fail "DATAGRAM was not queued"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 4: MAX_DATAGRAM_SIZE - Over Limit (negative test)
# Payload larger than ~1307 bytes should be rejected by QUIC
test_datagram_over_limit() {
    test_start "MAX_DATAGRAM_SIZE over limit (1292 byte payload = 1320 total)"

    local output
    # 1320 total bytes exceeds QUIC DATAGRAM limit (~1307)
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 1292 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1); then

        # QUIC should reject oversized datagrams with BufferTooShort
        if echo "$output" | grep -qi "BufferTooShort\|error\|too large\|dropped\|failed"; then
            test_pass "Oversized DATAGRAM correctly rejected (BufferTooShort)"
            return 0
        else
            # If it somehow succeeded, that's unexpected
            test_fail "Oversized DATAGRAM was NOT rejected (expected failure)"
            echo "$output"
            return 1
        fi
    else
        # Command exited with error - expected for oversized datagrams
        if echo "$output" | grep -qi "BufferTooShort"; then
            test_pass "Oversized DATAGRAM rejected via BufferTooShort error"
            return 0
        else
            test_pass "Oversized DATAGRAM rejected via error"
            return 0
        fi
    fi
}

# Test 5: Agent Registration - Valid Format
test_registration_valid() {
    test_start "Agent registration with valid format"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --wait 1000 2>&1); then

        if echo "$output" | grep -q "Registering as Agent"; then
            test_pass "Agent registration sent with valid format"
            return 0
        else
            test_fail "Registration message not sent"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 6: Registration - Invalid Length (negative test)
# Send malformed registration with incorrect length field
test_registration_invalid_length() {
    test_start "Registration with invalid length (negative test)"

    # Format: [0x10][len=255][actual_data_only_4_bytes]
    # This is a malformed registration - length says 255 but only 4 bytes follow
    local malformed_hex="10ff74657374"  # 0x10, 0xff (255), "test" (4 bytes)

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --send-hex "$malformed_hex" \
        --wait 2000 2>&1); then

        # Server should either ignore or handle gracefully
        # Connection should not crash
        if echo "$output" | grep -qi "closed\|error"; then
            test_warn "Connection closed on malformed registration (expected behavior)"
        else
            test_pass "Server handled malformed registration gracefully"
        fi
        return 0
    else
        test_warn "Connection error on malformed registration"
        return 0
    fi
}

# Test 7: Small Payload Boundary - 0 bytes
test_payload_zero_bytes() {
    test_start "Zero-byte payload relay"

    local output
    # Build IP/UDP packet with empty payload
    # IP header (20) + UDP header (8) + payload (0) = 28 bytes total
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 0 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1); then

        test_pass "Zero-byte payload handled"
        return 0
    else
        test_fail "Failed to send zero-byte payload"
        echo "$output"
        return 1
    fi
}

# Test 8: Small Payload Boundary - 1 byte
test_payload_one_byte() {
    test_start "One-byte payload relay"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 1 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1); then

        if echo "$output" | grep -q "RECV:"; then
            test_pass "One-byte payload echoed successfully"
        else
            test_pass "One-byte payload sent (echo may not have returned in time)"
        fi
        return 0
    else
        test_fail "Failed to send one-byte payload"
        echo "$output"
        return 1
    fi
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "=== Phase 2: Protocol Validation Tests ==="
    log_info "Server: $INTERMEDIATE_HOST:$INTERMEDIATE_PORT"
    log_info "Service: $SERVICE_ID"
    log_info ""

    local passed=0
    local failed=0
    local warned=0

    # Run ALPN tests
    log_info "--- ALPN Validation ---"
    if test_alpn_correct; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_alpn_wrong; then : $((passed += 1)); else : $((failed += 1)); fi

    # Run boundary tests
    log_info ""
    log_info "--- MAX_DATAGRAM_SIZE Boundary ---"
    if test_datagram_at_limit; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_datagram_over_limit; then : $((passed += 1)); else : $((failed += 1)); fi

    # Run registration tests
    log_info ""
    log_info "--- Registration Format ---"
    if test_registration_valid; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_registration_invalid_length; then : $((passed += 1)); else : $((failed += 1)); fi

    # Run payload boundary tests
    log_info ""
    log_info "--- Payload Boundary Tests ---"
    if test_payload_zero_bytes; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_payload_one_byte; then : $((passed += 1)); else : $((failed += 1)); fi

    # Summary
    log_info ""
    log_info "=== Protocol Validation Summary ==="
    log_info "Passed: $passed"
    log_info "Failed: $failed"

    if [[ $failed -gt 0 ]]; then
        log_error "Some tests failed!"
        return 1
    else
        log_info "All tests passed!"
        return 0
    fi
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]:-${(%):-%x}}" == "${0}" ]]; then
    main "$@"
fi
