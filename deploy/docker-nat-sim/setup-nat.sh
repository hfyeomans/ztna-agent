#!/bin/bash
# setup-nat.sh - Configure NAT rules for hole punching simulation
#
# This script is designed to run inside NAT gateway containers.
# It sets up iptables rules to simulate real-world NAT behavior.
#
# Usage:
#   ./setup-nat.sh [NAT_TYPE]
#
# NAT Types:
#   full-cone     - Endpoint-Independent Mapping (easiest for P2P)
#   restricted    - Address-Restricted Cone NAT
#   port-restrict - Port-Restricted Cone NAT
#   symmetric     - Symmetric NAT (hardest for P2P, requires TURN)
#
# Default: port-restrict (common in home routers)

set -e

NAT_TYPE="${1:-port-restrict}"
PRIVATE_IFACE="${PRIVATE_IFACE:-eth1}"
PUBLIC_IFACE="${PUBLIC_IFACE:-eth0}"
PRIVATE_NETWORK="${PRIVATE_NETWORK:-172.21.0.0/24}"

log() {
    echo "[NAT-SETUP] $(date '+%Y-%m-%d %H:%M:%S') $*"
}

# Validate interface name (alphanumeric only, prevents path injection)
validate_iface() {
    local iface="$1"
    if ! echo "$iface" | grep -qE '^[a-zA-Z0-9]+$'; then
        log "ERROR: Invalid interface name: '$iface' (must be alphanumeric)"
        exit 1
    fi
}

# Enable IP forwarding
enable_forwarding() {
    log "Enabling IP forwarding..."

    # L4: Validate interface names before using in /proc paths
    validate_iface "${PRIVATE_IFACE}"
    validate_iface "${PUBLIC_IFACE}"

    echo 1 > /proc/sys/net/ipv4/ip_forward

    # Disable reverse path filtering (needed for asymmetric routing)
    echo 0 > /proc/sys/net/ipv4/conf/all/rp_filter
    echo 0 > /proc/sys/net/ipv4/conf/${PRIVATE_IFACE}/rp_filter
    echo 0 > /proc/sys/net/ipv4/conf/${PUBLIC_IFACE}/rp_filter
}

# Clear existing rules
flush_rules() {
    log "Flushing existing iptables rules..."
    iptables -F
    iptables -t nat -F
    iptables -t mangle -F
    iptables -P INPUT ACCEPT
    iptables -P FORWARD ACCEPT
    iptables -P OUTPUT ACCEPT
}

# Full Cone NAT (Endpoint-Independent Mapping)
# - Once a mapping is created (internal:port -> external:port), any external host can send to external:port
# - Easiest for P2P hole punching
setup_full_cone() {
    log "Setting up Full Cone NAT (Endpoint-Independent)..."

    # Standard MASQUERADE for outbound
    iptables -t nat -A POSTROUTING -s ${PRIVATE_NETWORK} -o ${PUBLIC_IFACE} -j MASQUERADE

    # Allow all established/related connections back
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state RELATED,ESTABLISHED -j ACCEPT
    iptables -A FORWARD -i ${PRIVATE_IFACE} -o ${PUBLIC_IFACE} -j ACCEPT

    # For true full cone, we'd need to persist mappings and allow any external source
    # Linux conntrack approximates this with loose state tracking
    echo 1 > /proc/sys/net/netfilter/nf_conntrack_udp_timeout
    echo 30 > /proc/sys/net/netfilter/nf_conntrack_udp_timeout_stream
}

# Address-Restricted Cone NAT
# - Internal host must first send to external IP before that IP can send back
# - Any port from that IP is allowed
setup_restricted_cone() {
    log "Setting up Address-Restricted Cone NAT..."

    iptables -t nat -A POSTROUTING -s ${PRIVATE_NETWORK} -o ${PUBLIC_IFACE} -j MASQUERADE

    # Only allow return traffic from addresses we've sent to
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state RELATED,ESTABLISHED -j ACCEPT
    iptables -A FORWARD -i ${PRIVATE_IFACE} -o ${PUBLIC_IFACE} -j ACCEPT

    # Drop unsolicited inbound
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -j DROP
}

# Port-Restricted Cone NAT (most common in home routers)
# - Internal host must first send to external IP:port before that IP:port can send back
# - This is Linux's default conntrack behavior
setup_port_restricted() {
    log "Setting up Port-Restricted Cone NAT (default)..."

    iptables -t nat -A POSTROUTING -s ${PRIVATE_NETWORK} -o ${PUBLIC_IFACE} -j MASQUERADE

    # Only allow return traffic matching exact IP:port we sent to
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state RELATED,ESTABLISHED -j ACCEPT
    iptables -A FORWARD -i ${PRIVATE_IFACE} -o ${PUBLIC_IFACE} -j ACCEPT

    # Drop unsolicited inbound
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state NEW -j DROP
}

# Symmetric NAT (hardest for P2P)
# - Each destination gets a different external port mapping
# - Requires TURN relay, standard hole punching usually fails
setup_symmetric() {
    log "Setting up Symmetric NAT (most restrictive)..."

    # Use SNAT with random port assignment to simulate symmetric behavior
    # This creates different mappings for different destinations
    iptables -t nat -A POSTROUTING -s ${PRIVATE_NETWORK} -o ${PUBLIC_IFACE} -j MASQUERADE --random

    # Strict state matching
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state RELATED,ESTABLISHED -j ACCEPT
    iptables -A FORWARD -i ${PRIVATE_IFACE} -o ${PUBLIC_IFACE} -j ACCEPT
    iptables -A FORWARD -i ${PUBLIC_IFACE} -o ${PRIVATE_IFACE} -m state --state NEW -j DROP

    # Short UDP timeout to force new mappings
    echo 30 > /proc/sys/net/netfilter/nf_conntrack_udp_timeout
    echo 60 > /proc/sys/net/netfilter/nf_conntrack_udp_timeout_stream
}

# Print current NAT mappings
show_mappings() {
    log "Current NAT conntrack entries:"
    cat /proc/net/nf_conntrack 2>/dev/null | grep -E "^udp" | head -20 || \
    conntrack -L -p udp 2>/dev/null | head -20 || \
    echo "(conntrack not available)"
}

# Main
main() {
    log "NAT Setup Script starting..."
    log "  NAT Type: ${NAT_TYPE}"
    log "  Private Interface: ${PRIVATE_IFACE}"
    log "  Public Interface: ${PUBLIC_IFACE}"
    log "  Private Network: ${PRIVATE_NETWORK}"

    enable_forwarding
    flush_rules

    case "${NAT_TYPE}" in
        full-cone|fullcone|fc)
            setup_full_cone
            ;;
        restricted|address-restricted|rc)
            setup_restricted_cone
            ;;
        port-restrict|port-restricted|prc)
            setup_port_restricted
            ;;
        symmetric|sym)
            setup_symmetric
            ;;
        *)
            log "ERROR: Unknown NAT type '${NAT_TYPE}'"
            log "Valid types: full-cone, restricted, port-restrict, symmetric"
            exit 1
            ;;
    esac

    log "NAT rules configured successfully!"
    log ""
    log "Current iptables rules:"
    iptables -L -n -v
    log ""
    log "Current NAT rules:"
    iptables -t nat -L -n -v
    log ""

    # Keep running to show periodic status
    while true; do
        sleep 60
        show_mappings
    done
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
