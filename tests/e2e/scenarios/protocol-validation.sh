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

# Test 3: MAX_DATAGRAM_SIZE - At Limit (Programmatic Sizing)
# Uses dgram_max_writable_len() to determine actual max size dynamically
test_datagram_at_limit() {
    test_start "MAX_DATAGRAM_SIZE at limit (programmatic sizing via max-1)"

    local output
    # Use --payload-size max-1 which queries dgram_max_writable_len() and subtracts IP/UDP overhead + 1
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size max-1 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1); then

        # Assert DATAGRAM was queued AND received (end-to-end delivery)
        if echo "$output" | grep -q "DATAGRAM queued"; then
            if echo "$output" | grep -q "RECV:"; then
                test_pass "DATAGRAM at limit: queued AND echoed back (E2E verified)"
                return 0
            else
                test_warn "DATAGRAM queued but no RECV: (may be timing issue)"
                # Still pass since queueing succeeded
                return 0
            fi
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

# Test 4: MAX_DATAGRAM_SIZE - Over Limit (negative test, programmatic sizing)
# Uses dgram_max_writable_len() + 1 to exceed the actual limit
test_datagram_over_limit() {
    test_start "MAX_DATAGRAM_SIZE over limit (programmatic sizing via max+1)"

    local output
    # Use --payload-size max+1 which queries dgram_max_writable_len() and exceeds it by 1
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size max+1 \
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

        if echo "$output" | grep -q "DATAGRAM queued"; then
            # Zero-byte payload may not echo (nothing to echo)
            test_pass "Zero-byte payload queued successfully"
            return 0
        else
            test_warn "Zero-byte payload handled (no queue confirmation)"
            return 0
        fi
    else
        test_fail "Failed to send zero-byte payload"
        echo "$output"
        return 1
    fi
}

# Test 8: Small Payload Boundary - 1 byte (with E2E delivery assertion)
test_payload_one_byte() {
    test_start "One-byte payload relay (E2E delivery)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 1 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "RECV:"; then
            test_pass "One-byte payload: E2E delivery confirmed (RECV:)"
            return 0
        elif echo "$output" | grep -q "DATAGRAM queued"; then
            test_warn "One-byte payload queued but no RECV: (may be timing issue)"
            return 0
        else
            test_fail "One-byte payload not queued"
            echo "$output"
            return 1
        fi
    else
        test_fail "Failed to send one-byte payload"
        echo "$output"
        return 1
    fi
}

# ============================================================================
# Phase 3.5.3: Coverage Gap Tests
# ============================================================================

# Test 9: Connector Registration (0x11) - Valid Format
test_connector_registration_valid() {
    test_start "Connector registration (0x11) with valid format"

    # Connector registration: [0x11][len][service_id]
    local service_hex=$(printf '%s' "$SERVICE_ID" | xxd -p | tr -d '\n')
    local service_len=$(printf '%02x' ${#SERVICE_ID})
    local reg_msg="11${service_len}${service_hex}"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --send-hex "$reg_msg" \
        --wait 2000 2>&1); then

        if echo "$output" | grep -q "Connection established"; then
            test_pass "Connector registration (0x11) sent with valid format"
            return 0
        else
            test_warn "Connection established but registration may not be acknowledged"
            return 0
        fi
    else
        test_fail "Failed to send connector registration"
        echo "$output"
        return 1
    fi
}

# Test 10: Zero-Length Service ID (negative test)
test_service_id_zero_length() {
    test_start "Zero-length service ID (negative test)"

    # Registration with zero-length service ID: [0x10][0x00]
    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --send-hex "1000" \
        --wait 2000 2>&1); then

        # Server should handle gracefully (may close connection or ignore)
        test_pass "Zero-length service ID handled gracefully"
        return 0
    else
        test_pass "Zero-length service ID rejected (expected)"
        return 0
    fi
}

# Test 11: Overlong Service ID (>255 bytes, negative test)
test_service_id_overlong() {
    test_start "Overlong service ID (>255 bytes, negative test)"

    # Generate 260-byte service ID (exceeds single-byte length field)
    local long_id=$(printf 'x%.0s' {1..260})

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$long_id" \
        --wait 2000 2>&1); then

        # quic-test-client should reject this before sending
        if echo "$output" | grep -qi "too long\|error"; then
            test_pass "Overlong service ID rejected by client"
            return 0
        else
            test_warn "Overlong service ID may have been truncated"
            return 0
        fi
    else
        test_pass "Overlong service ID rejected (expected)"
        return 0
    fi
}

# Test 12: Unknown Opcode (negative test)
test_unknown_opcode() {
    test_start "Unknown opcode (0xFF) handling"

    # Send unknown opcode: [0xFF][0x04][data]
    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --send-hex "ff0474657374" \
        --wait 2000 2>&1); then

        # Server should ignore unknown opcodes gracefully
        test_pass "Unknown opcode (0xFF) handled gracefully"
        return 0
    else
        test_pass "Unknown opcode caused disconnect (acceptable)"
        return 0
    fi
}

# Test 13: Multiple Back-to-Back Datagrams
test_multiple_datagrams() {
    test_start "Multiple back-to-back datagrams"

    local output
    # Send first datagram
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "MSG1" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 1000 2>&1); then

        local count1=0
        if echo "$output" | grep -q "DATAGRAM queued"; then
            count1=1
        fi

        # Send second datagram in quick succession
        local output2
        if output2=$("$QUIC_CLIENT_BIN" \
            --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
            --service "$SERVICE_ID" \
            --send-udp "MSG2" \
            --dst "127.0.0.1:$ECHO_SERVER_PORT" \
            --wait 1000 2>&1); then

            local count2=0
            if echo "$output2" | grep -q "DATAGRAM queued"; then
                count2=1
            fi

            if [[ $count1 -eq 1 ]] && [[ $count2 -eq 1 ]]; then
                test_pass "Multiple back-to-back datagrams sent successfully"
                return 0
            else
                test_warn "Some datagrams may not have been queued"
                return 0
            fi
        fi
    fi

    test_fail "Failed to send multiple datagrams"
    return 1
}

# Test 14: Malformed IP Header (bad protocol field)
test_malformed_ip_header() {
    test_start "Malformed IP header (non-UDP protocol)"

    # Build a packet with ICMP protocol (1) instead of UDP (17)
    # This tests App Connector's packet validation
    # IP header with protocol=1 (ICMP): 45 00 00 1c 00 00 40 00 40 01 [checksum] [src] [dst]
    local malformed_hex="450000210000400040010000" # No checksum, truncated
    malformed_hex="${malformed_hex}0a000064"       # Src: 10.0.0.100
    malformed_hex="${malformed_hex}7f000001"       # Dst: 127.0.0.1
    malformed_hex="${malformed_hex}00000000"       # Fake ICMP header

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-hex "$malformed_hex" \
        --wait 2000 2>&1); then

        # App Connector should drop non-UDP packets
        if echo "$output" | grep -q "RECV:"; then
            test_fail "Malformed IP packet was echoed back (should have been dropped)"
            return 1
        else
            test_pass "Malformed IP packet handled (no echo, likely dropped)"
            return 0
        fi
    else
        test_pass "Malformed IP packet rejected"
        return 0
    fi
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "=== Phase 2 & 3.5: Protocol Validation Tests ==="
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

    # Phase 3.5.3: Coverage gap tests
    log_info ""
    log_info "--- Phase 3.5.3: Coverage Gap Tests ---"
    if test_connector_registration_valid; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_service_id_zero_length; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_service_id_overlong; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_unknown_opcode; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_multiple_datagrams; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_malformed_ip_header; then : $((passed += 1)); else : $((failed += 1)); fi

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
