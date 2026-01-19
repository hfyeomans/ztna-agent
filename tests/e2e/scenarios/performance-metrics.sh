#!/bin/zsh
# ============================================================================
# Phase 6: Performance Metrics Tests
# ============================================================================
#
# Measures:
# - 6.1 Latency: Baseline vs tunneled RTT, p50/p95/p99 percentiles
# - 6.2 Throughput: Baseline vs tunneled PPS and Mbps
# - 6.3 Timing: Handshake time, reconnection time, CPU/memory
#
# Usage:
#   tests/e2e/scenarios/performance-metrics.sh
#
# Output:
#   - Console summary with all metrics
#   - Detailed results in tests/e2e/artifacts/metrics/
#
# ============================================================================

set -e

# Get project root (script location)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

# Source common functions
source "$PROJECT_ROOT/tests/e2e/lib/common.sh"

# ============================================================================
# Configuration
# ============================================================================

RTT_SAMPLES=${RTT_SAMPLES:-50}
BURST_COUNT=${BURST_COUNT:-200}
THROUGHPUT_PAYLOAD_SIZE=${THROUGHPUT_PAYLOAD_SIZE:-1000}
HANDSHAKE_SAMPLES=${HANDSHAKE_SAMPLES:-10}
RECONNECT_SAMPLES=${RECONNECT_SAMPLES:-3}

# Echo server host (not defined in common.sh, so set here)
ECHO_SERVER_HOST="${ECHO_SERVER_HOST:-127.0.0.1}"

METRICS_DIR="$PROJECT_ROOT/tests/e2e/artifacts/metrics"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
METRICS_FILE="$METRICS_DIR/perf_${TIMESTAMP}.txt"

# ============================================================================
# Utility Functions
# ============================================================================

extract_metric() {
    local output="$1"
    local key="$2"
    echo "$output" | grep "^${key}:" | cut -d: -f2 | head -1
}

# Direct UDP latency measurement (baseline - no tunnel)
# NOTE: This is a best-effort estimate using Python for timing
# Standard netcat UDP doesn't reliably support request-response timing
measure_baseline_latency() {
    local samples="$1"
    local payload_size="${2:-64}"

    log_info "Measuring baseline latency (direct UDP, $samples samples)..."
    log_info "  Note: Baseline uses Python UDP client for timing accuracy"

    # Use Python for reliable UDP timing
    python3 << 'PYEOF' - "$samples" "$ECHO_SERVER_PORT"
import socket
import time
import sys
import statistics

samples = int(sys.argv[1])
port = int(sys.argv[2])

rtts = []
timeouts = 0

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(0.1)  # 100ms timeout

for i in range(samples):
    payload = f"PING{i}".encode()
    start = time.perf_counter_ns()
    try:
        sock.sendto(payload, ("127.0.0.1", port))
        data, addr = sock.recvfrom(1024)
        end = time.perf_counter_ns()
        rtt_us = (end - start) // 1000
        rtts.append(rtt_us)
    except socket.timeout:
        timeouts += 1
    time.sleep(0.001)  # 1ms between samples

sock.close()

if rtts:
    rtts.sort()
    count = len(rtts)
    print(f"BASELINE_RTT_COUNT:{count}")
    print(f"BASELINE_RTT_TIMEOUTS:{timeouts}")
    print(f"BASELINE_RTT_MIN_US:{rtts[0]}")
    print(f"BASELINE_RTT_MAX_US:{rtts[-1]}")
    print(f"BASELINE_RTT_AVG_US:{int(statistics.mean(rtts))}")
    print(f"BASELINE_RTT_P50_US:{rtts[count * 50 // 100]}")
    print(f"BASELINE_RTT_P95_US:{rtts[count * 95 // 100]}")
    print(f"BASELINE_RTT_P99_US:{rtts[min(count * 99 // 100, count - 1)]}")
else:
    print(f"BASELINE_RTT_COUNT:0")
    print(f"BASELINE_RTT_TIMEOUTS:{timeouts}")
PYEOF
}

# Tunneled latency measurement
measure_tunneled_latency() {
    local samples="$1"
    local payload_size="${2:-64}"

    log_info "Measuring tunneled latency ($samples samples, ${payload_size}B payload)..."

    local output
    output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --measure-rtt \
        --rtt-count "$samples" \
        --payload-size "$payload_size" \
        --dst "$ECHO_SERVER_HOST:$ECHO_SERVER_PORT" \
        2>&1)

    # Extract and prefix metrics
    echo "TUNNELED_RTT_COUNT:$(extract_metric "$output" "RTT_COUNT")"
    echo "TUNNELED_RTT_TIMEOUTS:$(extract_metric "$output" "RTT_TIMEOUTS")"
    echo "TUNNELED_RTT_MIN_US:$(extract_metric "$output" "RTT_MIN_US")"
    echo "TUNNELED_RTT_MAX_US:$(extract_metric "$output" "RTT_MAX_US")"
    echo "TUNNELED_RTT_AVG_US:$(extract_metric "$output" "RTT_AVG_US")"
    echo "TUNNELED_RTT_P50_US:$(extract_metric "$output" "RTT_P50_US")"
    echo "TUNNELED_RTT_P95_US:$(extract_metric "$output" "RTT_P95_US")"
    echo "TUNNELED_RTT_P99_US:$(extract_metric "$output" "RTT_P99_US")"
}

# Throughput measurement
measure_throughput() {
    local burst_count="$1"
    local payload_size="$2"

    log_info "Measuring throughput ($burst_count packets, ${payload_size}B payload)..."

    local output
    output=$("$QUIC_CLIENT_BIN" \
        --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
        --service "$SERVICE_ID" \
        --burst "$burst_count" \
        --payload-size "$payload_size" \
        --dst "$ECHO_SERVER_HOST:$ECHO_SERVER_PORT" \
        --wait 5000 \
        2>&1)

    local pps=$(extract_metric "$output" "BURST_PPS")
    local sent=$(extract_metric "$output" "BURST_SENT")

    # Calculate Mbps: (packets * payload_size * 8) / 1000000 * pps
    # But since burst is sent in rapid succession, we use total bytes / time
    # PPS is packets/second, so Mbps = (pps * payload_size * 8) / 1000000
    local mbps=""
    if [[ -n "$pps" ]] && [[ "$pps" != "0" ]]; then
        mbps=$(echo "scale=2; ($pps * $payload_size * 8) / 1000000" | bc 2>/dev/null || echo "N/A")
    fi

    echo "THROUGHPUT_SENT:$sent"
    echo "THROUGHPUT_PPS:$pps"
    echo "THROUGHPUT_MBPS:$mbps"
    echo "THROUGHPUT_PAYLOAD_SIZE:$payload_size"
}

# Handshake timing
measure_handshake() {
    local samples="$1"

    log_info "Measuring handshake time ($samples samples)..."

    local handshakes=()

    for i in $(seq 1 $samples); do
        local output
        output=$("$QUIC_CLIENT_BIN" \
            --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
            --service "$SERVICE_ID" \
            --measure-handshake \
            --send-udp "HANDSHAKE_TEST_$i" \
            --dst "$ECHO_SERVER_HOST:$ECHO_SERVER_PORT" \
            --wait 500 \
            2>&1)

        local hs_us=$(extract_metric "$output" "HANDSHAKE_US")
        if [[ -n "$hs_us" ]]; then
            handshakes+=($hs_us)
        fi

        # Small delay between samples
        sleep 0.1
    done

    if [[ ${#handshakes[@]} -gt 0 ]]; then
        local sorted=($(echo "${handshakes[@]}" | tr ' ' '\n' | sort -n))
        local count=${#sorted[@]}
        local min=${sorted[1]}
        local max=${sorted[$count]}

        local sum=0
        for hs in "${sorted[@]}"; do
            sum=$((sum + hs))
        done
        local avg=$((sum / count))

        local p50_idx=$((count * 50 / 100))
        [[ $p50_idx -lt 1 ]] && p50_idx=1
        local p50=${sorted[$p50_idx]}

        echo "HANDSHAKE_COUNT:$count"
        echo "HANDSHAKE_MIN_US:$min"
        echo "HANDSHAKE_MAX_US:$max"
        echo "HANDSHAKE_AVG_US:$avg"
        echo "HANDSHAKE_P50_US:$p50"
    else
        echo "HANDSHAKE_COUNT:0"
    fi
}

# CPU/Memory measurement
measure_resources() {
    log_info "Measuring resource usage..."

    # Get PIDs
    local intermediate_pid="${INTERMEDIATE_PID:-}"
    local connector_pid="${CONNECTOR_PID:-}"
    local echo_pid="${ECHO_PID:-}"

    if [[ -n "$intermediate_pid" ]]; then
        local intermediate_mem=$(ps -o rss= -p "$intermediate_pid" 2>/dev/null | tr -d ' ')
        local intermediate_cpu=$(ps -o %cpu= -p "$intermediate_pid" 2>/dev/null | tr -d ' ')
        echo "INTERMEDIATE_MEM_KB:${intermediate_mem:-0}"
        echo "INTERMEDIATE_CPU_PCT:${intermediate_cpu:-0}"
    fi

    if [[ -n "$connector_pid" ]]; then
        local connector_mem=$(ps -o rss= -p "$connector_pid" 2>/dev/null | tr -d ' ')
        local connector_cpu=$(ps -o %cpu= -p "$connector_pid" 2>/dev/null | tr -d ' ')
        echo "CONNECTOR_MEM_KB:${connector_mem:-0}"
        echo "CONNECTOR_CPU_PCT:${connector_cpu:-0}"
    fi

    if [[ -n "$echo_pid" ]]; then
        local echo_mem=$(ps -o rss= -p "$echo_pid" 2>/dev/null | tr -d ' ')
        local echo_cpu=$(ps -o %cpu= -p "$echo_pid" 2>/dev/null | tr -d ' ')
        echo "ECHO_MEM_KB:${echo_mem:-0}"
        echo "ECHO_CPU_PCT:${echo_cpu:-0}"
    fi
}

# Reconnection time measurement
# NOTE: This is a stretch metric - complex due to connector re-registration timing
# The reliability tests (Phase 5) cover restart behavior more thoroughly
measure_reconnection() {
    local samples="$1"

    log_info "Measuring reconnection time ($samples samples)..."
    log_info "  Note: Reconnection timing is approximate (includes re-registration)"

    local reconnects=()

    for i in $(seq 1 $samples); do
        # Restart connector
        stop_connector 2>/dev/null || true
        sleep 1

        local start_ms=$(($(date +%s%N) / 1000000))

        start_connector

        # Wait for connector to be ready, then measure time to first successful data flow
        sleep 1  # Give connector time to connect to intermediate

        # Now measure time to successful data flow
        local max_wait=30  # 3 seconds max
        local connected=0
        for j in $(seq 1 $max_wait); do
            local output
            output=$("$QUIC_CLIENT_BIN" \
                --server "$INTERMEDIATE_HOST:$INTERMEDIATE_PORT" \
                --service "$SERVICE_ID" \
                --send-udp "RECONNECT_$i" \
                --dst "$ECHO_SERVER_HOST:$ECHO_SERVER_PORT" \
                --wait 500 \
                2>&1) || true

            # Check for data echo (42+ byte response = 84+ hex chars)
            local data_recv=$(echo "$output" | grep -o "RECV:[0-9a-fA-F]*" | awk 'length($0) > 90 {print}' | wc -l | tr -d ' ')
            if [[ "$data_recv" -gt 0 ]]; then
                connected=1
                break
            fi
            sleep 0.1
        done

        local end_ms=$(($(date +%s%N) / 1000000))

        if [[ $connected -eq 1 ]]; then
            local reconnect_ms=$((end_ms - start_ms))
            reconnects+=($reconnect_ms)
            log_info "  Sample $i: ${reconnect_ms}ms"
        else
            log_warn "  Sample $i: timeout"
        fi
    done

    if [[ ${#reconnects[@]} -gt 0 ]]; then
        local sorted=($(echo "${reconnects[@]}" | tr ' ' '\n' | sort -n))
        local count=${#sorted[@]}
        local min=${sorted[1]}
        local max=${sorted[$count]}

        local sum=0
        for rc in "${sorted[@]}"; do
            sum=$((sum + rc))
        done
        local avg=$((sum / count))

        echo "RECONNECT_COUNT:$count"
        echo "RECONNECT_MIN_MS:$min"
        echo "RECONNECT_MAX_MS:$max"
        echo "RECONNECT_AVG_MS:$avg"
    else
        echo "RECONNECT_COUNT:0"
        log_warn "No successful reconnection samples collected"
    fi
}

# ============================================================================
# Main
# ============================================================================

main() {
    log_info "=== Phase 6: Performance Metrics Tests ==="
    log_info "Server: $INTERMEDIATE_HOST:$INTERMEDIATE_PORT"
    log_info "Service: $SERVICE_ID"
    log_info "Metrics output: $METRICS_FILE"
    log_info ""

    # Pre-cleanup
    log_info "Cleaning up stale processes..."
    pkill -f "$PROJECT_ROOT/intermediate-server" 2>/dev/null || true
    pkill -f "$PROJECT_ROOT/app-connector" 2>/dev/null || true
    pkill -f "udp-echo" 2>/dev/null || true
    sleep 2

    # Setup
    check_binaries
    mkdir -p "$METRICS_DIR"

    # Start components
    start_echo_server
    sleep 1

    start_intermediate
    sleep 1

    start_connector
    sleep 2  # Allow connection to establish

    # Capture PIDs for resource monitoring
    ECHO_PID="${COMPONENT_PIDS[echo]:-}"
    INTERMEDIATE_PID="${COMPONENT_PIDS[intermediate]:-}"
    CONNECTOR_PID="${COMPONENT_PIDS[connector]:-}"

    # Collect all metrics
    {
        echo "# Performance Metrics - $(date)"
        echo "# ============================================"
        echo ""

        # 6.1 Latency
        echo "# 6.1 Latency Measurements"
        echo "# ------------------------"
        measure_baseline_latency $RTT_SAMPLES
        echo ""
        measure_tunneled_latency $RTT_SAMPLES
        echo ""

        # 6.2 Throughput
        echo "# 6.2 Throughput Measurements"
        echo "# ---------------------------"
        measure_throughput $BURST_COUNT $THROUGHPUT_PAYLOAD_SIZE
        echo ""

        # 6.3 Timing
        echo "# 6.3 Timing Measurements"
        echo "# -----------------------"
        measure_handshake $HANDSHAKE_SAMPLES
        echo ""
        measure_resources
        echo ""
        measure_reconnection $RECONNECT_SAMPLES
        echo ""

    } | tee "$METRICS_FILE"

    # Print summary
    log_info ""
    log_info "=== Performance Summary ==="

    # Extract key metrics for summary
    local baseline_avg=$(grep "BASELINE_RTT_AVG_US" "$METRICS_FILE" | cut -d: -f2)
    local tunneled_avg=$(grep "TUNNELED_RTT_AVG_US" "$METRICS_FILE" | cut -d: -f2)
    local throughput_pps=$(grep "THROUGHPUT_PPS" "$METRICS_FILE" | cut -d: -f2)
    local throughput_mbps=$(grep "THROUGHPUT_MBPS" "$METRICS_FILE" | cut -d: -f2)
    local handshake_avg=$(grep "HANDSHAKE_AVG_US" "$METRICS_FILE" | cut -d: -f2)
    local reconnect_avg=$(grep "RECONNECT_AVG_MS" "$METRICS_FILE" | cut -d: -f2)

    if [[ -n "$baseline_avg" ]] && [[ -n "$tunneled_avg" ]] && [[ "$baseline_avg" -gt 0 ]]; then
        local overhead=$((tunneled_avg - baseline_avg))
        local overhead_pct=$(echo "scale=1; ($overhead * 100) / $baseline_avg" | bc 2>/dev/null || echo "N/A")
        log_info "Latency overhead: ${overhead} µs (${overhead_pct}%)"
    fi

    log_info "  Baseline RTT avg: ${baseline_avg:-N/A} µs"
    log_info "  Tunneled RTT avg: ${tunneled_avg:-N/A} µs"
    log_info "  Throughput: ${throughput_pps:-N/A} pps (${throughput_mbps:-N/A} Mbps)"
    log_info "  Handshake avg: ${handshake_avg:-N/A} µs"
    log_info "  Reconnect avg: ${reconnect_avg:-N/A} ms"
    log_info ""
    log_info "Full metrics: $METRICS_FILE"

    # Cleanup
    log_info "Cleaning up..."
    stop_all_components

    log_info "=== Performance metrics collection complete ==="
}

# Run main
main "$@"
