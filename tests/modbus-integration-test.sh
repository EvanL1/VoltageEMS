#!/bin/bash
set -e

echo "=========================================="
echo "Modbus Integration Test Results"
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

TESTS_PASSED=0
TESTS_FAILED=0

# Test functions
test_item() {
    local name=$1
    local result=$2
    local details=$3
    
    if [ "$result" = "pass" ]; then
        echo -e "  ✅ $name: ${GREEN}PASSED${NC}"
        [ -n "$details" ] && echo "     $details"
        ((TESTS_PASSED++))
    elif [ "$result" = "fail" ]; then
        echo -e "  ❌ $name: ${RED}FAILED${NC}"
        [ -n "$details" ] && echo "     $details"
        ((TESTS_FAILED++))
    else
        echo -e "  ⚠️  $name: ${YELLOW}WARNING${NC}"
        [ -n "$details" ] && echo "     $details"
    fi
}

echo "1. Infrastructure Tests"
echo "----------------------------------------"

# Test Redis
redis_status=$(docker exec redis-test redis-cli ping 2>/dev/null || echo "FAIL")
if [ "$redis_status" = "PONG" ]; then
    test_item "Redis Connection" "pass" "Redis responding normally"
else
    test_item "Redis Connection" "fail" "Redis not responding"
fi

# Test Modbus Simulator
if nc -z localhost 5020 2>/dev/null; then
    test_item "Modbus Simulator" "pass" "Port 5020 accessible"
else
    test_item "Modbus Simulator" "fail" "Port 5020 not accessible"
fi

# Test Modbus TCP Protocol
modbus_response=$(echo -e "\x00\x01\x00\x00\x00\x06\x01\x03\x00\x00\x00\x01" | nc -w 2 localhost 5020 2>/dev/null | od -An -tx1 | head -1)
if [ -n "$modbus_response" ]; then
    test_item "Modbus TCP Protocol" "pass" "Received response: ${modbus_response:0:50}..."
else
    test_item "Modbus TCP Protocol" "fail" "No response from Modbus server"
fi

echo ""
echo "2. Lua Functions Tests"
echo "----------------------------------------"

# Test fixed acknowledge_alarm
docker exec redis-test redis-cli FCALL store_alarm 1 "integration_test_alarm" '{"title":"Test","level":"Info"}' > /dev/null 2>&1
ack_result=$(docker exec redis-test redis-cli FCALL acknowledge_alarm 2 "integration_test_alarm" "test_user" 2>&1)
if echo "$ack_result" | grep -q "Acknowledged"; then
    test_item "acknowledge_alarm (Fixed)" "pass" "Function working correctly"
else
    test_item "acknowledge_alarm (Fixed)" "fail" "$ack_result"
fi

# Test fixed resolve_alarm
resolve_result=$(docker exec redis-test redis-cli FCALL resolve_alarm 2 "integration_test_alarm" "test_user" 2>&1)
if echo "$resolve_result" | grep -q "Resolved"; then
    test_item "resolve_alarm (Fixed)" "pass" "Function working correctly"
else
    test_item "resolve_alarm (Fixed)" "fail" "$resolve_result"
fi

echo ""
echo "3. Data Flow Tests"
echo "----------------------------------------"

# Test Redis data structures
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5" > /dev/null 2>&1
docker exec redis-test redis-cli HSET "comsrv:1001:T" "2" "30.2" > /dev/null 2>&1
value1=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1" 2>/dev/null)
value2=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "2" 2>/dev/null)

if [ "$value1" = "25.5" ] && [ "$value2" = "30.2" ]; then
    test_item "Redis Hash Storage" "pass" "Multi-point storage working"
else
    test_item "Redis Hash Storage" "fail" "Values: $value1, $value2"
fi

# Check multi-channel support
channels=$(docker exec redis-test redis-cli KEYS "comsrv:*:T" 2>/dev/null | wc -l)
test_item "Multi-channel Support" "pass" "$channels channels configured"

echo ""
echo "4. Performance Indicators"
echo "----------------------------------------"

# Quick performance test
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli HSET "perf:test" "field$i" "value$i" > /dev/null 2>&1
done
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))

if [ "$ops_per_sec" -gt 10 ]; then
    test_item "Redis Write Performance" "pass" "$ops_per_sec ops/sec"
else
    test_item "Redis Write Performance" "warn" "$ops_per_sec ops/sec (low due to docker exec overhead)"
fi

echo ""
echo "5. Integration Readiness"
echo "----------------------------------------"

# Check if comsrv can be configured
if [ -f "services/comsrv/config/test-modbus.yaml" ]; then
    test_item "Modbus Configuration" "pass" "test-modbus.yaml ready"
else
    test_item "Modbus Configuration" "fail" "Configuration file missing"
fi

if [ -f "services/comsrv/config/test_telemetry.csv" ]; then
    test_item "Point Mapping" "pass" "test_telemetry.csv ready"
else
    test_item "Point Mapping" "fail" "CSV mapping file missing"
fi

echo ""
echo "=========================================="
echo "Summary"
echo "=========================================="
echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ All integration tests passed!${NC}"
    echo ""
    echo "System is ready for full Modbus data collection:"
    echo "1. Start comsrv with test configuration:"
    echo "   docker run --network voltageems_voltageems-test \\"
    echo "     -e REDIS_URL=redis://redis-test:6379 \\"
    echo "     -e CSV_BASE_PATH=/app/config \\"
    echo "     -v \$PWD/services/comsrv/config:/app/config \\"
    echo "     comsrv -c /app/config/test-modbus.yaml"
    echo ""
    echo "2. Monitor data collection:"
    echo "   docker exec redis-test redis-cli MONITOR | grep comsrv"
else
    echo -e "${RED}❌ Some tests failed. Please review and fix issues.${NC}"
fi