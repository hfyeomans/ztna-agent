#!/bin/bash
# =============================================================================
# Build and Push Multi-Arch Images for ZTNA k8s Deployment
# =============================================================================
# Builds arm64 + amd64 images using Docker buildx and pushes to GHCR
#
# Prerequisites:
#   - Docker with buildx support
#   - Logged into GHCR: echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin
#
# Usage:
#   ./build-push.sh                    # Build and push all images
#   ./build-push.sh --no-push          # Build only (for local testing)
#   ./build-push.sh intermediate       # Build specific component
#   ./build-push.sh --tag v1.0.0       # Use specific tag
#
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
REGISTRY="${REGISTRY:-ghcr.io}"
OWNER="${OWNER:-hfyeomans}"
TAG="${TAG:-latest}"
PLATFORMS="${PLATFORMS:-linux/arm64,linux/amd64}"
DO_PUSH=true
COMPONENTS=()

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

print_help() {
    echo "Usage: $0 [OPTIONS] [COMPONENTS...]"
    echo ""
    echo "Options:"
    echo "  --no-push       Build images but don't push to registry"
    echo "  --tag TAG       Use specific tag (default: latest)"
    echo "  --registry REG  Use specific registry (default: ghcr.io)"
    echo "  --owner OWNER   Use specific owner (default: hfyeomans)"
    echo "  --arm64-only    Build only for arm64 (faster for Pi testing)"
    echo "  --help          Show this help message"
    echo ""
    echo "Components:"
    echo "  intermediate    Build intermediate-server image"
    echo "  connector       Build app-connector image"
    echo "  echo            Build echo-server image"
    echo "  all             Build all images (default)"
    echo ""
    echo "Examples:"
    echo "  $0                           # Build and push all images"
    echo "  $0 --no-push intermediate    # Build intermediate only, no push"
    echo "  $0 --tag v1.0.0 all          # Build all with v1.0.0 tag"
    echo "  $0 --arm64-only connector    # Build connector for arm64 only"
    echo ""
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --no-push)
                DO_PUSH=false
                shift
                ;;
            --tag)
                TAG="$2"
                shift 2
                ;;
            --registry)
                REGISTRY="$2"
                shift 2
                ;;
            --owner)
                OWNER="$2"
                shift 2
                ;;
            --arm64-only)
                PLATFORMS="linux/arm64"
                shift
                ;;
            --help|-h)
                print_help
                exit 0
                ;;
            intermediate|connector|echo|all)
                COMPONENTS+=("$1")
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                print_help
                exit 1
                ;;
        esac
    done

    # Default to all components
    if [[ ${#COMPONENTS[@]} -eq 0 ]]; then
        COMPONENTS=("all")
    fi
}

check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi

    # Check buildx is available
    if ! docker buildx version &> /dev/null; then
        log_error "Docker buildx is not available"
        exit 1
    fi

    # Check/create buildx builder
    if ! docker buildx inspect ztna-builder &> /dev/null; then
        log_info "Creating buildx builder 'ztna-builder'..."
        docker buildx create --name ztna-builder --use --bootstrap
    else
        docker buildx use ztna-builder
    fi

    log_success "Prerequisites OK"
}

build_image() {
    local name=$1
    local dockerfile=$2
    local context=$3
    local image="${REGISTRY}/${OWNER}/ztna-${name}:${TAG}"

    log_info "Building ${name} for platforms: ${PLATFORMS}"
    log_info "  Image: ${image}"

    local push_flag=""
    if [[ "$DO_PUSH" == "true" ]]; then
        push_flag="--push"
    else
        push_flag="--load"
        # --load only works with single platform
        if [[ "$PLATFORMS" == *","* ]]; then
            log_warn "Multi-platform build without push - using --push anyway (images stay in buildx cache)"
            push_flag="--push"
        fi
    fi

    docker buildx build \
        --platform "${PLATFORMS}" \
        --file "${dockerfile}" \
        --tag "${image}" \
        ${push_flag} \
        "${context}"

    log_success "Built ${name}: ${image}"
}

build_intermediate() {
    build_image "intermediate-server" \
        "${PROJECT_ROOT}/deploy/docker-nat-sim/Dockerfile.intermediate" \
        "${PROJECT_ROOT}"
}

build_connector() {
    # For k8s, we need a simpler Dockerfile without NAT-specific entrypoint
    log_info "Creating k8s-compatible connector Dockerfile..."

    cat > "${SCRIPT_DIR}/Dockerfile.connector-k8s" << 'DOCKERFILE'
# Dockerfile for ZTNA App Connector (k8s)
FROM rust:1.83-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev cmake build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY app-connector ./app-connector
WORKDIR /build/app-connector
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false -u 1000 ztna
COPY --from=builder /build/app-connector/target/release/app-connector /usr/local/bin/
RUN mkdir -p /etc/ztna/certs && chown ztna:ztna /etc/ztna/certs

EXPOSE 4434/udp
USER ztna
ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/app-connector"]
DOCKERFILE

    build_image "app-connector" \
        "${SCRIPT_DIR}/Dockerfile.connector-k8s" \
        "${PROJECT_ROOT}"

    rm -f "${SCRIPT_DIR}/Dockerfile.connector-k8s"
}

build_echo() {
    build_image "echo-server" \
        "${PROJECT_ROOT}/deploy/docker-nat-sim/Dockerfile.echo-server" \
        "${PROJECT_ROOT}"
}

main() {
    parse_args "$@"

    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}          ZTNA Multi-Arch Image Builder                       ${BLUE}║${NC}"
    echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${BLUE}║${NC}  Registry:  ${REGISTRY}                                      ${BLUE}║${NC}"
    echo -e "${BLUE}║${NC}  Owner:     ${OWNER}                                    ${BLUE}║${NC}"
    echo -e "${BLUE}║${NC}  Tag:       ${TAG}                                          ${BLUE}║${NC}"
    echo -e "${BLUE}║${NC}  Platforms: ${PLATFORMS}                         ${BLUE}║${NC}"
    echo -e "${BLUE}║${NC}  Push:      ${DO_PUSH}                                         ${BLUE}║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    check_prerequisites

    cd "${PROJECT_ROOT}"

    for component in "${COMPONENTS[@]}"; do
        case $component in
            intermediate)
                build_intermediate
                ;;
            connector)
                build_connector
                ;;
            echo)
                build_echo
                ;;
            all)
                build_intermediate
                build_connector
                build_echo
                ;;
        esac
    done

    echo ""
    log_success "All builds complete!"

    if [[ "$DO_PUSH" == "true" ]]; then
        echo ""
        echo "Images pushed to:"
        echo "  - ${REGISTRY}/${OWNER}/ztna-intermediate-server:${TAG}"
        echo "  - ${REGISTRY}/${OWNER}/ztna-app-connector:${TAG}"
        echo "  - ${REGISTRY}/${OWNER}/ztna-echo-server:${TAG}"
    fi
}

main "$@"
