#!/bin/bash
# VoltageEMS Test Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Running VoltageEMS Tests ===${NC}"

# Ensure Redis is running
if ! docker ps | grep -q redis-dev; then
    echo -e "${YELLOW}Starting test Redis...${NC}"
    docker run -d --name redis-test -p 6380:6379 redis:8-alpine
    sleep 2
fi

# Load Redis Functions
echo -e "${YELLOW}Loading Redis Functions...${NC}"
REDIS_PORT=6380 cd scripts/redis-functions && bash load_functions.sh
cd ../..

# Run unit tests
echo -e "${YELLOW}Running unit tests...${NC}"
cargo test --workspace -- --nocapture

# Run integration tests
echo -e "${YELLOW}Running integration tests...${NC}"
cargo test --workspace --features integration-tests -- --nocapture

# Clean up test Redis
echo -e "${YELLOW}Cleaning test environment...${NC}"
docker stop redis-test && docker rm redis-test

echo -e "${GREEN}Tests completed!${NC}"