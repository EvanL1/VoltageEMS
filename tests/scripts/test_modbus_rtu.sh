#!/bin/bash
# Modbus RTU Protocol Test Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "================================"
echo "Modbus RTU Protocol Test"
echo "================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

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
    esac
}

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    print_status "error" "Python3 is required but not installed"
    exit 1
fi

# Check if pymodbus is installed
if ! python3 -c "import pymodbus" &> /dev/null; then
    print_status "info" "Installing pymodbus..."
    pip3 install pymodbus pyserial
fi

# Check if socat is available (for virtual serial ports)
if ! command -v socat &> /dev/null; then
    print_status "error" "socat is required for virtual serial ports"
    print_status "info" "Install with: brew install socat (macOS) or apt-get install socat (Linux)"
    exit 1
fi

# Start Modbus RTU simulator with virtual serial port
print_status "info" "Starting Modbus RTU simulator with virtual serial port..."
python3 "${PROJECT_ROOT}/tests/simulators/modbus_rtu_simulator.py" \
    --create-virtual \
    --baudrate 9600 \
    --slave-id 1 \
    --update-interval 0.1 &
SIMULATOR_PID=$!

# Wait for simulator and virtual ports to start
sleep 5

# Check if simulator is running
if ! kill -0 $SIMULATOR_PID 2>/dev/null; then
    print_status "error" "Failed to start Modbus RTU simulator"
    exit 1
fi

print_status "success" "Modbus RTU simulator started (PID: $SIMULATOR_PID)"
print_status "info" "Virtual serial ports created:"
print_status "info" "  Master: /tmp/modbus_rtu_master"
print_status "info" "  Slave: /tmp/modbus_rtu_slave"

# Function to cleanup on exit
cleanup() {
    print_status "info" "Cleaning up..."
    
    if [[ -n $SIMULATOR_PID ]] && kill -0 $SIMULATOR_PID 2>/dev/null; then
        kill $SIMULATOR_PID
        print_status "info" "Stopped Modbus RTU simulator"
    fi
    
    # Clean up virtual serial ports
    rm -f /tmp/modbus_rtu_master /tmp/modbus_rtu_slave
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Run the Rust tests
print_status "info" "Running Modbus RTU tests..."

cd "${PROJECT_ROOT}"

# Build the test
print_status "info" "Building test executable..."
cargo build --bin modbus_rtu_test --release

# Run the test
print_status "info" "Executing tests..."
if ./target/release/modbus_rtu_test; then
    print_status "success" "All Modbus RTU tests passed!"
    TEST_RESULT=0
else
    print_status "error" "Some Modbus RTU tests failed"
    TEST_RESULT=1
fi

# Generate test report
if [[ -f "test_results/modbus_rtu_report.md" ]]; then
    print_status "info" "Test report generated at: test_results/modbus_rtu_report.md"
    
    # Display summary
    echo ""
    echo "Test Summary:"
    echo "============="
    grep -E "(Total Tests:|Passed:|Failed:|Success Rate:)" test_results/modbus_rtu_report.md || true
fi

exit $TEST_RESULT