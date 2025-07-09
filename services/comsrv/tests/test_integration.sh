#!/bin/bash

# Test comsrv integration with Modbus

echo "Starting comsrv in background..."
cargo run --bin comsrv -- --config config/modbus_test.yaml &
COMSRV_PID=$!

# Wait for comsrv to start
echo "Waiting for comsrv to start..."
sleep 5

# Check if comsrv is running
if ! ps -p $COMSRV_PID > /dev/null; then
    echo "comsrv failed to start!"
    exit 1
fi

echo "comsrv is running with PID: $COMSRV_PID"

# Test API status
echo "Testing API status..."
curl -s http://localhost:8090/api/v1/status | jq . || echo "API not available on port 8090"
curl -s http://localhost:3000/api/v1/status | jq . || echo "API not available on port 3000"

# Test channel status
echo "Testing channel status..."
curl -s http://localhost:8090/api/v1/channels | jq . || echo "Failed to get channels"

# Test reading a point
echo "Testing reading point 1..."
curl -s http://localhost:8090/api/v1/channels/1/points/1 | jq . || echo "Failed to read point"

# Keep running for monitoring
echo "Press Ctrl+C to stop..."
trap "kill $COMSRV_PID; exit" INT TERM
wait $COMSRV_PID