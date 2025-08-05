#!/bin/bash
# VoltageEMS Clean Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Cleaning VoltageEMS ===${NC}"

# Stop and remove containers
echo -e "${YELLOW}Stopping and removing containers...${NC}"
docker-compose down 2>/dev/null || true
docker stop redis-dev redis-test 2>/dev/null || true
docker rm redis-dev redis-test 2>/dev/null || true

# Remove images
if [ "$1" = "--images" ]; then
    echo -e "${YELLOW}Removing images...${NC}"
    docker rmi voltageems-comsrv:latest 2>/dev/null || true
    docker rmi voltageems-modsrv:latest 2>/dev/null || true
    docker rmi voltageems-alarmsrv:latest 2>/dev/null || true
    docker rmi voltageems-hissrv:latest 2>/dev/null || true
    docker rmi voltageems-rulesrv:latest 2>/dev/null || true
    docker rmi voltageems-apigateway:latest 2>/dev/null || true
    docker rmi voltageems-redis:latest 2>/dev/null || true
fi

# Clean Rust builds
echo -e "${YELLOW}Cleaning Rust builds...${NC}"
cargo clean

# Clean logs
echo -e "${YELLOW}Cleaning log files...${NC}"
find . -name "*.log" -type f -delete

echo -e "${GREEN}Cleanup completed!${NC}"