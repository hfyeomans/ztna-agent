#!/bin/zsh
# run-mvp.sh - Main orchestrator for E2E MVP tests
# Task 004: E2E Relay Testing
#
# Usage:
#   ./run-mvp.sh              # Run all MVP tests
#   ./run-mvp.sh --skip-build # Skip cargo build step
#   ./run-mvp.sh --keep       # Keep components running after tests
#   ./run-mvp.sh --scenario X # Run specific scenario only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${0:a}")" && pwd)"

# Source common functions
source "$SCRIPT_DIR/lib/common.sh"

# ============================================================================
# Parse Arguments
# ============================================================================

SKIP_BUILD=0
KEEP_RUNNING=0
SPECIFIC_SCENARIO=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --keep)
            KEEP_RUNNING=1
            shift
            ;;
        --scenario)
            SPECIFIC_SCENARIO="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --skip-build    Skip cargo build step"
            echo "  --keep          Keep components running after tests"
            echo "  --scenario X    Run specific scenario only"
            echo "  -h, --help      Show this help"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# ============================================================================
# Main
# ============================================================================

main() {
    echo ""
    echo "============================================"
    echo "  ZTNA E2E Relay Testing - MVP"
    echo "============================================"
    echo ""

    # Setup
    setup_directories

    # Build if needed
    if [[ $SKIP_BUILD -eq 0 ]]; then
        build_components
    else
        log_info "Skipping build (--skip-build)"
    fi

    # Check binaries exist
    if ! check_binaries; then
        log_error "Missing binaries. Run without --skip-build"
        exit 1
    fi

    # Check for certificates
    if [[ ! -f "$CERT_DIR/cert.pem" ]] || [[ ! -f "$CERT_DIR/key.pem" ]]; then
        log_warn "Certificates not found in $CERT_DIR"
        log_info "Generating self-signed certificates..."
        generate_test_certs
    fi

    # Start components
    log_info "Starting test infrastructure..."

    start_echo_server "$ECHO_SERVER_PORT"
    sleep 1

    start_intermediate "$CERT_DIR/cert.pem" "$CERT_DIR/key.pem"
    sleep 2

    start_connector
    sleep 2

    # Verify all components running
    if ! check_component_running "intermediate"; then
        log_error "Intermediate Server not running"
        cat "$LOG_DIR/intermediate-server.log" 2>/dev/null || true
        exit 1
    fi

    if ! check_component_running "connector"; then
        log_error "App Connector not running"
        cat "$LOG_DIR/app-connector.log" 2>/dev/null || true
        exit 1
    fi

    log_success "All components started"
    echo ""

    # Run test scenarios
    if [[ -n "$SPECIFIC_SCENARIO" ]]; then
        run_scenario "$SPECIFIC_SCENARIO"
    else
        run_all_scenarios
    fi

    # Print summary
    print_test_summary
    local result=$?

    # Cleanup unless --keep
    if [[ $KEEP_RUNNING -eq 1 ]]; then
        log_info "Keeping components running (--keep)"
        log_info "Intermediate: PID ${COMPONENT_PIDS[intermediate]:-N/A}"
        log_info "Connector: PID ${COMPONENT_PIDS[connector]:-N/A}"
        log_info "Echo Server: PID ${COMPONENT_PIDS[echo]:-N/A}"
        log_info "Run 'pkill -f intermediate-server && pkill -f app-connector' to stop"
        # Disable cleanup trap
        trap - EXIT
    fi

    return $result
}

# ============================================================================
# Certificate Generation
# ============================================================================

generate_test_certs() {
    mkdir -p "$CERT_DIR"

    openssl req -x509 -newkey rsa:2048 \
        -keyout "$CERT_DIR/key.pem" \
        -out "$CERT_DIR/cert.pem" \
        -days 365 -nodes \
        -subj "/CN=localhost" \
        2>/dev/null

    log_success "Generated test certificates in $CERT_DIR"
}

# ============================================================================
# Test Scenarios
# ============================================================================

run_all_scenarios() {
    log_info "Running all MVP test scenarios..."
    echo ""

    # Phase 2: Protocol validation (if script exists)
    if [[ -f "$SCRIPT_DIR/scenarios/protocol-validation.sh" ]]; then
        source "$SCRIPT_DIR/scenarios/protocol-validation.sh"
        run_protocol_validation_tests
    fi

    # Phase 3: Basic connectivity
    if [[ -f "$SCRIPT_DIR/scenarios/udp-connectivity.sh" ]]; then
        source "$SCRIPT_DIR/scenarios/udp-connectivity.sh"
        run_connectivity_tests
    fi

    # Phase 4: UDP echo tests
    if [[ -f "$SCRIPT_DIR/scenarios/udp-echo.sh" ]]; then
        source "$SCRIPT_DIR/scenarios/udp-echo.sh"
        run_echo_tests
    fi

    # Phase 4: Boundary tests
    if [[ -f "$SCRIPT_DIR/scenarios/udp-boundary.sh" ]]; then
        source "$SCRIPT_DIR/scenarios/udp-boundary.sh"
        run_boundary_tests
    fi

    # If no scenario scripts exist yet, run basic smoke test
    if [[ ! -f "$SCRIPT_DIR/scenarios/udp-echo.sh" ]]; then
        run_smoke_test
    fi
}

run_scenario() {
    local scenario="$1"
    local script="$SCRIPT_DIR/scenarios/${scenario}.sh"

    if [[ ! -f "$script" ]]; then
        log_error "Scenario not found: $script"
        exit 1
    fi

    log_info "Running scenario: $scenario"
    source "$script"

    case "$scenario" in
        protocol-validation)
            run_protocol_validation_tests
            ;;
        udp-connectivity)
            run_connectivity_tests
            ;;
        udp-echo)
            run_echo_tests
            ;;
        udp-boundary)
            run_boundary_tests
            ;;
        *)
            log_error "Unknown scenario: $scenario"
            exit 1
            ;;
    esac
}

# ============================================================================
# Smoke Test (Basic sanity check)
# ============================================================================

run_smoke_test() {
    log_info "Running smoke test..."

    # Test 1: Check intermediate server is listening
    run_test "Intermediate server is running" test_intermediate_running

    # Test 2: Check connector is running
    run_test "App Connector is running" test_connector_running

    # Test 3: Check echo server responds
    run_test "Echo server responds" test_echo_server
}

test_intermediate_running() {
    check_component_running "intermediate"
}

test_connector_running() {
    check_component_running "connector"
}

test_echo_server() {
    local response
    response=$(echo "hello" | nc -u -w 2 "$INTERMEDIATE_HOST" "$ECHO_SERVER_PORT" 2>/dev/null || true)

    # socat echo server returns what we send
    if [[ "$response" == "hello" ]]; then
        return 0
    fi

    # Some echo servers add newline
    if [[ "$response" == "hello"$'\n' ]]; then
        return 0
    fi

    # Check if we got any response
    if [[ -n "$response" ]]; then
        log_warn "Echo response: '$response' (expected 'hello')"
        return 0  # Accept any response for smoke test
    fi

    return 1
}

# ============================================================================
# Run Main
# ============================================================================

main "$@"
