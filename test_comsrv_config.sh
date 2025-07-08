#!/bin/bash

cd services/comsrv

echo "=== Test 1: Without config center ==="
(./../../target/debug/comsrv --config config/modbus_test.yml 2>&1 | head -10) &
PID=$!
sleep 1
kill $PID 2>/dev/null
wait $PID 2>/dev/null
echo ""

echo "=== Test 2: With config center URL ==="
echo "Setting CONFIG_CENTER_URL=http://localhost:8080"
(CONFIG_CENTER_URL=http://localhost:8080 ./../../target/debug/comsrv --config config/modbus_test.yml 2>&1 | head -15) &
PID=$!
sleep 1
kill $PID 2>/dev/null
wait $PID 2>/dev/null