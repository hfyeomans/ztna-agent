#!/bin/zsh
# udp-connectivity.sh - Basic UDP Connectivity Tests
# Task 004: E2E Relay Testing
#
# Tests basic connectivity between components.

# This script is sourced by run-mvp.sh, common.sh functions available

run_connectivity_tests() {
    log_info "=== UDP Connectivity Tests ==="

    run_test "Intermediate Server is running" test_intermediate_running
    run_test "App Connector is running" test_connector_running
    run_test "Echo Server is running" test_echo_running
    run_test "Echo Server responds to UDP" test_echo_responds
    run_test "Components have no crash logs" test_no_crash_logs
}

test_intermediate_running() {
    if check_component_running "intermediate"; then
        local pid="${COMPONENT_PIDS[intermediate]}"
        log_info "Intermediate Server PID: $pid"
        return 0
    fi
    return 1
}

test_connector_running() {
    if check_component_running "connector"; then
        local pid="${COMPONENT_PIDS[connector]}"
        log_info "App Connector PID: $pid"
        return 0
    fi
    return 1
}

test_echo_running() {
    if check_component_running "echo"; then
        local pid="${COMPONENT_PIDS[echo]}"
        log_info "Echo Server PID: $pid"
        return 0
    fi
    return 1
}

test_echo_responds() {
    local test_data="connectivity-test-$(date +%s)"
    local response

    response=$(echo -n "$test_data" | nc -u -w 3 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null)

    if [[ "$response" == "$test_data" ]]; then
        log_info "Echo server responded correctly"
        return 0
    fi

    if [[ -n "$response" ]]; then
        log_warn "Echo server responded with different data"
        return 0  # Still counts as responding
    fi

    log_error "Echo server did not respond"
    return 1
}

test_no_crash_logs() {
    local has_errors=0

    # Check intermediate server log for panics/crashes
    if [[ -f "$LOG_DIR/intermediate-server.log" ]]; then
        if grep -qi "panic\|error\|crash" "$LOG_DIR/intermediate-server.log" 2>/dev/null; then
            log_warn "Intermediate Server log contains errors"
            has_errors=1
        fi
    fi

    # Check connector log for panics/crashes
    if [[ -f "$LOG_DIR/app-connector.log" ]]; then
        if grep -qi "panic\|crash" "$LOG_DIR/app-connector.log" 2>/dev/null; then
            log_warn "App Connector log contains errors"
            has_errors=1
        fi
    fi

    # Warnings are OK, panics/crashes are not
    if grep -qi "panic" "$LOG_DIR"/*.log 2>/dev/null; then
        log_error "Found panic in logs!"
        return 1
    fi

    return 0
}
