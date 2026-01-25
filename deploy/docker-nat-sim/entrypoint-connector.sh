#!/bin/bash
# Entrypoint script for app-connector
# Sets up routing through NAT gateway, then runs the connector

set -e

# Add route to public network through NAT gateway
# This is run as root before dropping privileges
echo "Setting up route to public network via NAT gateway (172.22.0.2)..."
ip route add 172.20.0.0/24 via 172.22.0.2 2>/dev/null || true

# Show current routes
echo "Current routes:"
ip route

# Drop privileges and run the app-connector
exec gosu ztna /usr/local/bin/app-connector "$@"
