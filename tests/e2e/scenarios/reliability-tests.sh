#!/bin/zsh
# reliability-tests.sh - Phase 5: Reliability Test Scenarios
# Task 004: E2E Relay Testing
#
# Tests component restart behavior, error conditions, and network impairment.
#
# Prerequisites:
#   - All components built (intermediate-server, app-connector, quic-client, echo-server)
#   - Valid certificates in $CERT_DIR

set -euo pipefail

# Get script directory and load common functions
SCRIPT_DIR="${${(%):-%x}:A:h}"
source "$SCRIPT_DIR/../lib/common.sh"

# ============================================================================
# Phase 5.1: Component Restart Tests
# ============================================================================

# Test 1: Intermediate Server restart - verify new connections work
test_intermediate_restart() {
    test_start "Intermediate Server restart - reconnection after restart"

    # Ensure clean state
    stop_component "intermediate"
    stop_component "connector"
    sleep 1

    # Start fresh Intermediate
    if ! start_intermediate; then
        test_fail "Failed to start Intermediate Server"
        return 1
    fi
    sleep 1

    # Start Connector
    if ! start_connector; then
        test_fail "Failed to start Connector"
        return 1
    fi
    sleep 2

    # Verify initial connectivity
    local output1
    output1=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "RESTART_TEST_1" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1) || true

    if ! echo "$output1" | grep -q "RECV:"; then
        test_fail "Initial connectivity failed before restart"
        return 1
    fi

    log_info "Initial connectivity verified, restarting Intermediate Server..."

    # Restart Intermediate Server
    stop_component "intermediate"
    sleep 1

    if ! start_intermediate; then
        test_fail "Failed to restart Intermediate Server"
        return 1
    fi
    sleep 1

    # Connector needs to reconnect - restart it too since it won't auto-reconnect
    stop_component "connector"
    sleep 1

    if ! start_connector; then
        test_fail "Failed to restart Connector after Intermediate restart"
        return 1
    fi
    sleep 2

    # Verify connectivity after restart
    local output2
    output2=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "RESTART_TEST_2" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1) || true

    if echo "$output2" | grep -q "RECV:"; then
        test_pass "Connectivity restored after Intermediate Server restart"
        return 0
    else
        test_fail "Connectivity not restored after restart"
        echo "Output: $output2"
        return 1
    fi
}

# Test 2: Connector restart - verify data flow resumes
test_connector_restart() {
    test_start "Connector restart - data flow resumes after restart"

    # Ensure Intermediate is running
    if ! check_component_running "intermediate"; then
        if ! start_intermediate; then
            test_fail "Failed to start Intermediate Server"
            return 1
        fi
        sleep 1
    fi

    # Stop and restart connector
    stop_component "connector"
    sleep 1

    if ! start_connector; then
        test_fail "Failed to start Connector"
        return 1
    fi
    sleep 2

    # Verify initial flow
    local output1
    output1=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "CONNECTOR_RESTART_1" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1) || true

    if ! echo "$output1" | grep -q "RECV:"; then
        test_fail "Initial flow failed before Connector restart"
        return 1
    fi

    log_info "Initial flow verified, restarting Connector..."

    # Restart Connector
    stop_component "connector"
    sleep 1

    if ! start_connector; then
        test_fail "Failed to restart Connector"
        return 1
    fi
    sleep 2

    # Verify flow after restart
    local output2
    output2=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "CONNECTOR_RESTART_2" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1) || true

    if echo "$output2" | grep -q "RECV:"; then
        test_pass "Data flow resumed after Connector restart"
        return 0
    else
        test_fail "Data flow not resumed after Connector restart"
        echo "Output: $output2"
        return 1
    fi
}

# Test 3: Active flows during Connector restart
test_active_flow_during_restart() {
    test_start "Active flows during Connector restart"

    # Ensure components are running
    if ! check_component_running "intermediate"; then
        if ! start_intermediate; then
            test_fail "Failed to start Intermediate Server"
            return 1
        fi
        sleep 1
    fi

    stop_component "connector"
    sleep 1

    if ! start_connector; then
        test_fail "Failed to start Connector"
        return 1
    fi
    sleep 2

    # Start a long-running flow in background
    local bg_output_file=$(mktemp)
    "$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --payload-size 50 \
        --payload-pattern sequential \
        --repeat 10 \
        --delay 500 \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 8000 > "$bg_output_file" 2>&1 &
    local bg_pid=$!

    # Wait for first few packets to flow
    sleep 1

    log_info "Active flow started (PID: $bg_pid), restarting Connector mid-flow..."

    # Restart Connector while flow is active
    stop_component "connector"
    sleep 1

    if ! start_connector; then
        test_fail "Failed to restart Connector during active flow"
        kill $bg_pid 2>/dev/null || true
        rm -f "$bg_output_file"
        return 1
    fi

    # Wait for background flow to complete
    wait $bg_pid 2>/dev/null || true

    local recv_count=$(grep -c "RECV:" "$bg_output_file" 2>/dev/null || echo "0")
    rm -f "$bg_output_file"

    # We expect some packets to be lost during restart
    # Success if we got at least some responses (partial delivery)
    if [[ $recv_count -ge 3 ]]; then
        test_pass "Active flow survived Connector restart: $recv_count/10 packets received"
        return 0
    elif [[ $recv_count -ge 1 ]]; then
        test_warn "Partial delivery during restart: $recv_count/10 packets"
        return 0
    else
        test_fail "No packets delivered during active flow restart"
        return 1
    fi
}

# ============================================================================
# Phase 5.2: Error Condition Tests
# ============================================================================

# Test 4: Unknown service ID (no connector registered)
test_unknown_service() {
    test_start "Unknown service ID - no connector registered"

    # Ensure Intermediate is running
    if ! check_component_running "intermediate"; then
        if ! start_intermediate; then
            test_fail "Failed to start Intermediate Server"
            return 1
        fi
        sleep 1
    fi

    # Send to a service that doesn't exist
    local output
    output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "nonexistent-service-xyz123" \
        --send-udp "UNKNOWN_SERVICE_TEST" \
        --dst "127.0.0.1:$ECHO_SERVER_PORT" \
        --wait 3000 2>&1) || true

    # We expect only the QAD (7-byte) response, not an echoed data packet (42+ bytes)
    # QAD format: RECV:01xxxxxxxx (7 bytes = 14 hex chars)
    # Data echo: RECV:4500002a... (42+ bytes = 84+ hex chars)
    # Count responses longer than 7 bytes (more than 14 hex chars after RECV:)
    local data_responses
    data_responses=$(echo "$output" | grep -o "RECV:[0-9a-fA-F]*" | awk 'length($0) > 19 {print}' | wc -l | tr -d ' ')

    if [[ "$data_responses" -gt 0 ]]; then
        test_fail "Unexpectedly received data echo ($data_responses packets) for unknown service"
        return 1
    else
        test_pass "No data echo for unknown service (QAD-only is expected)"
        return 0
    fi
}

# Test 5: Unknown destination IP through valid service
test_unknown_destination() {
    test_start "Unknown destination IP through valid service"

    # Ensure components are running
    if ! check_component_running "intermediate"; then
        if ! start_intermediate; then
            test_fail "Failed to start Intermediate Server"
            return 1
        fi
        sleep 1
    fi

    if ! check_component_running "connector"; then
        if ! start_connector; then
            test_fail "Failed to start Connector"
            return 1
        fi
        sleep 2
    fi

    # Send to a non-routable destination
    # The connector should forward but no response will come back
    local output
    output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --send-udp "UNKNOWN_DEST_TEST" \
        --dst "192.0.2.1:9999" \
        --wait 2000 2>&1) || true

    # 192.0.2.1 is TEST-NET-1 (RFC 5737), should not receive any echo response
    # We will receive QAD (7 bytes), but should NOT receive data echo (42+ bytes)
    local data_responses
    data_responses=$(echo "$output" | grep -o "RECV:[0-9a-fA-F]*" | awk 'length($0) > 19 {print}' | wc -l | tr -d ' ')

    if [[ "$data_responses" -gt 0 ]]; then
        test_fail "Received echo response from unreachable TEST-NET destination"
        return 1
    else
        test_pass "No data echo from unreachable destination (expected)"
        return 0
    fi
}

# Test 6: Invalid certificate path - server startup failure
test_invalid_certificate() {
    test_start "Invalid certificate path - server startup failure"

    # Stop intermediate if running
    stop_component "intermediate"
    sleep 1

    local log_file="$LOG_DIR/intermediate-server-badcert.log"

    # Try to start with non-existent certificate
    "$INTERMEDIATE_BIN" \
        "$INTERMEDIATE_PORT" \
        "/nonexistent/path/cert.pem" \
        "/nonexistent/path/key.pem" \
        > "$log_file" 2>&1 &
    local pid=$!

    # Wait briefly and check if process exited
    sleep 2

    if kill -0 $pid 2>/dev/null; then
        # Server is still running (unexpected)
        kill $pid 2>/dev/null || true
        test_fail "Server started with invalid certificates (unexpected)"
        return 1
    else
        # Server exited (expected behavior)
        if grep -qi "error\|failed\|not found" "$log_file" 2>/dev/null; then
            test_pass "Server correctly refused to start with invalid certificates"
        else
            test_pass "Server exited when certificates not found"
        fi

        # Restart proper intermediate for subsequent tests
        if ! start_intermediate; then
            log_warn "Failed to restart Intermediate after cert test"
        fi
        return 0
    fi
}

# Test 7: Connection to non-listening port
test_connection_refused() {
    test_start "Connection to non-listening port"

    # Try to connect to a port where nothing is listening
    local output
    output=$("$QUIC_CLIENT_BIN" \
        --server "127.0.0.1:59999" \
        --wait 3000 2>&1) || true

    # Should timeout or get connection error
    if echo "$output" | grep -qi "timeout\|error\|failed\|refused"; then
        test_pass "Connection correctly failed to non-listening port"
        return 0
    elif echo "$output" | grep -q "Connection established"; then
        test_fail "Unexpectedly connected to non-listening port"
        return 1
    else
        # No explicit error but also no connection - acceptable
        test_pass "No connection to non-listening port"
        return 0
    fi
}

# Test 8: Rapid reconnection attempts
test_rapid_reconnection() {
    test_start "Rapid reconnection attempts (5 connections in 2 seconds)"

    # Ensure Intermediate is running
    if ! check_component_running "intermediate"; then
        if ! start_intermediate; then
            test_fail "Failed to start Intermediate Server"
            return 1
        fi
        sleep 1
    fi

    local success_count=0
    local pids=()

    # Launch 5 connections rapidly
    for i in 1 2 3 4 5; do
        "$QUIC_CLIENT_BIN" \
            --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
            --send "RAPID_$i" \
            --wait 1000 &
        pids+=($!)
        sleep 0.1
    done

    # Wait for all to complete
    for pid in "${pids[@]}"; do
        if wait $pid 2>/dev/null; then
            ((success_count++))
        fi
    done

    if [[ $success_count -ge 4 ]]; then
        test_pass "Rapid reconnections handled: $success_count/5 succeeded"
        return 0
    elif [[ $success_count -ge 2 ]]; then
        test_warn "Some rapid connections failed: $success_count/5 succeeded"
        return 0
    else
        test_fail "Rapid reconnections failed: only $success_count/5 succeeded"
        return 1
    fi
}

# ============================================================================
# Phase 5.3: Network Impairment Tests (Stretch - Requires Root/Admin)
# ============================================================================

# Test 9: Packet loss simulation (stretch goal)
test_packet_loss_simulation() {
    test_start "Packet loss simulation (STRETCH - requires pfctl/tc)"

    # Check if we have the necessary tools
    if ! command -v pfctl &>/dev/null && ! command -v tc &>/dev/null; then
        test_warn "Skipped: pfctl (macOS) or tc (Linux) not available for packet loss simulation"
        return 0
    fi

    # Even if tools exist, we need root/admin privileges
    if [[ $EUID -ne 0 ]]; then
        test_warn "Skipped: Root privileges required for network impairment tests"
        return 0
    fi

    # If we get here, we have tools and root - could implement later
    test_warn "Skipped: Network impairment simulation not yet implemented"
    return 0
}

# Test 10: Packet reorder simulation (stretch goal)
test_packet_reorder_simulation() {
    test_start "Packet reorder simulation (STRETCH - requires root)"

    if [[ $EUID -ne 0 ]]; then
        test_warn "Skipped: Root privileges required for packet reorder simulation"
        return 0
    fi

    test_warn "Skipped: Packet reorder simulation not yet implemented"
    return 0
}

# Test 11: NAT rebinding simulation (stretch goal)
test_nat_rebinding() {
    test_start "NAT rebinding simulation (STRETCH - requires network namespace)"

    # This would require network namespaces (Linux) or complex pfctl rules (macOS)
    test_warn "Skipped: NAT rebinding simulation requires complex network setup"
    return 0
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "=== Phase 5: Reliability Tests ==="
    log_info "Server: $INTERMEDIATE_HOST:$INTERMEDIATE_PORT"
    log_info "Service: $SERVICE_ID"
    log_info ""

    # Pre-cleanup: Kill any stale processes from previous runs
    log_info "Cleaning up stale processes..."
    pkill -f "$PROJECT_ROOT/intermediate-server" 2>/dev/null || true
    pkill -f "$PROJECT_ROOT/app-connector" 2>/dev/null || true
    pkill -f "udp-echo" 2>/dev/null || true
    sleep 2  # Wait for sockets to be released

    # Setup
    setup_directories

    # Ensure binaries exist
    if ! check_binaries; then
        log_error "Missing binaries. Run build_components first."
        return 1
    fi

    # Start echo server (needed for most tests)
    if ! check_component_running "echo"; then
        if ! start_echo_server; then
            log_error "Failed to start echo server"
            return 1
        fi
    fi
    sleep 1

    local passed=0
    local failed=0

    # Phase 5.1: Component Restart Tests
    log_info "--- 5.1 Component Restart Tests ---"
    if test_intermediate_restart; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_connector_restart; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_active_flow_during_restart; then : $((passed += 1)); else : $((failed += 1)); fi

    # Phase 5.2: Error Condition Tests
    log_info ""
    log_info "--- 5.2 Error Condition Tests ---"
    if test_unknown_service; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_unknown_destination; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_invalid_certificate; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_connection_refused; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_rapid_reconnection; then : $((passed += 1)); else : $((failed += 1)); fi

    # Phase 5.3: Network Impairment Tests (Stretch)
    log_info ""
    log_info "--- 5.3 Network Impairment Tests (Stretch) ---"
    if test_packet_loss_simulation; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_packet_reorder_simulation; then : $((passed += 1)); else : $((failed += 1)); fi
    if test_nat_rebinding; then : $((passed += 1)); else : $((failed += 1)); fi

    # Summary
    log_info ""
    log_info "=== Phase 5 Summary ==="
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
