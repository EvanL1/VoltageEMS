#!/bin/bash
# End-to-end test script for CSV-based Modbus communication

# Script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== CSV-based Modbus End-to-End Test ===${NC}"
echo ""

# Change to project directory
cd "$PROJECT_DIR"

# Step 1: Check if CSV simulator is running
echo -e "${YELLOW}Step 1: Checking if CSV Modbus simulator is running...${NC}"
if lsof -i:5020 > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Modbus simulator is already running on port 5020${NC}"
    SIMULATOR_PID=$(lsof -ti:5020)
    SIMULATOR_STARTED=false
else
    echo -e "${YELLOW}Starting CSV Modbus simulator...${NC}"
    nohup python3 tests/modbus_csv_simulator.py --port 5020 --debug > /tmp/csv_simulator.log 2>&1 &
    SIMULATOR_PID=$!
    SIMULATOR_STARTED=true
    sleep 3
    
    # Verify it started
    if lsof -i:5020 > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Modbus simulator started successfully (PID: $SIMULATOR_PID)${NC}"
        echo ""
        echo "Simulator log:"
        tail -5 /tmp/csv_simulator.log
    else
        echo -e "${RED}✗ Failed to start Modbus simulator${NC}"
        echo "Error log:"
        cat /tmp/csv_simulator.log
        exit 1
    fi
fi
echo ""

# Step 2: Build the test program
echo -e "${YELLOW}Step 2: Building e2e_csv_test...${NC}"
if cargo build --example e2e_csv_test 2>/dev/null; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    if [ "$SIMULATOR_STARTED" = true ]; then
        kill $SIMULATOR_PID 2>/dev/null
    fi
    exit 1
fi
echo ""

# Step 3: Run the end-to-end test
echo -e "${YELLOW}Step 3: Running end-to-end test...${NC}"
echo ""

# Run the test and capture output
TEST_OUTPUT=$(cargo run --example e2e_csv_test 2>&1)
TEST_EXIT_CODE=$?

# Display the output
echo "$TEST_OUTPUT"
echo ""

# Step 4: Analyze results
echo -e "${YELLOW}Step 4: Test Results Analysis${NC}"

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Test execution completed successfully${NC}"
    
    # Count successful operations
    TELEMETRY_COUNT=$(echo "$TEST_OUTPUT" | grep -c "Point 100[1-8]: value=")
    SIGNAL_COUNT=$(echo "$TEST_OUTPUT" | grep -c "Point 200[1-8]: value=")
    CONTROL_SUCCESS=$(echo "$TEST_OUTPUT" | grep -c "✓ Control command sent successfully")
    ADJUSTMENT_SUCCESS=$(echo "$TEST_OUTPUT" | grep -c "✓ .* set successfully")
    
    echo ""
    echo "Test Statistics:"
    echo "  - Telemetry points read: $TELEMETRY_COUNT/8"
    echo "  - Signal points read: $SIGNAL_COUNT/8"
    echo "  - Control commands sent: $CONTROL_SUCCESS/2"
    echo "  - Adjustment values set: $ADJUSTMENT_SUCCESS/4"
    
    # Check if all tests passed
    if [ $TELEMETRY_COUNT -ge 6 ] && [ $SIGNAL_COUNT -ge 6 ] && \
       [ $CONTROL_SUCCESS -ge 1 ] && [ $ADJUSTMENT_SUCCESS -ge 3 ]; then
        echo ""
        echo -e "${GREEN}🎉 All tests passed!${NC}"
        OVERALL_RESULT=0
    else
        echo ""
        echo -e "${YELLOW}⚠ Some tests had issues${NC}"
        OVERALL_RESULT=1
    fi
else
    echo -e "${RED}✗ Test execution failed${NC}"
    OVERALL_RESULT=1
fi

# Step 5: Show simulator status
echo ""
echo -e "${YELLOW}Step 5: Simulator Status${NC}"
if [ -f /tmp/csv_simulator.log ]; then
    echo "Recent simulator activity:"
    tail -10 /tmp/csv_simulator.log | grep -E "(Client connected|Write|RX:|TX:)" || echo "No recent activity"
fi

# Step 6: Cleanup
echo ""
echo -e "${YELLOW}Step 6: Cleanup${NC}"
if [ "$SIMULATOR_STARTED" = true ]; then
    echo "Stopping CSV Modbus simulator..."
    kill $SIMULATOR_PID 2>/dev/null
    sleep 1
    if ! lsof -i:5020 > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Simulator stopped${NC}"
    else
        echo -e "${YELLOW}⚠ Simulator still running, manual cleanup may be needed${NC}"
    fi
else
    echo "Simulator was already running, not stopping it"
fi

# Final summary
echo ""
echo -e "${GREEN}=== Test Complete ===${NC}"
if [ $OVERALL_RESULT -eq 0 ]; then
    echo -e "${GREEN}✅ CSV-based end-to-end test PASSED${NC}"
else
    echo -e "${RED}❌ CSV-based end-to-end test FAILED${NC}"
fi

exit $OVERALL_RESULT