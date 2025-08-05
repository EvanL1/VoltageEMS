#!/bin/bash
# VoltageEMS Build Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Building VoltageEMS ===${NC}"

# Build mode
BUILD_MODE=${1:-release}

# Clean
echo -e "${YELLOW}Cleaning old builds...${NC}"
cargo clean

# Build
if [ "$BUILD_MODE" = "release" ]; then
    echo -e "${YELLOW}Building Release version...${NC}"
    cargo build --release --workspace
else
    echo -e "${YELLOW}Building Debug version...${NC}"
    cargo build --workspace
fi

# Build Docker images
echo -e "${YELLOW}Building Docker images...${NC}"
for service in comsrv modsrv alarmsrv hissrv rulesrv apigateway; do
    echo -e "Building $service..."
    docker build -t voltageems-$service:latest -f services/$service/Dockerfile .
done

# Build Redis Functions image
echo -e "${YELLOW}Building Redis Functions image...${NC}"
docker build -t voltageems-redis:latest -f scripts/redis-functions/Dockerfile .

echo -e "${GREEN}Build completed!${NC}"