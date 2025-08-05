#!/bin/bash

# Test script for lightweight rulesrv and modsrv services

set -e

echo "========================================"
echo "Testing Lightweight Services"
echo "========================================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if service is healthy
check_service_health() {
    local service_name=$1
    local port=$2
    
    echo -n "Checking $service_name health on port $port... "
    
    if curl -s -f "http://localhost:$port/health" > /dev/null; then
        echo -e "${GREEN}✓ Healthy${NC}"
        return 0
    else
        echo -e "${RED}✗ Failed${NC}"
        return 1
    fi
}

# Function to load Redis functions
load_redis_functions() {
    echo "Loading Redis Functions..."
    cd scripts/redis-functions
    
    # Load rulesrv functions
    if redis-cli -x FUNCTION LOAD REPLACE < rulesrv.lua > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Loaded rulesrv functions${NC}"
    else
        echo -e "${RED}✗ Failed to load rulesrv functions${NC}"
    fi
    
    # Load modsrv functions  
    if redis-cli -x FUNCTION LOAD REPLACE < modsrv.lua > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Loaded modsrv functions${NC}"
    else
        echo -e "${RED}✗ Failed to load modsrv functions${NC}"
    fi
    
    cd ../..
}

# Check if Redis is running
echo "Checking Redis..."
if redis-cli ping > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis is running${NC}"
else
    echo -e "${RED}✗ Redis is not running${NC}"
    echo "Starting Redis..."
    docker run -d --name redis-test -p 6379:6379 redis:8-alpine
    sleep 2
fi

# Load Redis functions
load_redis_functions

# Build the lightweight services
echo -e "\n${YELLOW}Building lightweight services...${NC}"
cargo build --bin rulesrv-lightweight --release
cargo build --bin modsrv-lightweight --release

# Start rulesrv-lightweight
echo -e "\n${YELLOW}Starting rulesrv-lightweight...${NC}"
RUST_LOG=info ./target/release/rulesrv-lightweight services/rulesrv/config/rules.yaml &
RULESRV_PID=$!
sleep 3

# Start modsrv-lightweight
echo -e "\n${YELLOW}Starting modsrv-lightweight...${NC}"
RUST_LOG=info ./target/release/modsrv-lightweight services/modsrv/config/models.yaml &
MODSRV_PID=$!
sleep 3

# Check service health
echo -e "\n${YELLOW}Checking service health...${NC}"
check_service_health "rulesrv" 6003
check_service_health "modsrv" 6001

# Test rulesrv API
echo -e "\n${YELLOW}Testing rulesrv API...${NC}"

# List rules
echo "1. List rules:"
curl -s http://localhost:6003/api/v1/rules | jq '.[0].id' || echo "No rules found"

# Get statistics
echo -e "\n2. Get statistics:"
curl -s http://localhost:6003/api/v1/stats | jq '.'

# Execute all rules
echo -e "\n3. Execute all rules:"
curl -s -X POST http://localhost:6003/api/v1/rules/execute | jq '.'

# Test modsrv API
echo -e "\n${YELLOW}Testing modsrv API...${NC}"

# List templates
echo "1. List templates:"
curl -s http://localhost:6001/api/v1/templates | jq '.[0].id' || echo "No templates found"

# List models
echo -e "\n2. List models:"
curl -s http://localhost:6001/api/v1/models | jq '.[0].id' || echo "No models found"

# Get model statistics
echo -e "\n3. Get statistics:"
curl -s http://localhost:6001/api/v1/stats | jq '.'

# Test model value operations (if models exist)
FIRST_MODEL=$(curl -s http://localhost:6001/api/v1/models | jq -r '.[0].id' 2>/dev/null)
if [ "$FIRST_MODEL" != "null" ] && [ -n "$FIRST_MODEL" ]; then
    echo -e "\n4. Get model value for $FIRST_MODEL:"
    curl -s http://localhost:6001/api/v1/models/$FIRST_MODEL/values/voltage_a 2>/dev/null | jq '.' || echo "No value found"
fi

# Cleanup
echo -e "\n${YELLOW}Cleaning up...${NC}"
echo "Stopping services..."
kill $RULESRV_PID 2>/dev/null || true
kill $MODSRV_PID 2>/dev/null || true

echo -e "\n${GREEN}Test completed!${NC}"
echo "========================================"

# Optional: Keep Redis running for manual testing
echo -e "\n${YELLOW}Redis is still running for manual testing.${NC}"
echo "To stop Redis: docker stop redis-test && docker rm redis-test"
echo "To view Redis data: redis-cli"