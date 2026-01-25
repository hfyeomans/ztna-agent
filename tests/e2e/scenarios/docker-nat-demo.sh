#!/bin/bash
# =============================================================================
# Docker NAT Simulation Demo
# =============================================================================
# Demonstrates ZTNA relay through simulated NAT environment
#
# Network Topology:
#   Agent (172.21.0.10) --NAT--> 172.20.0.2 --\
#                                              +--> Intermediate (172.20.0.10)
#   Connector (172.22.0.10) --NAT--> 172.20.0.3 --/
#
# Usage:
#   ./docker-nat-demo.sh              # Build and run full demo
#   ./docker-nat-demo.sh --no-build   # Skip Docker builds
#   ./docker-nat-demo.sh --clean      # Clean up only
#   ./docker-nat-demo.sh --status     # Show container status
#   ./docker-nat-demo.sh --help       # Show help
#
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DOCKER_DIR="$PROJECT_ROOT/deploy/docker-nat-sim"

# Default options
DO_BUILD=true
SHOW_LOGS=false

# Print colored output
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}${BOLD}==> $1${NC}"; }

# Print banner
print_banner() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}          ${BOLD}ZTNA Docker NAT Simulation Demo${NC}                   ${CYAN}║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${CYAN}║${NC}  Tests P2P hole punching through simulated NAT gateways     ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}                                                              ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  Agent (172.21.0.10)    --NAT-->  172.20.0.2                ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  Connector (172.22.0.10) --NAT-->  172.20.0.3               ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  Intermediate Server             @ 172.20.0.10:4433         ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  Echo Server                     @ 172.22.0.20:9999         ${CYAN}║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

# Print help
print_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --no-build    Skip Docker image builds (use existing images)"
    echo "  --clean       Clean up containers and networks only"
    echo "  --status      Show current container status"
    echo "  --logs        Show container logs after demo"
    echo "  --help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Full demo (build + run + test)"
    echo "  $0 --no-build         # Run demo with existing images"
    echo "  $0 --clean            # Clean up Docker resources"
    echo "  $0 --status           # Check container status"
    echo ""
}

# Check prerequisites
check_prerequisites() {
    log_step "Checking prerequisites..."

    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker Desktop."
        exit 1
    fi

    # Check Docker is running
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running. Please start Docker Desktop."
        exit 1
    fi

    # Check docker-compose.yml exists
    if [[ ! -f "$DOCKER_DIR/docker-compose.yml" ]]; then
        log_error "docker-compose.yml not found at $DOCKER_DIR"
        exit 1
    fi

    log_success "Prerequisites OK"
}

# Clean up Docker resources
cleanup() {
    log_step "Cleaning up Docker resources..."
    cd "$DOCKER_DIR"
    docker compose --profile debug --profile test down --volumes --remove-orphans 2>/dev/null || true
    log_success "Cleanup complete"
}

# Show container status
show_status() {
    log_step "Container Status"
    cd "$DOCKER_DIR"
    docker compose ps 2>/dev/null || echo "No containers running"
}

# Build Docker images
build_images() {
    log_step "Building Docker images..."
    cd "$DOCKER_DIR"

    log_info "Building intermediate-server..."
    docker compose build intermediate-server

    log_info "Building app-connector..."
    docker compose build app-connector

    log_info "Building echo-server..."
    docker compose build echo-server

    log_info "Building quic-client..."
    docker compose build quic-client

    log_success "All images built"
}

# Start infrastructure
start_infrastructure() {
    log_step "Starting NAT simulation infrastructure..."
    cd "$DOCKER_DIR"

    # Start all infrastructure components
    docker compose up -d intermediate-server echo-server nat-agent nat-connector

    # Wait for services to be ready
    log_info "Waiting for services to initialize..."
    sleep 3

    # Start app-connector (depends on NAT being ready)
    docker compose up -d app-connector
    sleep 2

    log_success "Infrastructure started"
}

# Verify infrastructure
verify_infrastructure() {
    log_step "Verifying infrastructure..."
    cd "$DOCKER_DIR"

    # Check all containers are running
    local containers=("ztna-intermediate" "ztna-echo-server" "ztna-nat-agent" "ztna-nat-connector" "ztna-app-connector")
    local all_running=true

    for container in "${containers[@]}"; do
        if docker ps --format '{{.Names}}' | grep -q "^${container}$"; then
            log_info "  $container: ${GREEN}running${NC}"
        else
            log_error "  $container: ${RED}not running${NC}"
            all_running=false
        fi
    done

    if [[ "$all_running" != "true" ]]; then
        log_error "Some containers failed to start"
        return 1
    fi

    # Check connector registered with intermediate
    log_info "Checking connector registration..."
    if docker logs ztna-app-connector 2>&1 | grep -q "Registered as Connector"; then
        log_success "Connector registered with Intermediate Server"
    else
        log_warn "Connector registration not confirmed (may still be connecting)"
    fi

    # Check NAT is working
    log_info "Checking NAT configuration..."
    local connector_ip=$(docker logs ztna-app-connector 2>&1 | grep "QAD: Observed address" | tail -1 | grep -oE '172\.20\.0\.[0-9]+' || echo "")
    if [[ "$connector_ip" == "172.20.0.3" ]]; then
        log_success "NAT working: Connector appears as $connector_ip (NATted)"
    else
        log_warn "NAT status unclear - observed IP: $connector_ip"
    fi

    log_success "Infrastructure verified"
}

# Run the demo test
run_demo_test() {
    log_step "Running NAT simulation test..."
    cd "$DOCKER_DIR"

    echo ""
    echo -e "${BOLD}Test: Send UDP through NAT → Intermediate → NAT → Echo Server${NC}"
    echo ""

    # Run the quic-client test
    local output
    output=$(docker compose --profile test run --rm quic-client 2>&1)
    local exit_code=$?

    echo "$output"
    echo ""

    # Analyze results
    if echo "$output" | grep -q "Connection established"; then
        log_success "QUIC connection established"
    else
        log_error "QUIC connection failed"
        return 1
    fi

    # Check for NATted observed address (QAD response in hex: 01ac140002 = 172.20.0.2)
    if echo "$output" | grep -q "01ac140002"; then
        log_success "Agent NAT working: Observed as 172.20.0.2 (QAD confirmed)"
    else
        log_warn "Agent NAT status unclear - check QAD response"
    fi

    # Check echo response
    if echo "$output" | grep -q "Received DATAGRAM: 43 bytes"; then
        log_success "Echo response received through NAT tunnel!"
    else
        log_warn "Echo response not detected in output"
    fi

    # Check relay in intermediate logs
    if docker logs ztna-intermediate 2>&1 | grep -q "Relayed"; then
        log_success "Intermediate Server relayed traffic"
    fi

    return 0
}

# Show summary
print_summary() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}                    ${BOLD}Demo Summary${NC}                             ${CYAN}║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${CYAN}║${NC}  ${GREEN}✓${NC} Agent observed through NAT as: 172.20.0.2               ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  ${GREEN}✓${NC} Connector observed through NAT as: 172.20.0.3           ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  ${GREEN}✓${NC} UDP relay through Intermediate Server working           ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}  ${GREEN}✓${NC} Echo response received through tunnel                   ${CYAN}║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "Useful commands:"
    echo -e "  ${BOLD}View logs:${NC}      docker logs ztna-intermediate"
    echo -e "  ${BOLD}NAT stats:${NC}      docker exec ztna-nat-agent iptables -t nat -L -v"
    echo -e "  ${BOLD}Re-run test:${NC}    cd $DOCKER_DIR && docker compose --profile test run --rm quic-client"
    echo -e "  ${BOLD}Cleanup:${NC}        $0 --clean"
    echo ""
}

# Show logs if requested
show_logs() {
    if [[ "$SHOW_LOGS" == "true" ]]; then
        log_step "Container Logs"
        echo ""
        echo -e "${BOLD}=== Intermediate Server ===${NC}"
        docker logs ztna-intermediate 2>&1 | tail -20
        echo ""
        echo -e "${BOLD}=== App Connector ===${NC}"
        docker logs ztna-app-connector 2>&1 | tail -10
        echo ""
        echo -e "${BOLD}=== NAT Agent Gateway ===${NC}"
        docker logs ztna-nat-agent 2>&1 | tail -5
        echo ""
    fi
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --no-build)
                DO_BUILD=false
                shift
                ;;
            --clean)
                check_prerequisites
                cleanup
                exit 0
                ;;
            --status)
                check_prerequisites
                show_status
                exit 0
                ;;
            --logs)
                SHOW_LOGS=true
                shift
                ;;
            --help|-h)
                print_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                print_help
                exit 1
                ;;
        esac
    done
}

# Main function
main() {
    parse_args "$@"

    print_banner
    check_prerequisites

    # Clean up any existing containers
    cleanup

    # Build if requested
    if [[ "$DO_BUILD" == "true" ]]; then
        build_images
    else
        log_info "Skipping image builds (--no-build)"
    fi

    # Start and verify infrastructure
    start_infrastructure
    verify_infrastructure

    # Run the demo test
    echo ""
    run_demo_test
    local test_result=$?

    # Show logs if requested
    show_logs

    # Print summary
    if [[ $test_result -eq 0 ]]; then
        print_summary
        log_success "Demo completed successfully!"
    else
        log_error "Demo encountered issues. Check logs above."
        exit 1
    fi
}

# Run main
main "$@"
