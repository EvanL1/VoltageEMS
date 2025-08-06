#!/bin/bash
set -e

echo "=========================================="
echo "VoltageEMS Quick Test"  
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Step 1: Build and start Redis with Lua functions
log_info "Building Redis image with Lua functions..."
docker-compose -f docker-compose.test.yml build redis

log_info "Starting Redis..."
docker-compose -f docker-compose.test.yml up -d redis

# Wait for Redis to be ready
log_info "Waiting for Redis to be ready..."
sleep 5

# Step 2: Test Redis connection
log_info "Testing Redis connection..."
docker exec redis-test redis-cli ping

# Step 3: Check loaded functions
log_info "Checking loaded Lua functions..."
docker exec redis-test redis-cli FUNCTION LIST 2>/dev/null || echo "Function list not available, trying alternative method..."

# Step 4: Test basic Redis operations
log_info "Testing basic Redis operations..."
docker exec redis-test redis-cli SET test_key "test_value"
result=$(docker exec redis-test redis-cli GET test_key)
if [ "$result" = "test_value" ]; then
    echo -e "${GREEN}✓${NC} Redis basic operations working"
else
    echo -e "${RED}✗${NC} Redis basic operations failed"
fi

# Step 5: Test data structure for VoltageEMS
log_info "Testing VoltageEMS data structure..."
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5"
docker exec redis-test redis-cli HSET "comsrv:1001:T" "2" "30.2"
value=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1")
if [ "$value" = "25.5" ]; then
    echo -e "${GREEN}✓${NC} VoltageEMS data structure working"
else
    echo -e "${RED}✗${NC} VoltageEMS data structure failed"
fi

# Step 6: Test a single service (modsrv)
log_info "Building modsrv..."
docker-compose -f docker-compose.test.yml build modsrv

log_info "Starting modsrv..."
docker-compose -f docker-compose.test.yml up -d modsrv

# Wait for service to start
sleep 5

# Test health endpoint
log_info "Testing modsrv health endpoint..."
status=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:6001/health 2>/dev/null || echo "000")
if [ "$status" = "200" ]; then
    echo -e "${GREEN}✓${NC} modsrv is healthy"
else
    echo -e "${YELLOW}⚠${NC} modsrv health check returned: $status"
fi

# Show service logs
log_info "modsrv logs:"
docker logs modsrv-test 2>&1 | tail -10

echo ""
echo "=========================================="
echo "Quick Test Complete"
echo "=========================================="

# Cleanup option
echo ""
read -p "Do you want to keep the test environment running? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "Stopping test environment..."
    docker-compose -f docker-compose.test.yml down
fi