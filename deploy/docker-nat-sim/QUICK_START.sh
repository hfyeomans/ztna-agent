#!/bin/bash
# Quick Start Script for Docker NAT Simulation Environment
# Run this after Docker Desktop is started

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Docker NAT Simulation Environment - Quick Start ===${NC}\n"

# Check Docker is running
echo -e "${YELLOW}Step 1: Checking Docker daemon...${NC}"
if ! docker info >/dev/null 2>&1; then
    echo -e "${RED}ERROR: Docker daemon is not running!${NC}"
    echo "Please start Docker Desktop first:"
    echo "  open /Applications/Docker.app"
    exit 1
fi
echo -e "${GREEN}✓ Docker daemon is running${NC}\n"

# Navigate to correct directory
cd "$(dirname "$0")"
echo -e "${YELLOW}Step 2: Working directory: $(pwd)${NC}\n"

# Build images
echo -e "${YELLOW}Step 3: Building Docker images (this may take 10-20 minutes)...${NC}"
docker compose build --progress=plain
echo -e "${GREEN}✓ Images built successfully${NC}\n"

# Start infrastructure services
echo -e "${YELLOW}Step 4: Starting infrastructure services...${NC}"
docker compose up -d intermediate-server nat-agent nat-connector echo-server app-connector
echo -e "${GREEN}✓ Services started${NC}\n"

# Wait for services to initialize
echo -e "${YELLOW}Step 5: Waiting for services to initialize (10 seconds)...${NC}"
sleep 10

# Check container status
echo -e "${YELLOW}Step 6: Verifying container status...${NC}"
docker compose ps
echo ""

# Check intermediate server logs
echo -e "${YELLOW}Step 7: Checking intermediate server logs...${NC}"
docker logs ztna-intermediate 2>&1 | tail -20
echo ""

# Check app connector logs
echo -e "${YELLOW}Step 8: Checking app connector logs...${NC}"
docker logs ztna-app-connector 2>&1 | tail -20
echo ""

# Verify NAT rules on agent gateway
echo -e "${YELLOW}Step 9: Verifying NAT rules on Agent gateway...${NC}"
docker exec ztna-nat-agent iptables -t nat -L -n -v | head -20
echo ""

# Run connectivity test
echo -e "${YELLOW}Step 10: Running connectivity test...${NC}"
echo "Sending test packet through NAT to echo server..."
docker compose run --rm quic-client \
    --server 172.20.0.10:4433 \
    --service test-service \
    --send "Hello from behind NAT!" \
    --dst 172.22.0.20:9999 \
    --wait 5000

echo ""
echo -e "${GREEN}=== Test Complete ===${NC}"
echo ""
echo "Next steps:"
echo "  1. Review logs: docker compose logs -f"
echo "  2. Run advanced tests: ./test-nat-simulation.sh --verbose"
echo "  3. Debug mode: docker compose --profile debug up -d"
echo "  4. Stop services: docker compose down"
echo ""
echo "For detailed test report, see: TEST_REPORT.md"
