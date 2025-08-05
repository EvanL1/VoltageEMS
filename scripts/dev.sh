#!/bin/bash
# VoltageEMS Development Environment Startup Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Starting VoltageEMS Development Environment ===${NC}"

# Start Redis
echo -e "${YELLOW}Starting Redis...${NC}"
docker run -d --name redis-dev -p 6379:6379 redis:8-alpine 2>/dev/null || {
    echo "Redis container already exists, attempting to start..."
    docker start redis-dev
}

# Wait for Redis to start
sleep 2

# Load Redis Functions
echo -e "${YELLOW}Loading Redis Functions...${NC}"
cd scripts/redis-functions && bash load_functions.sh
cd ../..

echo -e "${GREEN}Development environment is ready!${NC}"
echo ""
echo "Available development commands:"
echo "  cargo run --bin comsrv    # Run comsrv"
echo "  cargo run --bin modsrv    # Run modsrv"
echo "  cargo run --bin alarmsrv  # Run alarmsrv"
echo "  cargo run --bin hissrv    # Run hissrv"
echo "  cargo run --bin rulesrv   # Run rulesrv"