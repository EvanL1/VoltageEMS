#!/bin/bash
# CAN Protocol Test Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "================================"
echo "CAN Protocol Test"
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

# Check if python-can is installed
if ! python3 -c "import can" &> /dev/null; then
    print_status "info" "Installing python-can..."
    pip3 install python-can
fi

# Check if we're on Linux (required for vcan)
if [[ "$(uname)" != "Linux" ]]; then
    print_status "error" "CAN tests require Linux with vcan support"
    print_status "info" "On macOS, you can use:"
    print_status "info" "  - USB CAN adapters with appropriate drivers"
    print_status "info" "  - Docker container with Linux and vcan support"
    exit 1
fi

# Setup virtual CAN interface
print_status "info" "Setting up virtual CAN interface..."

# Load vcan kernel module
if ! lsmod | grep -q vcan; then
    print_status "info" "Loading vcan kernel module..."
    sudo modprobe vcan
fi

# Create vcan0 interface if it doesn't exist
if ! ip link show vcan0 &> /dev/null; then
    print_status "info" "Creating vcan0 interface..."
    sudo ip link add dev vcan0 type vcan
fi

# Bring up vcan0 interface
if ! ip link show vcan0 | grep -q "UP"; then
    print_status "info" "Bringing up vcan0 interface..."
    sudo ip link set up vcan0
fi

print_status "success" "Virtual CAN interface ready"

# Start CAN simulator
print_status "info" "Starting CAN simulator..."
python3 "${PROJECT_ROOT}/tests/simulators/can_simulator.py" \
    --interface vcan0 \
    --bitrate 500000 \
    --update-interval 0.05 &
SIMULATOR_PID=$!

# Wait for simulator to start
sleep 3

# Check if simulator is running
if ! kill -0 $SIMULATOR_PID 2>/dev/null; then
    print_status "error" "Failed to start CAN simulator"
    exit 1
fi

print_status "success" "CAN simulator started (PID: $SIMULATOR_PID)"

# Function to cleanup on exit
cleanup() {
    print_status "info" "Cleaning up..."
    
    if [[ -n $SIMULATOR_PID ]] && kill -0 $SIMULATOR_PID 2>/dev/null; then
        kill $SIMULATOR_PID
        print_status "info" "Stopped CAN simulator"
    fi
    
    # Optionally bring down vcan0 (commented out to allow reuse)
    # sudo ip link set down vcan0
    # sudo ip link delete vcan0
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Run the Rust tests
print_status "info" "Running CAN tests..."

cd "${PROJECT_ROOT}"

# Build the test
print_status "info" "Building test executable..."
cargo build --bin can_test --release --features can

# Run the test
print_status "info" "Executing tests..."
if ./target/release/can_test; then
    print_status "success" "All CAN tests passed!"
    TEST_RESULT=0
else
    print_status "error" "Some CAN tests failed"
    TEST_RESULT=1
fi

# Generate test report
if [[ -f "test_results/can_report.md" ]]; then
    print_status "info" "Test report generated at: test_results/can_report.md"
    
    # Display summary
    echo ""
    echo "Test Summary:"
    echo "============="
    grep -E "(Total Tests:|Passed:|Failed:|Success Rate:)" test_results/can_report.md || true
fi

# Optional: Monitor CAN traffic
print_status "info" "CAN interface statistics:"
ip -details -statistics link show vcan0

exit $TEST_RESULT