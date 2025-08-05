#!/bin/bash
# VoltageEMS Deployment Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Deploying VoltageEMS ===${NC}"

# Check Docker Compose
if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}Error: docker-compose not found${NC}"
    exit 1
fi

# Stop old containers
echo -e "${YELLOW}Stopping old containers...${NC}"
docker-compose down

# Build images
echo -e "${YELLOW}Building images...${NC}"
bash scripts/build.sh release

# Start services
echo -e "${YELLOW}Starting services...${NC}"
docker-compose up -d

# Wait for services to start
echo -e "${YELLOW}Waiting for services to start...${NC}"
sleep 10

# Verify service status
echo -e "${YELLOW}Verifying service status...${NC}"
for port in 80 6000 6001 6002 6003 6004 6005; do
    if curl -s http://localhost:$port/health > /dev/null 2>&1; then
        echo -e "Port $port: ${GREEN}OK${NC}"
    else
        echo -e "Port $port: ${RED}Failed${NC}"
    fi
done

echo -e "${GREEN}Deployment completed!${NC}"
echo ""
echo "Access URLs:"
echo "  Homepage: http://localhost"
echo "  API: http://localhost/api"