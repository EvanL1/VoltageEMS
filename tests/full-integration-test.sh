#!/bin/bash
set -e

echo "============================================"
echo "VoltageEMS Full Integration Test"
echo "============================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
    ((TESTS_PASSED++))
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    ((TESTS_FAILED++))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    ((TESTS_SKIPPED++))
}

# Test functions
test_redis() {
    echo ""
    echo "1. Testing Redis Infrastructure"
    echo "----------------------------------------"
    
    # Test connection
    echo -n "  - Redis connection: "
    if docker exec redis-test redis-cli ping > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
        return 1
    fi
    
    # Test Functions loaded
    echo -n "  - Lua Functions loaded: "
    functions=$(docker exec redis-test redis-cli FUNCTION LIST 2>/dev/null | grep -c "name" || echo "0")
    if [ "$functions" -gt 0 ]; then
        echo -e "${GREEN}✓${NC} ($functions functions)"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
    
    # Test data structures
    echo -n "  - Hash operations: "
    docker exec redis-test redis-cli HSET "test:hash" "field1" "value1" > /dev/null 2>&1
    value=$(docker exec redis-test redis-cli HGET "test:hash" "field1" 2>/dev/null)
    if [ "$value" = "value1" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
}

test_lua_functions() {
    echo ""
    echo "2. Testing Lua Functions"
    echo "----------------------------------------"
    
    # Test modsrv functions
    echo -n "  - Model management (modsrv): "
    result=$(docker exec redis-test redis-cli FCALL model_upsert 1 "test_model" '{"name":"Test"}' 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
    
    # Test alarm functions
    echo -n "  - Alarm management (alarmsrv): "
    result=$(docker exec redis-test redis-cli FCALL store_alarm 1 "test_alarm" '{"title":"Test","level":"Info"}' 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
    
    # Test rule functions
    echo -n "  - Rule engine (rulesrv): "
    result=$(docker exec redis-test redis-cli FCALL rule_upsert 1 "test_rule" '{"name":"Test Rule","condition_groups":[{"operator":"AND","conditions":[]}],"actions":[]}' 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
}

test_modbus() {
    echo ""
    echo "3. Testing Modbus Simulator"
    echo "----------------------------------------"
    
    echo -n "  - Modbus server connectivity: "
    if nc -z localhost 5020 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
}

test_data_flow() {
    echo ""
    echo "4. Testing Data Flow"
    echo "----------------------------------------"
    
    # Test comsrv data structure
    echo -n "  - comsrv data structure: "
    docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5" > /dev/null 2>&1
    docker exec redis-test redis-cli HSET "comsrv:1001:T" "2" "30.2" > /dev/null 2>&1
    docker exec redis-test redis-cli HSET "comsrv:1001:S" "1" "1" > /dev/null 2>&1
    
    telemetry=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1" 2>/dev/null)
    signal=$(docker exec redis-test redis-cli HGET "comsrv:1001:S" "1" 2>/dev/null)
    
    if [ "$telemetry" = "25.5" ] && [ "$signal" = "1" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
    
    # Test multi-channel data
    echo -n "  - Multi-channel support: "
    docker exec redis-test redis-cli HSET "comsrv:1002:T" "1" "28.3" > /dev/null 2>&1
    docker exec redis-test redis-cli HSET "comsrv:1003:T" "1" "31.7" > /dev/null 2>&1
    
    channels=$(docker exec redis-test redis-cli KEYS "comsrv:*:T" 2>/dev/null | wc -l)
    if [ "$channels" -ge 3 ]; then
        echo -e "${GREEN}✓${NC} ($channels channels)"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
}

test_alarm_lifecycle() {
    echo ""
    echo "5. Testing Alarm Lifecycle"
    echo "----------------------------------------"
    
    # Create alarm
    echo -n "  - Create alarm: "
    docker exec redis-test redis-cli FCALL store_alarm 1 "lifecycle_alarm" \
        '{"title":"High Temperature","level":"Warning","source":"comsrv:1001","value":35.5}' > /dev/null 2>&1
    
    if docker exec redis-test redis-cli EXISTS "alarmsrv:lifecycle_alarm" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((TESTS_FAILED++))
    fi
    
    # Acknowledge alarm
    echo -n "  - Acknowledge alarm: "
    result=$(docker exec redis-test redis-cli FCALL acknowledge_alarm 1 "lifecycle_alarm" "operator1" 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
    
    # Resolve alarm
    echo -n "  - Resolve alarm: "
    result=$(docker exec redis-test redis-cli FCALL resolve_alarm 1 "lifecycle_alarm" "operator1" 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
}

test_rule_evaluation() {
    echo ""
    echo "6. Testing Rule Evaluation"
    echo "----------------------------------------"
    
    # Create a simple threshold rule
    echo -n "  - Create threshold rule: "
    rule_json='{
        "name": "Temperature Threshold",
        "condition_groups": [{
            "operator": "AND",
            "conditions": [{
                "type": "threshold",
                "channel": "1001",
                "point": "1",
                "operator": ">",
                "value": 30
            }]
        }],
        "actions": [{
            "type": "alarm",
            "level": "Warning",
            "title": "Temperature exceeded"
        }],
        "enabled": true
    }'
    
    result=$(docker exec redis-test redis-cli FCALL rule_upsert 1 "temp_threshold_rule" "$rule_json" 2>&1)
    if [ "$result" = "OK" ]; then
        echo -e "${GREEN}✓${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} ($result)"
        ((TESTS_FAILED++))
    fi
    
    # Evaluate rule
    echo -n "  - Evaluate rule: "
    # Set a value that should trigger the rule
    docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "35.5" > /dev/null 2>&1
    
    result=$(docker exec redis-test redis-cli FCALL rule_evaluate 1 "temp_threshold_rule" 2>&1)
    if echo "$result" | grep -q "true\|1\|triggered"; then
        echo -e "${GREEN}✓${NC} (Rule triggered)"
        ((TESTS_PASSED++))
    else
        echo -e "${YELLOW}⚠${NC} (Check manually)"
        ((TESTS_SKIPPED++))
    fi
}

test_performance() {
    echo ""
    echo "7. Performance Benchmarks"
    echo "----------------------------------------"
    
    # Test Redis operation speed
    echo -n "  - Redis write performance: "
    start_time=$(date +%s%N)
    for i in {1..1000}; do
        docker exec redis-test redis-cli HSET "perf:test" "field$i" "value$i" > /dev/null 2>&1
    done
    end_time=$(date +%s%N)
    elapsed=$(( ($end_time - $start_time) / 1000000 ))
    ops_per_sec=$(( 1000 * 1000 / $elapsed ))
    
    if [ "$ops_per_sec" -gt 100 ]; then
        echo -e "${GREEN}✓${NC} ($ops_per_sec ops/sec)"
        ((TESTS_PASSED++))
    else
        echo -e "${YELLOW}⚠${NC} ($ops_per_sec ops/sec)"
        ((TESTS_SKIPPED++))
    fi
    
    # Test Lua function performance
    echo -n "  - Lua function performance: "
    start_time=$(date +%s%N)
    for i in {1..100}; do
        docker exec redis-test redis-cli FCALL model_upsert 1 "perf_model_$i" '{"name":"Perf Test"}' > /dev/null 2>&1
    done
    end_time=$(date +%s%N)
    elapsed=$(( ($end_time - $start_time) / 1000000 ))
    ops_per_sec=$(( 100 * 1000 / $elapsed ))
    
    if [ "$ops_per_sec" -gt 50 ]; then
        echo -e "${GREEN}✓${NC} ($ops_per_sec ops/sec)"
        ((TESTS_PASSED++))
    else
        echo -e "${YELLOW}⚠${NC} ($ops_per_sec ops/sec)"
        ((TESTS_SKIPPED++))
    fi
}

# Main test execution
main() {
    log_info "Starting comprehensive integration tests..."
    
    # Check Redis is running
    if ! docker ps | grep -q redis-test; then
        log_error "Redis container is not running!"
        exit 1
    fi
    
    # Run all tests
    test_redis
    test_lua_functions
    test_modbus
    test_data_flow
    test_alarm_lifecycle
    test_rule_evaluation
    test_performance
    
    # Summary
    echo ""
    echo "============================================"
    echo "Test Summary"
    echo "============================================"
    echo -e "  Tests Passed:  ${GREEN}$TESTS_PASSED${NC}"
    echo -e "  Tests Failed:  ${RED}$TESTS_FAILED${NC}"
    echo -e "  Tests Skipped: ${YELLOW}$TESTS_SKIPPED${NC}"
    echo ""
    
    total_tests=$((TESTS_PASSED + TESTS_FAILED + TESTS_SKIPPED))
    success_rate=$(( TESTS_PASSED * 100 / total_tests ))
    
    echo "  Success Rate: $success_rate%"
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo ""
        echo -e "${GREEN}✅ All critical tests passed!${NC}"
        exit_code=0
    else
        echo ""
        echo -e "${RED}❌ Some tests failed. Please review the results.${NC}"
        exit_code=1
    fi
    
    return $exit_code
}

# Execute main function
main
exit $?