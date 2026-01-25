#!/bin/bash
# Entrypoint script for quic-client (agent simulator)
# Sets up routing through NAT gateway, then runs the client

set -e

# Add route to public network through NAT gateway
# Agent network uses 172.21.0.2 as the NAT gateway
echo "Setting up route to public network via NAT gateway (172.21.0.2)..."
ip route add 172.20.0.0/24 via 172.21.0.2 2>/dev/null || true

# Show current routes
echo "Current routes:"
ip route

# Run the quic-test-client
exec /usr/local/bin/quic-test-client "$@"
