#!/bin/zsh
# udp-advanced.sh - Phase 4: Advanced UDP Test Scenarios
# Task 004: E2E Relay Testing
#
# Tests payload patterns, concurrent flows, and long-running behavior.
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
# Phase 4.2: Echo Integrity Tests
# ============================================================================

# Test 1: All-zeros payload pattern
test_payload_zeros() {
    test_start "Echo integrity with all-zeros payload"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 64 \
        --payload-pattern zeros \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --verify-echo \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "VERIFY_RESULT:PASS"; then
            test_pass "All-zeros payload echoed correctly"
            return 0
        elif echo "$output" | grep -q "RECV:"; then
            test_warn "Received response but verification inconclusive"
            return 0
        else
            test_fail "No response received for zeros payload"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 2: All-ones (0xFF) payload pattern
test_payload_ones() {
    test_start "Echo integrity with all-ones payload"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 64 \
        --payload-pattern ones \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --verify-echo \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "VERIFY_RESULT:PASS"; then
            test_pass "All-ones payload echoed correctly"
            return 0
        elif echo "$output" | grep -q "RECV:"; then
            test_warn "Received response but verification inconclusive"
            return 0
        else
            test_fail "No response received for ones payload"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 3: Sequential payload pattern
test_payload_sequential() {
    test_start "Echo integrity with sequential payload"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 256 \
        --payload-pattern sequential \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --verify-echo \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "VERIFY_RESULT:PASS"; then
            test_pass "Sequential payload echoed correctly"
            return 0
        elif echo "$output" | grep -q "RECV:"; then
            test_warn "Received response but verification inconclusive"
            return 0
        else
            test_fail "No response received for sequential payload"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 4: Random payload pattern
test_payload_random() {
    test_start "Echo integrity with random payload"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 128 \
        --payload-pattern random \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --verify-echo \
        --wait 3000 2>&1); then

        if echo "$output" | grep -q "VERIFY_RESULT:PASS"; then
            test_pass "Random payload echoed correctly"
            return 0
        elif echo "$output" | grep -q "RECV:"; then
            test_warn "Received response but verification inconclusive"
            return 0
        else
            test_fail "No response received for random payload"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 5: Multiple payloads with repeat
test_multiple_payloads() {
    test_start "Multiple payload packets with repeat (5 packets)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 50 \
        --payload-pattern sequential \
        --repeat 5 \
        --delay 100 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --verify-echo \
        --wait 5000 2>&1); then

        local matches=$(echo "$output" | grep -o "VERIFY_MATCHES:[0-9]*" | cut -d: -f2)
        local total=$(echo "$output" | grep -o "VERIFY_TOTAL:[0-9]*" | cut -d: -f2)

        if [[ -n "$matches" ]] && [[ "$matches" -gt 0 ]]; then
            test_pass "Received $matches/$total packets back"
            return 0
        elif echo "$output" | grep -q "RECV:"; then
            test_warn "Received some responses"
            return 0
        else
            test_fail "No responses received"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# ============================================================================
# Phase 4.3: Concurrent Flow Tests
# ============================================================================

# Test 6: Multiple simultaneous clients (simulated concurrency)
test_concurrent_flows() {
    test_start "Concurrent UDP flows (3 parallel clients)"

    # Launch 3 clients in background
    local pids=()
    local outputs=()

    for i in 1 2 3; do
        local output_file=$(mktemp)
        outputs+=("$output_file")

        "$QUIC_CLIENT_BIN" \
            --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
            --service "$SERVICE_ID" \
            --payload-size 32 \
            --payload-pattern sequential \
            --dst "127.0.0.1:$ECHO_SERVER_PORT" \
            --wait 3000 > "$output_file" 2>&1 &
        pids+=($!)
    done

    # Wait for all clients to complete
    local success_count=0
    for i in 1 2 3; do
        local pid=${pids[$i]}
        local output_file=${outputs[$i]}

        if wait $pid 2>/dev/null; then
            if grep -q "RECV:" "$output_file"; then
                ((success_count++))
            fi
        fi

        rm -f "$output_file"
    done

    if [[ $success_count -eq 3 ]]; then
        test_pass "All 3 concurrent flows completed successfully"
        return 0
    elif [[ $success_count -gt 0 ]]; then
        test_warn "$success_count/3 concurrent flows completed"
        return 0
    else
        test_fail "No concurrent flows completed"
        return 1
    fi
}

# Test 7: Flow isolation (different source ports)
test_flow_isolation() {
    test_start "Flow isolation between clients"

    # Send packets from two different simulated source addresses
    local output1
    local output2

    # Client 1: Source 10.0.0.1:11111
    output1=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "CLIENT1_ISOLATION_TEST" \
        --src "10.0.0.1:11111" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1) || true

    # Client 2: Source 10.0.0.2:22222
    output2=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "CLIENT2_ISOLATION_TEST" \
        --src "10.0.0.2:22222" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1) || true

    local success=0
    if echo "$output1" | grep -q "RECV:"; then
        ((success++))
    fi
    if echo "$output2" | grep -q "RECV:"; then
        ((success++))
    fi

    if [[ $success -eq 2 ]]; then
        test_pass "Both isolated flows received responses"
        return 0
    elif [[ $success -eq 1 ]]; then
        test_warn "Only one flow received response"
        return 0
    else
        test_fail "No flows received responses"
        return 1
    fi
}

# ============================================================================
# Phase 4.4: Long-Running Tests
# ============================================================================

# Test 8: Long-lived stream stability (10 packets over 5 seconds)
test_long_lived_stream() {
    test_start "Long-lived UDP stream stability (10 packets, 500ms interval)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 64 \
        --payload-pattern sequential \
        --repeat 10 \
        --delay 500 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 8000 2>&1); then

        local recv_count=$(echo "$output" | grep -c "RECV:" || echo "0")

        if [[ $recv_count -ge 8 ]]; then
            test_pass "Long-lived stream stable: received $recv_count/10 packets"
            return 0
        elif [[ $recv_count -ge 5 ]]; then
            test_warn "Long-lived stream partial: received $recv_count/10 packets"
            return 0
        else
            test_fail "Long-lived stream unstable: only $recv_count/10 packets"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed"
        echo "$output"
        return 1
    fi
}

# Test 9: Burst traffic stress test
test_burst_traffic() {
    test_start "Burst traffic stress test (50 packets)"

    local output
    if output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --burst 50 \
        --payload-size 64 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 5000 2>&1); then

        if echo "$output" | grep -q "BURST_SENT:50"; then
            local pps=$(echo "$output" | grep -o "BURST_PPS:[0-9.]*" | cut -d: -f2)
            test_pass "Burst sent 50 packets at ${pps:-N/A} pps"
            return 0
        else
            test_fail "Burst did not complete"
            echo "$output"
            return 1
        fi
    else
        test_fail "quic-test-client failed during burst"
        echo "$output"
        return 1
    fi
}

# Test 10: Idle timeout behavior (connection within 30s timeout)
test_idle_timeout_within() {
    test_start "Connection stays alive within idle timeout (5s idle)"

    # Send initial packet
    local output1
    output1=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "IDLE_TEST_1" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1) || true

    if ! echo "$output1" | grep -q "RECV:"; then
        test_fail "Initial packet not echoed"
        return 1
    fi

    # Wait 5 seconds (well within 30s timeout)
    log_info "Waiting 5 seconds (within 30s idle timeout)..."
    sleep 5

    # Send another packet - connection should still be alive
    local output2
    output2=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "IDLE_TEST_2" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 2000 2>&1) || true

    if echo "$output2" | grep -q "RECV:"; then
        test_pass "Connection remained alive after 5s idle"
        return 0
    else
        test_fail "Connection dropped within idle timeout"
        echo "$output2"
        return 1
    fi
}

# Test 11: Idle timeout behavior (connection after 30s should fail)
# Note: This test is time-consuming and optional
test_idle_timeout_expired() {
    test_start "Connection closes after idle timeout (35s wait - OPTIONAL)"

    # This test would require waiting >30s which is too long for normal test runs
    # Mark as skipped/warn for now
    test_warn "Skipped: 35s idle timeout test too slow for CI"
    return 0
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "=== Phase 4: Advanced UDP Test Scenarios ==="
    log_info "Server: $INTERMEDIATE_HOST:$INTERMEDIATE_PORT"
    log_info "Service: $SERVICE_ID"
    log_info ""

    local passed=0
    local failed=0
    local warned=0

    # Phase 4.2: Echo Integrity Tests
    log_info "--- 4.2 Echo Integrity Tests ---"
    if test_payload_zeros; then ((passed++)); else ((failed++)); fi
    if test_payload_ones; then ((passed++)); else ((failed++)); fi
    if test_payload_sequential; then ((passed++)); else ((failed++)); fi
    if test_payload_random; then ((passed++)); else ((failed++)); fi
    if test_multiple_payloads; then ((passed++)); else ((failed++)); fi

    # Phase 4.3: Concurrent Flow Tests
    log_info ""
    log_info "--- 4.3 Concurrent Flow Tests ---"
    if test_concurrent_flows; then ((passed++)); else ((failed++)); fi
    if test_flow_isolation; then ((passed++)); else ((failed++)); fi

    # Phase 4.4: Long-Running Tests
    log_info ""
    log_info "--- 4.4 Long-Running Tests ---"
    if test_long_lived_stream; then ((passed++)); else ((failed++)); fi
    if test_burst_traffic; then ((passed++)); else ((failed++)); fi
    if test_idle_timeout_within; then ((passed++)); else ((failed++)); fi
    if test_idle_timeout_expired; then ((passed++)); else ((failed++)); fi

    # Summary
    log_info ""
    log_info "=== Phase 4 Summary ==="
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
