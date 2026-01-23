#!/bin/bash
# =============================================================================
# macOS Agent Demo Script
# =============================================================================
# Demonstrates the full ZTNA stack with the native macOS Agent app.
#
# Usage:
#   ./tests/e2e/scenarios/macos-agent-demo.sh [OPTIONS]
#
# Options:
#   --build         Build all components first
#   --duration N    Run for N seconds (default: 30)
#   --auto          Full automation (start, wait, stop, exit)
#   --manual        Interactive mode (starts components, waits for user)
#   --logs          Show live logs in separate terminals
#   --help          Show this help
#
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
E2E_DIR="$PROJECT_ROOT/tests/e2e"

# Default settings
DURATION=30
MODE="manual"
BUILD=false
SHOW_LOGS=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build)
            BUILD=true
            shift
            ;;
        --duration)
            DURATION="$2"
            shift 2
            ;;
        --auto)
            MODE="auto"
            shift
            ;;
        --manual)
            MODE="manual"
            shift
            ;;
        --logs)
            SHOW_LOGS=true
            shift
            ;;
        --help|-h)
            head -30 "$0" | tail -25
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[DEMO]${NC} $1"; }
ok() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; exit 1; }

# Cleanup function
cleanup() {
    log "Cleaning up..."
    pkill -f "intermediate-server" 2>/dev/null || true
    pkill -f "app-connector" 2>/dev/null || true
    pkill -f "udp-echo" 2>/dev/null || true
    # Don't kill the ZtnaAgent app - let it stop naturally or user can close it
}
trap cleanup EXIT

# =============================================================================
# Build Phase
# =============================================================================
if [ "$BUILD" = true ]; then
    log "Building all components..."

    log "  Building Rust library..."
    (cd "$PROJECT_ROOT/core/packet_processor" && cargo build --release --target aarch64-apple-darwin) || fail "Rust build failed"

    log "  Building Intermediate Server..."
    (cd "$PROJECT_ROOT/intermediate-server" && cargo build --release) || fail "Intermediate build failed"

    log "  Building App Connector..."
    (cd "$PROJECT_ROOT/app-connector" && cargo build --release) || fail "Connector build failed"

    log "  Building Echo Server..."
    (cd "$PROJECT_ROOT/tests/e2e/fixtures/echo-server" && cargo build --release) || fail "Echo build failed"

    log "  Building Xcode project..."
    xcodebuild -project "$PROJECT_ROOT/ios-macos/ZtnaAgent/ZtnaAgent.xcodeproj" \
        -scheme ZtnaAgent -configuration Debug \
        -derivedDataPath /tmp/ZtnaAgent-build \
        build 2>&1 | tail -5 || fail "Xcode build failed"

    ok "All components built successfully"
fi

# =============================================================================
# Check Binaries
# =============================================================================
log "Checking binaries..."

INTERMEDIATE="$PROJECT_ROOT/intermediate-server/target/release/intermediate-server"
CONNECTOR="$PROJECT_ROOT/app-connector/target/release/app-connector"
ECHO="$PROJECT_ROOT/tests/e2e/fixtures/echo-server/target/release/udp-echo"
AGENT_APP="/tmp/ZtnaAgent-build/Build/Products/Debug/ZtnaAgent.app"

[ -f "$INTERMEDIATE" ] || fail "Intermediate server not built. Run with --build"
[ -f "$CONNECTOR" ] || fail "App connector not built. Run with --build"
[ -f "$ECHO" ] || fail "Echo server not built. Run with --build"
[ -d "$AGENT_APP" ] || fail "ZtnaAgent.app not built. Run with --build"

ok "All binaries found"

# =============================================================================
# Start Infrastructure
# =============================================================================
log "Starting test infrastructure..."

mkdir -p "$E2E_DIR/artifacts/logs"

# Start Echo Server
log "  Starting Echo Server on port 9999..."
$ECHO --port 9999 > "$E2E_DIR/artifacts/logs/echo-server.log" 2>&1 &
sleep 0.5
pgrep -f "udp-echo" > /dev/null || fail "Echo server failed to start"
ok "Echo Server running"

# Start Intermediate Server
log "  Starting Intermediate Server on port 4433..."
RUST_LOG=info $INTERMEDIATE 4433 \
    "$PROJECT_ROOT/certs/cert.pem" \
    "$PROJECT_ROOT/certs/key.pem" \
    > "$E2E_DIR/artifacts/logs/intermediate-server.log" 2>&1 &
sleep 0.5
pgrep -f "intermediate-server" > /dev/null || fail "Intermediate server failed to start"
ok "Intermediate Server running"

# Start App Connector
log "  Starting App Connector..."
RUST_LOG=info $CONNECTOR \
    --server 127.0.0.1:4433 \
    --service test-service \
    --forward 127.0.0.1:9999 \
    > "$E2E_DIR/artifacts/logs/app-connector.log" 2>&1 &
sleep 0.5
pgrep -f "app-connector" > /dev/null || fail "App connector failed to start"
ok "App Connector running"

# Wait for connector to register
sleep 1
ok "Test infrastructure ready"

# =============================================================================
# Show Log Windows (optional)
# =============================================================================
if [ "$SHOW_LOGS" = true ]; then
    log "Opening log windows..."
    osascript -e "tell app \"Terminal\" to do script \"tail -f '$E2E_DIR/artifacts/logs/intermediate-server.log'\""
    osascript -e "tell app \"Terminal\" to do script \"tail -f '$E2E_DIR/artifacts/logs/app-connector.log'\""
fi

# =============================================================================
# Launch macOS Agent
# =============================================================================
log "Launching ZtnaAgent app..."

if [ "$MODE" = "auto" ]; then
    log "  Mode: Automatic (--auto-start --auto-stop $DURATION --exit-after-stop)"
    open -a "$AGENT_APP" --args --auto-start --auto-stop "$DURATION" --exit-after-stop

    log "Waiting for agent to connect and run for $DURATION seconds..."

    # Monitor agent logs
    sleep 2
    EXT_PID=$(pgrep -f "com.hankyeomans.ztna-agent.ZtnaAgent.Extension" | head -1 || true)
    if [ -n "$EXT_PID" ]; then
        ok "Extension running (PID: $EXT_PID)"

        # Wait for connection
        for i in {1..30}; do
            if /usr/bin/log show --last 5s --predicate "processIdentifier == $EXT_PID" --info 2>/dev/null | grep -q "QUIC connection established"; then
                ok "QUIC connection established!"
                break
            fi
            sleep 1
        done

        # Show key log lines
        log "Extension logs:"
        /usr/bin/log show --last 10s --predicate "processIdentifier == $EXT_PID" --info 2>/dev/null | grep -E "(Starting|connected|established|QAD|observed)" | head -10 || true
    fi

    log "Running for $DURATION seconds..."
    sleep "$DURATION"

    ok "Demo complete"
else
    log "  Mode: Manual (use Start/Stop buttons in app)"
    open -a "$AGENT_APP"

    echo ""
    echo "=============================================="
    echo "  ZtnaAgent App Launched!"
    echo "=============================================="
    echo ""
    echo "  Click 'Start' to connect to Intermediate Server"
    echo "  Click 'Stop' when done"
    echo ""
    echo "  View logs with:"
    echo "    log stream --predicate 'subsystem CONTAINS \"ztna\"' --info"
    echo ""
    echo "  Infrastructure logs:"
    echo "    tail -f $E2E_DIR/artifacts/logs/*.log"
    echo ""
    echo "  Press Ctrl+C to stop infrastructure..."
    echo "=============================================="

    # Wait for user to Ctrl+C
    while true; do
        sleep 1
    done
fi
