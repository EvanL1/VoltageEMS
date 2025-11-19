#!/bin/bash
# Docker Compose V1/V2 compatibility wrapper
# Usage: ./scripts/docker-compose-wrapper.sh [compose-args...]

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# Detect docker compose command
if docker compose version &>/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
    VERSION="V2"
elif command -v docker-compose &>/dev/null; then
    COMPOSE_CMD="docker-compose"
    VERSION="V1"
else
    echo -e "${RED}❌ Error: Neither 'docker compose' (V2) nor 'docker-compose' (V1) found${NC}"
    echo ""
    echo "Please install Docker Compose:"
    echo "  V2 (recommended): https://docs.docker.com/compose/install/"
    echo "  V1 (legacy):      pip install docker-compose"
    exit 1
fi

echo -e "${BLUE}🐳 Using Docker Compose $VERSION: $COMPOSE_CMD${NC}"

# Execute with all arguments
exec $COMPOSE_CMD "$@"
