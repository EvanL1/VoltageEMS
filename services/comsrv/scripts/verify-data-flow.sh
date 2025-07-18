#!/bin/bash
# Verify data flow in comsrv test environment

set -e

echo "=== Verifying ComSrv Data Flow ==="

# Check if containers are running
echo "1. Checking container status..."
docker ps | grep -E "(comsrv|redis|modbus)" || {
    echo "ERROR: Required containers are not running!"
    echo "Please run start-test-env.sh first"
    exit 1
}

# Check comsrv health
echo ""
echo "2. Checking ComSrv health..."
curl -s http://localhost:3000/api/health | jq . || echo "ComSrv API not responding"

# Check channel status
echo ""
echo "3. Checking channel status..."
curl -s http://localhost:3000/api/channels | jq . || echo "Failed to get channels"

# Check Redis data
echo ""
echo "4. Checking Redis data..."
echo "Telemetry data (channel 1001):"
docker exec comsrv-redis-test redis-cli --user comsrv --pass comsrv_secure_password_2025 keys "1001:m:*" | head -10

echo ""
echo "Sample telemetry values:"
docker exec comsrv-redis-test redis-cli --user comsrv --pass comsrv_secure_password_2025 mget "1001:m:10001" "1001:m:10002" "1001:m:10003"

# Check polling logs
echo ""
echo "5. Recent polling activity (last 20 lines):"
docker logs comsrv-test 2>&1 | grep -i "polling\|modbus" | tail -20

# Test ACL permissions
echo ""
echo "6. Testing ACL permissions..."
echo "Testing readonly user (should fail to write):"
docker exec comsrv-redis-test redis-cli --user readonly --pass readonly_password_2025 set "test:key" "value" 2>&1 | grep -E "(NOPERM|denied)" && echo "✓ Read-only user correctly denied write access" || echo "✗ ACL test failed"

echo ""
echo "=== Data Flow Verification Complete ==="