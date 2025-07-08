#!/bin/bash

echo "========================================="
echo "Testing ComsRV Configuration Center Integration"
echo "========================================="

# Build first
echo "Building comsrv..."
cargo build --bin comsrv 2>&1 > /dev/null

# Test 1: Normal operation without config center
echo -e "\nTest 1: Running without config center..."
unset CONFIG_CENTER_URL
./target/debug/comsrv --config config/modbus_test.yml 2>&1 | head -10 &
PID1=$!
sleep 2
kill $PID1 2>/dev/null

# Test 2: With config center URL (should fail but fallback)
echo -e "\nTest 2: Running with unavailable config center..."
export CONFIG_CENTER_URL=http://nonexistent.example.com
export RUST_LOG=info,comsrv::core::config=debug
./target/debug/comsrv --config config/modbus_test.yml 2>&1 | head -20 | grep -i "config" &
PID2=$!
sleep 2
kill $PID2 2>/dev/null

# Test 3: Show environment logs
echo -e "\nTest 3: Checking logs for config center detection..."
export CONFIG_CENTER_URL=http://test-config-center:8080
export RUST_LOG=debug
./target/debug/comsrv --config config/modbus_test.yml 2>&1 | grep -i "config" | head -10 &
PID3=$!
sleep 2
kill $PID3 2>/dev/null

echo -e "\nTest completed."