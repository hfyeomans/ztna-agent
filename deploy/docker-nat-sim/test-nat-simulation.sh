#!/bin/bash
# test-nat-simulation.sh - Automated test script for NAT simulation environment
#
# This script:
# 1. Builds and starts all containers
# 2. Validates network connectivity
# 3. Tests P2P hole punching through NAT
# 4. Captures logs and reports results
#
# Usage:
#   ./test-nat-simulation.sh [--build] [--clean] [--debug] [--verbose]
#
# Options:
#   --build    Force rebuild of all images
#   --clean    Remove all containers and networks before starting
#   --debug    Start debug containers for manual inspection
#   --verbose  Show detailed output

set -euo pipefail

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Options
BUILD_IMAGES=false
CLEAN_FIRST=false
DEBUG_MODE=false
VERBOSE=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build)
            BUILD_IMAGES=true
            shift
            ;;
        --clean)
            CLEAN_FIRST=true
            shift
            ;;
        --debug)
            DEBUG_MODE=true
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--build] [--clean] [--debug] [--verbose]"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $*"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_step() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $*${NC}"
    echo -e "${BLUE}========================================${NC}"
}

# Test result tracking
TESTS_PASSED=0
TESTS_FAILED=0

test_passed() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    log_success "$1"
}

test_failed() {
    TESTS_FAILED=$((TESTS_FAILED + 1))
    log_error "$1"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    cd "${SCRIPT_DIR}"

    if [[ "${DEBUG_MODE}" == "true" ]]; then
        log_warn "Debug mode: containers left running for inspection"
        log_info "To stop: docker compose down"
        log_info "To view logs: docker compose logs -f"
    else
        docker compose down --volumes --remove-orphans 2>/dev/null || true
    fi
}

trap cleanup EXIT

# Wait for container to be healthy
wait_for_container() {
    local container=$1
    local timeout=${2:-60}
    local elapsed=0

    log_info "Waiting for ${container} to be ready..."

    while [[ $elapsed -lt $timeout ]]; do
        if docker inspect --format='{{.State.Running}}' "${container}" 2>/dev/null | grep -q true; then
            # Check if container has health check
            local health=$(docker inspect --format='{{.State.Health.Status}}' "${container}" 2>/dev/null || echo "none")
            if [[ "${health}" == "none" ]] || [[ "${health}" == "healthy" ]]; then
                return 0
            fi
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done

    log_error "Container ${container} failed to start within ${timeout}s"
    return 1
}

# Wait for network connectivity
wait_for_connectivity() {
    local container=$1
    local target=$2
    local port=$3
    local timeout=${4:-30}
    local elapsed=0

    log_info "Testing connectivity from ${container} to ${target}:${port}..."

    while [[ $elapsed -lt $timeout ]]; do
        if docker exec "${container}" nc -z -u -w2 "${target}" "${port}" 2>/dev/null; then
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done

    return 1
}

# Main test flow
main() {
    cd "${SCRIPT_DIR}"

    log_step "NAT Simulation Test Suite"
    log_info "Project root: ${PROJECT_ROOT}"
    log_info "Script dir: ${SCRIPT_DIR}"

    # Pre-flight checks
    log_step "Step 1: Pre-flight Checks"

    if ! command -v docker &>/dev/null; then
        log_error "Docker not found. Please install Docker."
        exit 1
    fi
    test_passed "Docker is installed"

    if ! docker compose version &>/dev/null; then
        log_error "Docker Compose not found. Please install Docker Compose."
        exit 1
    fi
    test_passed "Docker Compose is installed"

    if ! docker info &>/dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    test_passed "Docker daemon is running"

    # Check for required files
    if [[ ! -f "${PROJECT_ROOT}/certs/cert.pem" ]]; then
        log_error "Certificate not found at ${PROJECT_ROOT}/certs/cert.pem"
        exit 1
    fi
    test_passed "TLS certificates found"

    # Cleanup if requested
    if [[ "${CLEAN_FIRST}" == "true" ]]; then
        log_step "Step 2: Cleaning Previous Environment"
        docker compose down --volumes --remove-orphans 2>/dev/null || true
        docker network prune -f 2>/dev/null || true
        log_info "Previous environment cleaned"
    fi

    # Build images
    log_step "Step 2: Building Docker Images"

    if [[ "${BUILD_IMAGES}" == "true" ]] || ! docker images | grep -q ztna; then
        log_info "Building all images (this may take a few minutes)..."
        if [[ "${VERBOSE}" == "true" ]]; then
            docker compose build
        else
            docker compose build 2>&1 | tail -20
        fi
        test_passed "Docker images built successfully"
    else
        log_info "Using existing images (use --build to rebuild)"
        test_passed "Docker images available"
    fi

    # Start core services
    log_step "Step 3: Starting Core Services"

    log_info "Starting intermediate-server, NAT gateways, echo-server, app-connector..."
    docker compose up -d intermediate-server nat-agent nat-connector echo-server app-connector

    # Wait for services
    wait_for_container "ztna-intermediate" 60 || { test_failed "Intermediate server failed to start"; exit 1; }
    test_passed "Intermediate server started"

    wait_for_container "ztna-nat-agent" 30 || { test_failed "Agent NAT gateway failed to start"; exit 1; }
    test_passed "Agent NAT gateway started"

    wait_for_container "ztna-nat-connector" 30 || { test_failed "Connector NAT gateway failed to start"; exit 1; }
    test_passed "Connector NAT gateway started"

    wait_for_container "ztna-echo-server" 30 || { test_failed "Echo server failed to start"; exit 1; }
    test_passed "Echo server started"

    wait_for_container "ztna-app-connector" 60 || { test_failed "App connector failed to start"; exit 1; }
    test_passed "App connector started"

    # Brief pause for services to initialize
    sleep 5

    # Verify network topology
    log_step "Step 4: Verifying Network Topology"

    # Test: Intermediate server is reachable from public network
    log_info "Testing intermediate server accessibility..."
    if docker exec ztna-nat-agent ping -c 1 -W 2 172.20.0.10 &>/dev/null; then
        test_passed "Intermediate server (172.20.0.10) reachable from NAT gateway"
    else
        test_failed "Intermediate server not reachable"
    fi

    # Test: Echo server is reachable within connector LAN
    log_info "Testing echo server accessibility..."
    if docker exec ztna-app-connector ping -c 1 -W 2 172.22.0.20 &>/dev/null; then
        test_passed "Echo server (172.22.0.20) reachable from connector"
    else
        test_failed "Echo server not reachable from connector"
    fi

    # Check NAT rules are in place
    log_info "Verifying NAT rules on gateways..."
    if docker exec ztna-nat-agent iptables -t nat -L -n | grep -q MASQUERADE; then
        test_passed "Agent NAT MASQUERADE rule is active"
    else
        test_failed "Agent NAT rules not configured"
    fi

    if docker exec ztna-nat-connector iptables -t nat -L -n | grep -q MASQUERADE; then
        test_passed "Connector NAT MASQUERADE rule is active"
    else
        test_failed "Connector NAT rules not configured"
    fi

    # Verify App Connector connected to Intermediate Server
    log_step "Step 5: Verifying Component Connections"

    log_info "Checking app-connector logs for connection status..."
    sleep 3  # Give time for connection
    if docker logs ztna-app-connector 2>&1 | grep -qi "established\|connected\|registered"; then
        test_passed "App connector established connection to intermediate server"
    else
        log_warn "Could not confirm app-connector connection (checking logs...)"
        docker logs ztna-app-connector 2>&1 | tail -20
    fi

    # Run QUIC client test
    log_step "Step 6: P2P Hole Punching Test"

    log_info "Starting QUIC test client behind NAT..."

    # Run the client with test profile
    docker compose run --rm quic-client \
        --server 172.20.0.10:4433 \
        --service test-service \
        --send-udp "Hello from NAT simulation!" \
        --dst 172.22.0.20:9999 \
        --wait 10000 \
        2>&1 | tee /tmp/quic-client-output.txt

    # Check results
    if grep -q "RECV:" /tmp/quic-client-output.txt; then
        test_passed "QUIC client received response through NAT!"
        log_info "Response data:"
        grep "RECV:" /tmp/quic-client-output.txt
    else
        test_failed "QUIC client did not receive response"
        log_warn "This may indicate P2P hole punching failed or relay was used"
    fi

    # Check if echo server received the data
    log_info "Checking echo server logs..."
    if docker logs ztna-echo-server 2>&1 | grep -q "Received.*bytes"; then
        test_passed "Echo server received forwarded data"
    else
        log_warn "Echo server may not have received data yet"
    fi

    # RTT measurement test
    log_step "Step 7: Performance Test (RTT Measurement)"

    log_info "Measuring round-trip time through NAT..."
    docker compose run --rm quic-client \
        --server 172.20.0.10:4433 \
        --service test-service \
        --measure-rtt \
        --rtt-count 10 \
        --payload-size 64 \
        --dst 172.22.0.20:9999 \
        2>&1 | tee /tmp/quic-rtt-output.txt

    if grep -q "RTT_AVG_US:" /tmp/quic-rtt-output.txt; then
        test_passed "RTT measurement completed"
        log_info "RTT Statistics:"
        grep "RTT_" /tmp/quic-rtt-output.txt
    else
        test_failed "RTT measurement failed"
    fi

    # Collect logs if verbose
    if [[ "${VERBOSE}" == "true" ]]; then
        log_step "Container Logs"
        for container in ztna-intermediate ztna-app-connector ztna-echo-server; do
            echo ""
            echo "=== ${container} ==="
            docker logs --tail 50 "${container}" 2>&1
        done
    fi

    # Start debug containers if requested
    if [[ "${DEBUG_MODE}" == "true" ]]; then
        log_step "Debug Mode: Starting Debug Containers"
        docker compose --profile debug up -d
        log_info "Debug containers started:"
        log_info "  - ztna-debug-agent (172.21.0.100) on Agent LAN"
        log_info "  - ztna-debug-connector (172.22.0.100) on Connector LAN"
        log_info "  - ztna-debug-public (172.20.0.100) on Public network"
        log_info ""
        log_info "Connect with: docker exec -it ztna-debug-agent bash"
    fi

    # Summary
    log_step "Test Summary"

    echo ""
    echo "Tests Passed: ${TESTS_PASSED}"
    echo "Tests Failed: ${TESTS_FAILED}"
    echo ""

    if [[ ${TESTS_FAILED} -eq 0 ]]; then
        log_success "All tests passed!"
        exit 0
    else
        log_error "Some tests failed. Check logs above for details."
        exit 1
    fi
}

# Run main
main "$@"
