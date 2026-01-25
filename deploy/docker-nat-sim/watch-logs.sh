#!/bin/bash
# =============================================================================
# Docker NAT Simulation - Live Log Watcher
# =============================================================================
# Run this script with a component name to watch its logs in real-time.
# Open multiple terminals and run different components to see traffic flow.
#
# Usage:
#   ./watch-logs.sh intermediate    # Watch Intermediate Server
#   ./watch-logs.sh connector       # Watch App Connector
#   ./watch-logs.sh nat-agent       # Watch Agent NAT Gateway
#   ./watch-logs.sh nat-connector   # Watch Connector NAT Gateway
#   ./watch-logs.sh echo            # Watch Echo Server
#   ./watch-logs.sh all             # Watch all (combined)
#   ./watch-logs.sh traffic         # Watch NAT traffic stats (refreshing)
#
# For best experience, open 4 terminals:
#   Terminal 1: ./watch-logs.sh intermediate
#   Terminal 2: ./watch-logs.sh connector
#   Terminal 3: ./watch-logs.sh nat-agent
#   Terminal 4: Run the test client
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'
BOLD='\033[1m'

print_header() {
    local title="$1"
    local color="$2"
    echo -e "${color}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${color}║${NC} ${BOLD}$title${NC}"
    echo -e "${color}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

case "${1:-help}" in
    intermediate|int|i)
        print_header "📡 Intermediate Server - Relay Hub" "$CYAN"
        echo -e "${CYAN}Watching for: connections, registrations, relays${NC}"
        echo -e "${CYAN}Press Ctrl+C to stop${NC}"
        echo ""
        docker logs -f ztna-intermediate 2>&1
        ;;

    connector|conn|c)
        print_header "🔌 App Connector - Behind NAT" "$GREEN"
        echo -e "${GREEN}Watching for: registration, forwarding, NAT address${NC}"
        echo -e "${GREEN}Press Ctrl+C to stop${NC}"
        echo ""
        docker logs -f ztna-app-connector 2>&1
        ;;

    nat-agent|na)
        print_header "🔀 NAT Gateway (Agent Side)" "$YELLOW"
        echo -e "${YELLOW}Watching for: NAT rules, packet forwarding${NC}"
        echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
        echo ""
        docker logs -f ztna-nat-agent 2>&1
        ;;

    nat-connector|nc)
        print_header "🔀 NAT Gateway (Connector Side)" "$MAGENTA"
        echo -e "${MAGENTA}Watching for: NAT rules, packet forwarding${NC}"
        echo -e "${MAGENTA}Press Ctrl+C to stop${NC}"
        echo ""
        docker logs -f ztna-nat-connector 2>&1
        ;;

    echo|e)
        print_header "🔊 Echo Server - Test Service" "$BLUE"
        echo -e "${BLUE}Watching for: incoming packets, echoes${NC}"
        echo -e "${BLUE}Press Ctrl+C to stop${NC}"
        echo ""
        docker logs -f ztna-echo-server 2>&1
        ;;

    all|a)
        print_header "📊 All Logs Combined" "$CYAN"
        echo -e "${CYAN}Watching all containers (use component-specific for cleaner view)${NC}"
        echo -e "${CYAN}Press Ctrl+C to stop${NC}"
        echo ""
        docker compose -f "$(dirname "$0")/docker-compose.yml" logs -f 2>&1
        ;;

    traffic|t)
        print_header "📈 NAT Traffic Stats (Refreshing)" "$YELLOW"
        echo -e "${YELLOW}Showing iptables packet counts every 2 seconds${NC}"
        echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
        echo ""
        while true; do
            clear
            echo -e "${BOLD}=== Agent NAT Gateway (172.21.0.0/24 → 172.20.0.2) ===${NC}"
            docker exec ztna-nat-agent iptables -t nat -L POSTROUTING -v -n 2>/dev/null | grep -E "MASQ|pkts" || echo "Container not running"
            echo ""
            echo -e "${BOLD}=== Connector NAT Gateway (172.22.0.0/24 → 172.20.0.3) ===${NC}"
            docker exec ztna-nat-connector iptables -t nat -L POSTROUTING -v -n 2>/dev/null | grep -E "MASQ|pkts" || echo "Container not running"
            echo ""
            echo -e "${CYAN}Refreshing every 2s... (Ctrl+C to stop)${NC}"
            sleep 2
        done
        ;;

    help|h|--help|-h|*)
        echo ""
        echo -e "${BOLD}Docker NAT Simulation - Log Watcher${NC}"
        echo ""
        echo "Usage: $0 <component>"
        echo ""
        echo "Components:"
        echo "  intermediate (i)    Watch Intermediate Server (relay hub)"
        echo "  connector (c)       Watch App Connector (behind NAT)"
        echo "  nat-agent (na)      Watch Agent NAT Gateway"
        echo "  nat-connector (nc)  Watch Connector NAT Gateway"
        echo "  echo (e)            Watch Echo Server"
        echo "  all (a)             Watch all logs combined"
        echo "  traffic (t)         Watch NAT traffic stats (refreshing)"
        echo ""
        echo -e "${BOLD}Recommended Multi-Terminal Setup:${NC}"
        echo ""
        echo "  Terminal 1: $0 intermediate"
        echo "  Terminal 2: $0 connector"
        echo "  Terminal 3: $0 traffic"
        echo "  Terminal 4: Run the test:"
        echo "              cd deploy/docker-nat-sim"
        echo "              docker compose --profile test run --rm quic-client"
        echo ""
        echo -e "${BOLD}Quick Copy Commands:${NC}"
        echo ""
        echo "  # Watch relay activity"
        echo "  docker logs -f ztna-intermediate"
        echo ""
        echo "  # Watch connector registration and forwarding"
        echo "  docker logs -f ztna-app-connector"
        echo ""
        echo "  # Watch NAT packet counts"
        echo "  watch -n1 'docker exec ztna-nat-agent iptables -t nat -L -v -n | grep MASQ'"
        echo ""
        echo "  # Packet capture on NAT gateway"
        echo "  docker exec ztna-nat-agent tcpdump -i eth1 -n udp"
        echo ""
        ;;
esac
