#!/bin/bash
set -e

echo "=========================================="
echo "VoltageEMS Integration Test"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

test_endpoint() {
    local name=$1
    local url=$2
    local expected_status=$3
    
    echo -n "Testing $name... "
    status=$(curl -s -o /dev/null -w "%{http_code}" $url 2>/dev/null || echo "000")
    
    if [ "$status" = "$expected_status" ]; then
        echo -e "${GREEN}✓${NC} (Status: $status)"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}✗${NC} (Expected: $expected_status, Got: $status)"
        ((TESTS_FAILED++))
        return 1
    fi
}

test_redis_function() {
    local name=$1
    local command=$2
    
    echo -n "Testing Redis function: $name... "
    result=$(docker exec redis-test redis-cli $command 2>&1)
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
        return 1
    fi
}

# Clean up previous test environment
log_info "Cleaning up previous test environment..."
docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true

# Build and start services
log_info "Building Docker images..."
docker-compose -f docker-compose.test.yml build

log_info "Starting test environment..."
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be ready
log_info "Waiting for services to be ready..."
sleep 10

# Check if Redis is running and has functions loaded
log_info "Checking Redis status..."
docker exec redis-test redis-cli ping

echo ""
echo "=========================================="
echo "Running Integration Tests"
echo "=========================================="

# Test 1: Service Health Endpoints
echo ""
echo "1. Testing Service Health Endpoints"
echo "----------------------------------------"
test_endpoint "comsrv health" "http://localhost:6000/health" "200"
test_endpoint "modsrv health" "http://localhost:6001/health" "200"
test_endpoint "alarmsrv health" "http://localhost:6002/health" "200"
test_endpoint "rulesrv health" "http://localhost:6003/health" "200"
test_endpoint "hissrv health" "http://localhost:6004/health" "200"
test_endpoint "apigateway health" "http://localhost:6005/health" "200"

# Test 2: Redis Lua Functions
echo ""
echo "2. Testing Redis Lua Functions"
echo "----------------------------------------"
test_redis_function "model_create" "FCALL model_create 1 test_model '{\"name\":\"Test Model\"}'"
test_redis_function "store_alarm" "FCALL store_alarm 1 test_alarm '{\"title\":\"Test Alarm\",\"level\":\"Warning\"}'"

# Test 3: Service APIs
echo ""
echo "3. Testing Service APIs"
echo "----------------------------------------"

# Test modsrv API
echo -n "Testing modsrv model creation... "
response=$(curl -s -X POST http://localhost:6001/models \
    -H "Content-Type: application/json" \
    -d '{"id":"model_001","name":"Test Model","tags":["test"]}' 2>/dev/null)
if echo "$response" | grep -q "model_001"; then
    echo -e "${GREEN}✓${NC}"
    ((TESTS_PASSED++))
else
    echo -e "${RED}✗${NC}"
    ((TESTS_FAILED++))
fi

# Test alarmsrv API
echo -n "Testing alarmsrv alarm creation... "
response=$(curl -s -X POST http://localhost:6002/alarms \
    -H "Content-Type: application/json" \
    -d '{"title":"Integration Test Alarm","description":"Test","level":"Info"}' 2>/dev/null)
if echo "$response" | grep -q "alarm_id"; then
    echo -e "${GREEN}✓${NC}"
    ((TESTS_PASSED++))
else
    echo -e "${RED}✗${NC}"
    ((TESTS_FAILED++))
fi

# Test 4: Data Flow
echo ""
echo "4. Testing Data Flow"
echo "----------------------------------------"

# Write data to Redis and verify
echo -n "Testing Redis data persistence... "
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5" > /dev/null
value=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1")
if [ "$value" = "25.5" ]; then
    echo -e "${GREEN}✓${NC}"
    ((TESTS_PASSED++))
else
    echo -e "${RED}✗${NC}"
    ((TESTS_FAILED++))
fi

# Test 5: Service Logs
echo ""
echo "5. Checking Service Logs for Errors"
echo "----------------------------------------"
for service in comsrv modsrv alarmsrv rulesrv hissrv apigateway; do
    echo -n "Checking $service logs... "
    errors=$(docker logs ${service}-test 2>&1 | grep -i error | wc -l)
    if [ "$errors" -eq 0 ]; then
        echo -e "${GREEN}✓${NC} (No errors)"
        ((TESTS_PASSED++))
    else
        echo -e "${YELLOW}⚠${NC} ($errors errors found)"
        ((TESTS_FAILED++))
    fi
done

# Summary
echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit_code=0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit_code=1
fi

# Cleanup option
echo ""
read -p "Do you want to keep the test environment running? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "Stopping test environment..."
    docker-compose -f docker-compose.test.yml down
fi

exit $exit_code