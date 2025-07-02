#!/bin/bash

# Test comsrv integration with Modbus simulator

echo "=== Comsrv Modbus Integration Test ==="
echo ""

# Step 1: Start Modbus simulator
echo "1. Starting Modbus TCP simulator on port 5020..."
./scripts/start_modbus_simulator.sh --port 5020 > /tmp/modbus_sim.log 2>&1 &
MODBUS_PID=$!
sleep 2

# Check if simulator started
if ! ps -p $MODBUS_PID > /dev/null; then
    echo "❌ Failed to start Modbus simulator"
    exit 1
fi
echo "✅ Modbus simulator started (PID: $MODBUS_PID)"

# Step 2: Test with Python client
echo ""
echo "2. Testing Modbus simulator with Python client..."
python3 tests/test_modbus_client.py --port 5020
if [ $? -eq 0 ]; then
    echo "✅ Python client test passed"
else
    echo "❌ Python client test failed"
    kill $MODBUS_PID
    exit 1
fi

echo ""
echo "3. Modbus simulator is ready for comsrv testing"
echo ""
echo "To test with comsrv:"
echo "  1. In another terminal, run: cargo run --bin comsrv"
echo "  2. Check channel status: curl http://127.0.0.1:3000/api/channels"
echo "  3. View real-time data: curl http://127.0.0.1:3000/api/channels/1001/data"
echo ""
echo "Simulator running on port 5020. Press Ctrl+C to stop..."

# Wait for user interrupt
trap "kill $MODBUS_PID; echo ''; echo 'Modbus simulator stopped.'; exit 0" INT
wait $MODBUS_PID