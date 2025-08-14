#!/bin/bash

# Test script for Modbus reconnection mechanism
echo "=== Testing Modbus Reconnection Mechanism ==="
echo ""
echo "This script will test the automatic reconnection feature."
echo "Prerequisites:"
echo "  1. comsrv is running with a Modbus TCP channel"
echo "  2. Modbus simulator is accessible"
echo ""
echo "Test Steps:"
echo "  1. Start monitoring comsrv logs: docker logs -f voltageems-comsrv"
echo "  2. Stop the Modbus simulator: docker stop voltageems-modbus-sim"
echo "  3. Watch for reconnection attempts in the logs"
echo "  4. Restart the simulator: docker start voltageems-modbus-sim"
echo "  5. Verify automatic reconnection"
echo ""
echo "Expected Results:"
echo "  - Connection lost detection: 'Broken pipe' or 'Connection reset'"
echo "  - Reconnection attempts: 'attempting reconnection...'"
echo "  - Exponential backoff: delays of 1s, 2s, 4s, 8s, 16s, 30s"
echo "  - Successful reconnection: 'reconnected successfully'"
echo ""
echo "Configuration in comsrv.yaml:"
echo "  polling:"
echo "    reconnect_enabled: true"
echo "    reconnect_retries: 5"
echo "    reconnect_delay_ms: 1000"
echo ""
echo "Press Enter to start the test..."
read

# Monitor logs in background
echo "Starting log monitoring..."
docker logs -f voltageems-comsrv 2>&1 | grep -E "(reconnect|Broken pipe|Connection)" &
LOG_PID=$!

# Wait for user to manually test
echo ""
echo "Now manually:"
echo "1. Stop the Modbus simulator"
echo "2. Wait for reconnection attempts"
echo "3. Restart the simulator"
echo "4. Verify reconnection"
echo ""
echo "Press Enter when test is complete..."
read

# Clean up
kill $LOG_PID 2>/dev/null

echo "Test completed!"