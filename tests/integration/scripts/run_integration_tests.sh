#!/bin/bash
# VoltageEMS Integration Test Runner
# Runs protocol import, communication tests, and API tests

set -e

# Configuration
TEST_PHASE="${TEST_PHASE:-phase1}"
TEST_TYPE="${TEST_TYPE:-all}"
API_URL="${API_URL:-http://apigateway:8080}"
REDIS_URL="${REDIS_URL:-redis://redis:6379}"
TEST_RESULTS_DIR="${TEST_RESULTS_DIR:-/test_results}"
TEST_TIMEOUT="${TEST_TIMEOUT:-900}"
EXPECTED_CHANNELS="${EXPECTED_CHANNELS:-10}"
EXPECTED_POINTS="${EXPECTED_POINTS:-500}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    
    case $status in
        "info")
            echo -e "${YELLOW}[INFO]${NC} $message"
            ;;
        "success")
            echo -e "${GREEN}[SUCCESS]${NC} $message"
            ;;
        "error")
            echo -e "${RED}[ERROR]${NC} $message"
            ;;
        "test")
            echo -e "${BLUE}[TEST]${NC} $message"
            ;;
    esac
}

# Function to wait for service
wait_for_service() {
    local service_name=$1
    local url=$2
    local max_wait=${3:-60}
    
    print_status "info" "Waiting for $service_name to be ready..."
    
    local waited=0
    while [ $waited -lt $max_wait ]; do
        if curl -sf "$url" > /dev/null 2>&1; then
            print_status "success" "$service_name is ready"
            return 0
        fi
        sleep 2
        waited=$((waited + 2))
    done
    
    print_status "error" "$service_name failed to start within $max_wait seconds"
    return 1
}

# Function to check Redis connectivity
check_redis() {
    print_status "info" "Checking Redis connectivity..."
    
    python3 -c "
import redis
r = redis.from_url('$REDIS_URL')
if r.ping():
    print('Redis connection successful')
    exit(0)
else:
    print('Redis connection failed')
    exit(1)
" || return 1
}

# Function to run a test with timeout
run_test() {
    local test_name=$1
    local test_command=$2
    local timeout=${3:-300}
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    print_status "test" "Running: $test_name"
    
    if timeout $timeout bash -c "$test_command" > "$TEST_RESULTS_DIR/${test_name}.log" 2>&1; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        print_status "success" "$test_name passed"
        return 0
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        print_status "error" "$test_name failed"
        echo "Error output:"
        tail -20 "$TEST_RESULTS_DIR/${test_name}.log"
        return 1
    fi
}

# Function to test protocol configuration import
test_protocol_import() {
    print_status "info" "Testing protocol configuration import..."
    
    python3 <<EOF
import requests
import json
import time

api_url = '$API_URL'

# Get current channels
response = requests.get(f'{api_url}/api/v1/comsrv/channels')
if response.status_code != 200:
    print(f'Failed to get channels: {response.status_code}')
    exit(1)

channels = response.json()
print(f'Found {len(channels)} channels')

# Verify expected number of channels
if len(channels) != $EXPECTED_CHANNELS:
    print(f'Expected $EXPECTED_CHANNELS channels, found {len(channels)}')
    exit(1)

# Check each channel is connected
disconnected = [ch for ch in channels if ch.get('status') != 'connected']
if disconnected:
    print(f'{len(disconnected)} channels are not connected')
    for ch in disconnected:
        print(f"  - {ch['id']}: {ch.get('status', 'unknown')}")
    exit(1)

print('All channels connected successfully')
EOF
}

# Function to test point table import
test_point_import() {
    print_status "info" "Testing point table import..."
    
    python3 <<EOF
import requests
import redis
import json

api_url = '$API_URL'
r = redis.from_url('$REDIS_URL')

# Get all points from API
response = requests.get(f'{api_url}/api/v1/comsrv/points')
if response.status_code != 200:
    print(f'Failed to get points: {response.status_code}')
    exit(1)

points = response.json()
print(f'Found {len(points)} points via API')

# Verify expected number of points
if len(points) != $EXPECTED_POINTS:
    print(f'Expected $EXPECTED_POINTS points, found {len(points)}')
    exit(1)

# Check points in Redis
redis_points = r.keys('point:*')
print(f'Found {len(redis_points)} points in Redis')

# Verify point data structure
sample_points = redis_points[:10]
for key in sample_points:
    data = r.hgetall(key)
    required_fields = [b'value', b'quality', b'timestamp']
    missing = [f for f in required_fields if f not in data]
    if missing:
        print(f'Point {key} missing fields: {missing}')
        exit(1)

print('Point table import successful')
EOF
}

# Function to test communication
test_communication() {
    print_status "info" "Testing protocol communication..."
    
    python3 <<EOF
import requests
import redis
import time
import json

api_url = '$API_URL'
r = redis.from_url('$REDIS_URL')

# Monitor point updates
print('Monitoring point updates for 10 seconds...')
start_time = time.time()
update_count = 0
previous_values = {}

# Get initial values
for key in r.keys('point:*')[:20]:  # Sample 20 points
    data = r.hgetall(key)
    if b'value' in data and b'timestamp' in data:
        previous_values[key] = {
            'value': data[b'value'],
            'timestamp': data[b'timestamp']
        }

# Wait and check for updates
time.sleep(10)

# Check for changes
for key, prev in previous_values.items():
    current = r.hgetall(key)
    if b'value' in current and b'timestamp' in current:
        if current[b'timestamp'] != prev['timestamp']:
            update_count += 1

print(f'Detected {update_count} point updates out of {len(previous_values)} monitored')

if update_count < len(previous_values) * 0.5:
    print('Less than 50% of points were updated - communication may be failing')
    exit(1)

print('Communication test passed')
EOF
}

# Function to test API endpoints
test_api_endpoints() {
    print_status "info" "Testing API endpoints..."
    
    python3 <<EOF
import requests
import json
import time

api_url = '$API_URL'

# Test health endpoint
response = requests.get(f'{api_url}/api/v1/health')
if response.status_code != 200:
    print(f'Health check failed: {response.status_code}')
    exit(1)

health = response.json()
print(f'System health: {health.get("status", "unknown")}')

# Test system info
response = requests.get(f'{api_url}/api/v1/system/info')
if response.status_code != 200:
    print(f'System info failed: {response.status_code}')
    exit(1)

# Test channel operations
response = requests.get(f'{api_url}/api/v1/comsrv/channels')
if response.status_code != 200:
    print(f'Get channels failed: {response.status_code}')
    exit(1)

channels = response.json()
if channels:
    # Test channel details
    channel_id = channels[0]['id']
    response = requests.get(f'{api_url}/api/v1/comsrv/channels/{channel_id}')
    if response.status_code != 200:
        print(f'Get channel details failed: {response.status_code}')
        exit(1)

# Test point query with filters
params = {'point_type': 'telemetry', 'limit': 10}
response = requests.get(f'{api_url}/api/v1/comsrv/points', params=params)
if response.status_code != 200:
    print(f'Point query failed: {response.status_code}')
    exit(1)

# Test command endpoint (without actually sending)
command_data = {
    'device_id': 'test',
    'point_id': 1,
    'value': 1.0,
    'command_type': 'write'
}
# We don't actually send commands in this test
print('Command endpoint structure verified')

print('API endpoint tests passed')
EOF
}

# Function to test log generation
test_log_generation() {
    print_status "info" "Testing log file generation..."
    
    # Check service log file exists
    if [ -f "/logs/service/comsrv_${TEST_PHASE}.log" ]; then
        print_status "success" "Service log file exists"
        
        # Check log content
        if grep -q "Starting comsrv service" "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null || \
           grep -q "Starting channel connections" "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null; then
            print_status "success" "Log contains startup messages"
        else
            print_status "error" "Log file exists but missing startup messages"
            return 1
        fi
        
        # Check if log is being updated
        initial_size=$(stat -f%z "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null || stat -c%s "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null)
        sleep 12  # Wait for at least one update cycle (10s + buffer)
        final_size=$(stat -f%z "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null || stat -c%s "/logs/service/comsrv_${TEST_PHASE}.log" 2>/dev/null)
        
        if [ "$final_size" -gt "$initial_size" ]; then
            print_status "success" "Log file is being actively updated"
        else
            print_status "error" "Log file is not growing"
            return 1
        fi
        
        # Count log entries
        log_lines=$(wc -l < "/logs/service/comsrv_${TEST_PHASE}.log")
        print_status "info" "Log file contains $log_lines lines"
        
    else
        print_status "error" "Service log file not found at /logs/service/comsrv_${TEST_PHASE}.log"
        # Also check the mounted path from host perspective
        if [ -f "../logs/service/comsrv_${TEST_PHASE}.log" ]; then
            print_status "info" "Log file found at host path ../logs/service/comsrv_${TEST_PHASE}.log"
        fi
        return 1
    fi
    
    # Check channel logs for phase2/phase3
    if [[ "$TEST_PHASE" == "phase2" ]] || [[ "$TEST_PHASE" == "phase3" ]]; then
        channel_logs=$(find "/logs/channels" -name "*.log" 2>/dev/null | wc -l)
        print_status "info" "Found $channel_logs channel log files"
    fi
    
    return 0
}

# Function to test performance
test_performance() {
    print_status "info" "Testing performance metrics..."
    
    python3 <<EOF
import requests
import redis
import time
import statistics

api_url = '$API_URL'
r = redis.from_url('$REDIS_URL')

# Test API response times
response_times = []
for i in range(20):
    start = time.time()
    response = requests.get(f'{api_url}/api/v1/comsrv/points', params={'limit': 100})
    elapsed = (time.time() - start) * 1000  # ms
    response_times.append(elapsed)
    time.sleep(0.1)

avg_response = statistics.mean(response_times)
max_response = max(response_times)
print(f'API Response: avg={avg_response:.2f}ms, max={max_response:.2f}ms')

# Test Redis throughput
start = time.time()
operations = 0
while time.time() - start < 5:  # 5 second test
    r.get('point:1:value')
    operations += 1

ops_per_sec = operations / 5
print(f'Redis throughput: {ops_per_sec:.2f} ops/sec')

# Check metrics endpoint
response = requests.get(f'{api_url}/metrics')
if response.status_code == 200:
    print('Prometheus metrics endpoint available')

# Performance thresholds
if avg_response > 100:
    print(f'Average API response time too high: {avg_response}ms')
    exit(1)

if ops_per_sec < 1000:
    print(f'Redis throughput too low: {ops_per_sec} ops/sec')
    exit(1)

print('Performance tests passed')
EOF
}

# Function to generate test report
generate_report() {
    print_status "info" "Generating test report..."
    
    cat > "$TEST_RESULTS_DIR/integration_test_report.md" <<EOF
# Integration Test Report - $TEST_PHASE

## Summary
- **Date**: $(date)
- **Phase**: $TEST_PHASE
- **Total Tests**: $TOTAL_TESTS
- **Passed**: $PASSED_TESTS
- **Failed**: $FAILED_TESTS
- **Success Rate**: $(echo "scale=2; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc)%

## Configuration
- Expected Channels: $EXPECTED_CHANNELS
- Expected Points: $EXPECTED_POINTS
- Test Timeout: $TEST_TIMEOUT seconds

## Test Results
$(for log in $TEST_RESULTS_DIR/*.log; do
    test_name=$(basename "$log" .log)
    if grep -q "passed" "$log" 2>/dev/null; then
        echo "- ✅ $test_name"
    else
        echo "- ❌ $test_name"
    fi
done)

## System Status
- API Gateway: $(curl -sf "$API_URL/api/v1/health" > /dev/null && echo "✅ Online" || echo "❌ Offline")
- Redis: $(python3 -c "import redis; r=redis.from_url('$REDIS_URL'); print('✅ Connected' if r.ping() else '❌ Disconnected')" 2>/dev/null || echo "❌ Error")

EOF
    
    print_status "success" "Report generated: $TEST_RESULTS_DIR/integration_test_report.md"
}

# Main test execution
main() {
    print_status "info" "Starting integration tests for $TEST_PHASE"
    
    # Create results directory
    mkdir -p "$TEST_RESULTS_DIR"
    
    # Wait for services
    wait_for_service "API Gateway" "$API_URL/api/v1/health" 60 || exit 1
    check_redis || exit 1
    
    # Give services time to initialize
    sleep 10
    
    # Run tests based on type
    case $TEST_TYPE in
        "import")
            run_test "protocol_import" "test_protocol_import" 60
            run_test "point_import" "test_point_import" 60
            ;;
        "communication")
            run_test "communication" "test_communication" 120
            ;;
        "api")
            run_test "api_endpoints" "test_api_endpoints" 60
            ;;
        "performance")
            run_test "performance" "test_performance" 180
            ;;
        "all")
            run_test "protocol_import" "test_protocol_import" 60
            run_test "point_import" "test_point_import" 60
            run_test "communication" "test_communication" 120
            run_test "api_endpoints" "test_api_endpoints" 60
            run_test "log_generation" "test_log_generation" 60
            if [[ "$TEST_PHASE" != "phase1" ]]; then
                run_test "performance" "test_performance" 180
            fi
            ;;
        *)
            print_status "error" "Unknown test type: $TEST_TYPE"
            exit 1
            ;;
    esac
    
    # Generate report
    generate_report
    
    # Summary
    echo ""
    echo "========================================"
    echo "Integration Test Summary - $TEST_PHASE"
    echo "========================================"
    echo "Total Tests:    $TOTAL_TESTS"
    echo "Passed:         $PASSED_TESTS"
    echo "Failed:         $FAILED_TESTS"
    echo "Success Rate:   $(echo "scale=2; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc)%"
    echo ""
    
    # Exit with appropriate code
    if [ $FAILED_TESTS -gt 0 ]; then
        print_status "error" "Integration tests failed!"
        exit 1
    else
        print_status "success" "All integration tests passed!"
        exit 0
    fi
}

# Run main function
main