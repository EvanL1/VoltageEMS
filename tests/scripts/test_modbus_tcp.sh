#!/bin/bash
# Modbus TCP Protocol Test Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "================================"
echo "Modbus TCP Protocol Test"
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
    pip3 install pymodbus
fi

# Start Modbus TCP simulator
print_status "info" "Starting Modbus TCP simulator..."
python3 "${PROJECT_ROOT}/tests/simulators/modbus_tcp_simulator.py" \
    --host 127.0.0.1 \
    --port 5502 \
    --slave-id 1 \
    --update-interval 0.1 &
SIMULATOR_PID=$!

# Wait for simulator to start
sleep 3

# Check if simulator is running
if ! kill -0 $SIMULATOR_PID 2>/dev/null; then
    print_status "error" "Failed to start Modbus TCP simulator"
    exit 1
fi

print_status "success" "Modbus TCP simulator started (PID: $SIMULATOR_PID)"

# Function to cleanup on exit
cleanup() {
    print_status "info" "Cleaning up..."
    
    if [[ -n $SIMULATOR_PID ]] && kill -0 $SIMULATOR_PID 2>/dev/null; then
        kill $SIMULATOR_PID
        print_status "info" "Stopped Modbus TCP simulator"
    fi
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Run the Rust tests
print_status "info" "Running Modbus TCP tests..."

cd "${PROJECT_ROOT}"

# Build the test
print_status "info" "Building test executable..."
cargo build --bin modbus_tcp_test --release

# Run the test
print_status "info" "Executing tests..."
if ./target/release/modbus_tcp_test; then
    print_status "success" "All Modbus TCP tests passed!"
    TEST_RESULT=0
else
    print_status "error" "Some Modbus TCP tests failed"
    TEST_RESULT=1
fi

# Generate test report
if [[ -f "test_results/modbus_tcp_report.md" ]]; then
    print_status "info" "Test report generated at: test_results/modbus_tcp_report.md"
    
    # Display summary
    echo ""
    echo "Test Summary:"
    echo "============="
    grep -E "(Total Tests:|Passed:|Failed:|Success Rate:)" test_results/modbus_tcp_report.md || true
fi

exit $TEST_RESULT