#!/bin/zsh
# common.sh - Shared functions for E2E test scripts
# Task 004: E2E Relay Testing

set -euo pipefail

# ============================================================================
# Configuration
# ============================================================================

# Get the directory of THIS script (works when sourced in zsh)
# ${(%):-%x} is the zsh way to get the current source file
_COMMON_SH_DIR="${${(%):-%x}:A:h}"
E2E_DIR="${_COMMON_SH_DIR:h}"
PROJECT_ROOT="${E2E_DIR:h:h}"

# Load environment config if exists
if [[ -f "$E2E_DIR/config/env.local" ]]; then
    source "$E2E_DIR/config/env.local"
fi

# Default configuration (can be overridden by env.local)
: "${INTERMEDIATE_HOST:=127.0.0.1}"
: "${INTERMEDIATE_PORT:=4433}"
: "${CONNECTOR_LOCAL_PORT:=8080}"
: "${ECHO_SERVER_PORT:=9999}"
: "${TEST_SERVICE_ID:=test-service}"
: "${LOG_DIR:=$E2E_DIR/artifacts/logs}"
: "${METRICS_DIR:=$E2E_DIR/artifacts/metrics}"
: "${CERT_DIR:=$PROJECT_ROOT/certs}"
: "${TIMEOUT_SECONDS:=30}"

# Component binaries (each crate has its own target directory)
INTERMEDIATE_BIN="$PROJECT_ROOT/intermediate-server/target/release/intermediate-server"
CONNECTOR_BIN="$PROJECT_ROOT/app-connector/target/release/app-connector"
ECHO_SERVER_BIN="$E2E_DIR/fixtures/echo-server/target/release/udp-echo"
QUIC_CLIENT_BIN="$E2E_DIR/fixtures/quic-client/target/release/quic-test-client"

# PID tracking (zsh associative array)
typeset -A COMPONENT_PIDS

# ============================================================================
# Logging
# ============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $(date '+%H:%M:%S') $*"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $(date '+%H:%M:%S') $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%H:%M:%S') $*"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $(date '+%H:%M:%S') $*"
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $(date '+%H:%M:%S') $*"
}

# ============================================================================
# Directory Setup
# ============================================================================

setup_directories() {
    mkdir -p "$LOG_DIR"
    mkdir -p "$METRICS_DIR"
    log_info "Created directories: $LOG_DIR, $METRICS_DIR"
}

# ============================================================================
# Build Components
# ============================================================================

build_components() {
    log_info "Building components..."

    log_info "Building intermediate-server..."
    if ! (cd "$PROJECT_ROOT/intermediate-server" && cargo build --release 2>&1); then
        log_error "Failed to build intermediate-server"
        return 1
    fi

    log_info "Building app-connector..."
    if ! (cd "$PROJECT_ROOT/app-connector" && cargo build --release 2>&1); then
        log_error "Failed to build app-connector"
        return 1
    fi

    log_info "Building echo-server..."
    if ! (cd "$E2E_DIR/fixtures/echo-server" && cargo build --release 2>&1); then
        log_error "Failed to build echo-server"
        return 1
    fi

    log_info "Building quic-test-client..."
    if ! (cd "$E2E_DIR/fixtures/quic-client" && cargo build --release 2>&1); then
        log_error "Failed to build quic-test-client"
        return 1
    fi

    log_success "Components built successfully"
}

check_binaries() {
    local missing=0

    if [[ ! -x "$INTERMEDIATE_BIN" ]]; then
        log_error "Missing: $INTERMEDIATE_BIN"
        missing=1
    fi

    if [[ ! -x "$CONNECTOR_BIN" ]]; then
        log_error "Missing: $CONNECTOR_BIN"
        missing=1
    fi

    if [[ ! -x "$ECHO_SERVER_BIN" ]]; then
        log_error "Missing: $ECHO_SERVER_BIN"
        missing=1
    fi

    if [[ ! -x "$QUIC_CLIENT_BIN" ]]; then
        log_error "Missing: $QUIC_CLIENT_BIN"
        missing=1
    fi

    if [[ $missing -eq 1 ]]; then
        log_info "Run 'build_components' or build each crate individually"
        return 1
    fi

    log_info "All binaries found"
    return 0
}

# ============================================================================
# Component Lifecycle
# ============================================================================

start_intermediate() {
    local cert="${1:-$CERT_DIR/cert.pem}"
    local key="${2:-$CERT_DIR/key.pem}"
    local log_file="$LOG_DIR/intermediate-server.log"

    log_info "Starting Intermediate Server on $INTERMEDIATE_HOST:$INTERMEDIATE_PORT..."

    if [[ ! -f "$cert" ]] || [[ ! -f "$key" ]]; then
        log_error "Certificates not found: $cert, $key"
        return 1
    fi

    # intermediate-server takes positional args: port cert_path key_path
    "$INTERMEDIATE_BIN" \
        "$INTERMEDIATE_PORT" \
        "$cert" \
        "$key" \
        > "$log_file" 2>&1 &

    COMPONENT_PIDS[intermediate]=${!}
    log_info "Intermediate Server started (PID: ${COMPONENT_PIDS[intermediate]})"

    # Wait for server to be ready
    sleep 1
    if ! kill -0 "${COMPONENT_PIDS[intermediate]}" 2>/dev/null; then
        log_error "Intermediate Server failed to start. Check $log_file"
        return 1
    fi

    return 0
}

start_connector() {
    local log_file="$LOG_DIR/app-connector.log"

    log_info "Starting App Connector (service: $TEST_SERVICE_ID)..."

    # app-connector takes flags: --server, --service, --forward
    "$CONNECTOR_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$TEST_SERVICE_ID" \
        --forward "$INTERMEDIATE_HOST:$ECHO_SERVER_PORT" \
        > "$log_file" 2>&1 &

    COMPONENT_PIDS[connector]=${!}
    log_info "App Connector started (PID: ${COMPONENT_PIDS[connector]})"

    # Wait for connector to connect
    sleep 2
    if ! kill -0 "${COMPONENT_PIDS[connector]}" 2>/dev/null; then
        log_error "App Connector failed to start. Check $log_file"
        return 1
    fi

    return 0
}

start_echo_server() {
    local port="${1:-$ECHO_SERVER_PORT}"
    local log_file="$LOG_DIR/echo-server.log"

    log_info "Starting UDP Echo Server on port $port..."

    # Use socat if available, otherwise use our custom echo server
    if command -v socat &>/dev/null; then
        socat -v UDP4-LISTEN:"$port",fork EXEC:'/bin/cat' > "$log_file" 2>&1 &
        COMPONENT_PIDS[echo]=${!}
    elif [[ -x "$ECHO_SERVER_BIN" ]]; then
        "$ECHO_SERVER_BIN" --port "$port" > "$log_file" 2>&1 &
        COMPONENT_PIDS[echo]=${!}
    else
        # Fallback: simple netcat-based echo (may not work on all systems)
        log_warn "socat not found, using nc (may have limited functionality)"
        while true; do nc -u -l "$port" | nc -u "$INTERMEDIATE_HOST" "$port"; done > "$log_file" 2>&1 &
        COMPONENT_PIDS[echo]=${!}
    fi

    log_info "Echo Server started (PID: ${COMPONENT_PIDS[echo]})"
    sleep 1
    return 0
}

stop_component() {
    local name="$1"

    if [[ -n "${COMPONENT_PIDS[$name]:-}" ]]; then
        local pid="${COMPONENT_PIDS[$name]}"
        if kill -0 "$pid" 2>/dev/null; then
            log_info "Stopping $name (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
        unset "COMPONENT_PIDS[$name]"
    fi
}

stop_all_components() {
    log_info "Stopping all components..."

    for name in ${(k)COMPONENT_PIDS}; do
        stop_component "$name"
    done

    # Also kill any orphaned processes
    pkill -f "intermediate-server" 2>/dev/null || true
    pkill -f "app-connector" 2>/dev/null || true

    log_info "All components stopped"
}

# ============================================================================
# Health Checks
# ============================================================================

check_component_running() {
    local name="$1"

    if [[ -z "${COMPONENT_PIDS[$name]:-}" ]]; then
        return 1
    fi

    kill -0 "${COMPONENT_PIDS[$name]}" 2>/dev/null
}

wait_for_port() {
    local host="$1"
    local port="$2"
    local timeout="${3:-10}"
    local elapsed=0

    log_info "Waiting for $host:$port (timeout: ${timeout}s)..."

    while [[ $elapsed -lt $timeout ]]; do
        if nc -z -u "$host" "$port" 2>/dev/null; then
            log_info "Port $host:$port is ready"
            return 0
        fi
        sleep 1
        : $((elapsed++))
    done

    log_error "Timeout waiting for $host:$port"
    return 1
}

# ============================================================================
# Test Helpers
# ============================================================================

send_udp() {
    local host="$1"
    local port="$2"
    local data="$3"
    local timeout="${4:-5}"

    echo -n "$data" | nc -u -w "$timeout" "$host" "$port"
}

send_udp_expect_response() {
    local host="$1"
    local port="$2"
    local data="$3"
    local expected="$4"
    local timeout="${5:-5}"

    local response
    response=$(echo -n "$data" | nc -u -w "$timeout" "$host" "$port" 2>/dev/null)

    if [[ "$response" == "$expected" ]]; then
        return 0
    else
        log_error "Expected: '$expected', Got: '$response'"
        return 1
    fi
}

generate_random_data() {
    local size="$1"
    head -c "$size" /dev/urandom | base64 | head -c "$size"
}

# ============================================================================
# QUIC Test Client Helpers
# ============================================================================

# Send data via QUIC DATAGRAM through the relay
# Returns: Output from quic-test-client including any RECV: lines
send_via_quic() {
    local data="$1"
    local server="${2:-$INTERMEDIATE_HOST:$INTERMEDIATE_PORT}"
    local wait_ms="${3:-2000}"
    local log_file="$LOG_DIR/quic-client.log"

    "$QUIC_CLIENT_BIN" \
        --server "$server" \
        --send "$data" \
        --wait "$wait_ms" \
        2>&1 | tee "$log_file"
}

# Send hex data via QUIC DATAGRAM
send_hex_via_quic() {
    local hex_data="$1"
    local server="${2:-$INTERMEDIATE_HOST:$INTERMEDIATE_PORT}"
    local wait_ms="${3:-2000}"
    local log_file="$LOG_DIR/quic-client.log"

    "$QUIC_CLIENT_BIN" \
        --server "$server" \
        --send-hex "$hex_data" \
        --wait "$wait_ms" \
        2>&1 | tee "$log_file"
}

# Send via QUIC and expect a specific response
# The QUIC client outputs RECV:<hex> for received DATAGRAMs
send_quic_expect_response() {
    local data="$1"
    local expected_hex="$2"
    local server="${3:-$INTERMEDIATE_HOST:$INTERMEDIATE_PORT}"
    local wait_ms="${4:-3000}"

    local output
    output=$(send_via_quic "$data" "$server" "$wait_ms" 2>/dev/null)

    if echo "$output" | grep -q "RECV:$expected_hex"; then
        return 0
    else
        log_error "Expected RECV:$expected_hex in output"
        log_error "Got: $output"
        return 1
    fi
}

# Convert string to hex for comparison
string_to_hex() {
    echo -n "$1" | xxd -p | tr -d '\n'
}

# Test QUIC connectivity (just connect, no data)
test_quic_connection() {
    local server="${1:-$INTERMEDIATE_HOST:$INTERMEDIATE_PORT}"
    local log_file="$LOG_DIR/quic-client.log"

    # Connect without sending data (uses --wait to stay connected briefly)
    "$QUIC_CLIENT_BIN" \
        --server "$server" \
        --wait 1000 \
        > "$log_file" 2>&1

    if grep -q "Connection established" "$log_file"; then
        return 0
    else
        log_error "QUIC connection failed. See $log_file"
        return 1
    fi
}

# ============================================================================
# Metrics
# ============================================================================

record_metric() {
    local test_name="$1"
    local metric_name="$2"
    local value="$3"
    local unit="${4:-}"

    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    local metrics_file="$METRICS_DIR/metrics.json"

    # Append as JSON line
    echo "{\"timestamp\":\"$timestamp\",\"test\":\"$test_name\",\"metric\":\"$metric_name\",\"value\":$value,\"unit\":\"$unit\"}" >> "$metrics_file"
}

measure_rtt() {
    local host="$1"
    local port="$2"
    local iterations="${3:-10}"

    local total=0
    local count=0

    for ((i=0; i<iterations; i++)); do
        local start end duration
        start=$(python3 -c 'import time; print(int(time.time() * 1000))')

        if echo "ping" | nc -u -w 1 "$host" "$port" >/dev/null 2>&1; then
            end=$(python3 -c 'import time; print(int(time.time() * 1000))')
            duration=$((end - start))
            total=$((total + duration))
            : $((count++))
        fi
    done

    if [[ $count -gt 0 ]]; then
        echo $((total / count))
    else
        echo "-1"
    fi
}

# ============================================================================
# Cleanup
# ============================================================================

cleanup() {
    log_info "Cleaning up..."
    stop_all_components

    # Remove stale PID files if any (use nullglob to handle no matches)
    setopt local_options nullglob
    rm -f "$E2E_DIR"/*.pid 2>/dev/null || true
}

# Register cleanup on exit
trap cleanup EXIT INT TERM

# ============================================================================
# Test Framework
# ============================================================================

TEST_COUNT=0
TEST_PASSED=0
TEST_FAILED=0

run_test() {
    local test_name="$1"
    local test_func="$2"

    : $((TEST_COUNT++))
    log_test "Running: $test_name"

    # Disable errexit for test execution
    set +e
    $test_func
    local result=$?
    set -e

    if [[ $result -eq 0 ]]; then
        : $((TEST_PASSED++))
        log_success "$test_name"
    else
        : $((TEST_FAILED++))
        log_error "$test_name"
    fi
    # Always return 0 to not exit on test failure
    return 0
}

print_test_summary() {
    echo ""
    echo "============================================"
    echo "Test Summary"
    echo "============================================"
    echo "Total:  $TEST_COUNT"
    echo -e "Passed: ${GREEN}$TEST_PASSED${NC}"
    echo -e "Failed: ${RED}$TEST_FAILED${NC}"
    echo "============================================"

    if [[ $TEST_FAILED -gt 0 ]]; then
        return 1
    fi
    return 0
}
