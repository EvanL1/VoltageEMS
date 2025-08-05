#!/bin/bash

# Quick test script for lightweight services

set -e

echo "========================================"
echo "Quick Test for Lightweight Services"
echo "========================================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if Redis is running
echo "Checking Redis..."
if redis-cli ping > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis is running${NC}"
else
    echo -e "${RED}✗ Redis is not running${NC}"
    exit 1
fi

# Kill any existing services
pkill -f rulesrv-lightweight || true
pkill -f modsrv-lightweight || true
sleep 1

# Test rulesrv API
echo -e "\n${YELLOW}Testing rulesrv API...${NC}"

# Check health
echo "1. Health check:"
if curl -s -f "http://localhost:6003/health" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Service is healthy${NC}"
    curl -s http://localhost:6003/health | jq '.'
else
    echo -e "${RED}✗ Service is not running${NC}"
fi

# List rules
echo -e "\n2. List rules:"
curl -s http://localhost:6003/api/v1/rules | jq 'length' | xargs -I {} echo "Found {} rules"

# Get statistics
echo -e "\n3. Get statistics:"
curl -s http://localhost:6003/api/v1/stats | jq '.'

# Execute a single rule
echo -e "\n4. Execute rule 'temp_high_alarm':"
curl -s -X POST http://localhost:6003/api/v1/rules/temp_high_alarm/execute | jq '.'

# Test modsrv API
echo -e "\n${YELLOW}Testing modsrv API...${NC}"

# Check health
echo "1. Health check:"
if curl -s -f "http://localhost:6001/health" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Service is healthy${NC}"
    curl -s http://localhost:6001/health | jq '.'
else
    echo -e "${RED}✗ Service is not running${NC}"
fi

# List templates
echo -e "\n2. List templates:"
curl -s http://localhost:6001/api/v1/templates | jq 'length' | xargs -I {} echo "Found {} templates"

# List models
echo -e "\n3. List models:"
curl -s http://localhost:6001/api/v1/models | jq 'length' | xargs -I {} echo "Found {} models"

# Get model statistics
echo -e "\n4. Get statistics:"
curl -s http://localhost:6001/api/v1/stats | jq '.'

# Get a specific model
echo -e "\n5. Get model 'transformer_01':"
curl -s http://localhost:6001/api/v1/models/transformer_01 | jq '.name, .template'

echo -e "\n${GREEN}Quick test completed!${NC}"
echo "========================================"