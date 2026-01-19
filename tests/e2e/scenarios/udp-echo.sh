#!/bin/zsh
# udp-echo.sh - UDP Echo Tests
# Task 004: E2E Relay Testing
#
# Tests basic UDP echo functionality through the relay.

# This script is sourced by run-mvp.sh, common.sh functions available

run_echo_tests() {
    log_info "=== UDP Echo Tests ==="

    run_test "Echo small payload (10 bytes)" test_echo_small
    run_test "Echo medium payload (100 bytes)" test_echo_medium
    run_test "Echo payload with pattern" test_echo_pattern
    run_test "Echo multiple packets" test_echo_multiple
}

test_echo_small() {
    local payload="HelloWorld"  # 10 bytes
    local response

    response=$(echo -n "$payload" | nc -u -w 2 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null)

    if [[ "$response" == "$payload" ]]; then
        return 0
    fi

    log_error "Expected: '$payload', Got: '$response'"
    return 1
}

test_echo_medium() {
    local payload
    payload=$(head -c 100 /dev/urandom | base64 | head -c 100)
    local response

    response=$(echo -n "$payload" | nc -u -w 2 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null)

    if [[ "$response" == "$payload" ]]; then
        return 0
    fi

    log_error "Response length: ${#response}, Expected: ${#payload}"
    return 1
}

test_echo_pattern() {
    # Test with a recognizable pattern
    local payload="ABCDEFGHIJ0123456789abcdefghij"
    local response

    response=$(echo -n "$payload" | nc -u -w 2 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null)

    if [[ "$response" == "$payload" ]]; then
        return 0
    fi

    log_error "Pattern mismatch"
    return 1
}

test_echo_multiple() {
    local success=0
    local total=5

    for i in $(seq 1 $total); do
        local payload="packet-$i-$(date +%s%N)"
        local response

        response=$(echo -n "$payload" | nc -u -w 1 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null)

        if [[ "$response" == "$payload" ]]; then
            ((success++))
        fi
    done

    if [[ $success -eq $total ]]; then
        return 0
    fi

    log_error "Only $success/$total packets echoed successfully"
    return 1
}
