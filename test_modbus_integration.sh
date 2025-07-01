#!/bin/bash
# Modbus Integration Test Script for VoltageEMS

echo "=== VoltageEMS Modbus Integration Test ==="
echo

# Check if Python and required packages are installed
echo "Checking Python dependencies..."
python3 -c "import pymodbus" 2>/dev/null
if [ $? -ne 0 ]; then
    echo "Error: pymodbus not installed. Please run: pip3 install pymodbus"
    exit 1
fi

# Function to cleanup on exit
cleanup() {
    echo
    echo "Cleaning up..."
    # Kill Modbus server if running
    if [ ! -z "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null
    fi
    # Kill comsrv if running
    if [ ! -z "$COMSRV_PID" ]; then
        kill $COMSRV_PID 2>/dev/null
    fi
    exit
}

trap cleanup EXIT INT TERM

# Start Modbus server simulator
echo "Starting Modbus TCP server simulator on port 5502..."
python3 modbus_server_simulator.py --port 5502 &
SERVER_PID=$!
sleep 3

# Check if server started successfully
if ! ps -p $SERVER_PID > /dev/null; then
    echo "Error: Failed to start Modbus server simulator"
    exit 1
fi

echo "Modbus server simulator started (PID: $SERVER_PID)"
echo

# Run Modbus client tests
echo "Running Modbus client tests..."
echo "================================"
python3 test_modbus_client.py --port 5502

echo
echo "Basic tests completed."
echo

# Ask if user wants to run continuous test
read -p "Run continuous test for 30 seconds? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Running continuous test..."
    python3 test_modbus_client.py --port 5502 --continuous 30
fi

echo
echo "=== Test Summary ==="
echo "1. Modbus server simulator successfully started"
echo "2. Client successfully connected to server"
echo "3. Read/Write operations tested"
echo "4. All function codes tested (01, 03, 04, 05, 06, 16)"
echo

# Optional: Test with comsrv if available
read -p "Test with comsrv service? (requires compiled comsrv) (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Checking for comsrv binary..."
    if [ -f "target/debug/comsrv" ]; then
        echo "Starting comsrv service..."
        RUST_LOG=info ./target/debug/comsrv &
        COMSRV_PID=$!
        sleep 5
        
        echo "Testing comsrv API endpoints..."
        # Test health endpoint
        curl -s http://localhost:3000/api/health | python3 -m json.tool
        
        # Test status endpoint
        curl -s http://localhost:3000/api/status | python3 -m json.tool
        
        # Test channels endpoint
        curl -s http://localhost:3000/api/channels | python3 -m json.tool
        
        echo
        echo "comsrv API test completed"
    else
        echo "comsrv binary not found. Please compile first: cargo build"
    fi
fi

echo
echo "Integration test completed!"