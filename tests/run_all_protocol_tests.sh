#!/bin/bash
# Run all protocol tests for CI/CD integration

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "======================================"
echo "VoltageEMS Protocol Test Suite"
echo "======================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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
        "test")
            echo -e "${BLUE}[TEST]${NC} $message"
            ;;
    esac
}

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Create results directory
mkdir -p "${PROJECT_ROOT}/test_results"

# Function to run a protocol test
run_protocol_test() {
    local protocol=$1
    local script_path="${SCRIPT_DIR}/scripts/test_${protocol}.sh"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    print_status "test" "Starting ${protocol} protocol test..."
    
    if [[ ! -f "$script_path" ]]; then
        print_status "info" "Test script not found for ${protocol}, skipping..."
        SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
        return
    fi
    
    if bash "$script_path"; then
        print_status "success" "${protocol} test passed!"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        print_status "error" "${protocol} test failed!"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
    
    echo ""
}

# Function to check dependencies
check_dependencies() {
    print_status "info" "Checking dependencies..."
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        print_status "error" "Python3 is required but not installed"
        exit 1
    fi
    
    # Check Rust/Cargo
    if ! command -v cargo &> /dev/null; then
        print_status "error" "Cargo is required but not installed"
        exit 1
    fi
    
    # Check Redis
    if ! command -v redis-cli &> /dev/null; then
        print_status "error" "Redis is required but not installed"
        exit 1
    fi
    
    print_status "success" "All dependencies satisfied"
}

# Function to setup test environment
setup_test_env() {
    print_status "info" "Setting up test environment..."
    
    # Start Redis if not running
    if ! redis-cli ping &> /dev/null; then
        print_status "info" "Starting Redis..."
        redis-server --daemonize yes
        sleep 2
    fi
    
    # Install Python dependencies
    print_status "info" "Installing Python dependencies..."
    pip3 install -q pymodbus pytest
    
    print_status "success" "Test environment ready"
}

# Main test execution
main() {
    local start_time=$(date +%s)
    
    check_dependencies
    setup_test_env
    
    echo ""
    print_status "info" "Running protocol tests..."
    echo ""
    
    # Run tests based on command line arguments or all by default
    if [[ $# -eq 0 ]]; then
        # Run all tests
        run_protocol_test "modbus_tcp"
        run_protocol_test "modbus_rtu"
        run_protocol_test "can"
        run_protocol_test "iec104"
        run_protocol_test "api"
    else
        # Run specific tests
        for protocol in "$@"; do
            run_protocol_test "$protocol"
        done
    fi
    
    # Calculate execution time
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # Generate summary report
    echo ""
    echo "======================================"
    echo "Test Summary"
    echo "======================================"
    echo "Total Tests:    $TOTAL_TESTS"
    echo "Passed:         $PASSED_TESTS"
    echo "Failed:         $FAILED_TESTS"
    echo "Skipped:        $SKIPPED_TESTS"
    echo "Duration:       ${duration}s"
    echo ""
    
    # Generate detailed report
    cat > "${PROJECT_ROOT}/test_results/summary.txt" <<EOF
VoltageEMS Protocol Test Summary
Generated: $(date)

Total Tests: $TOTAL_TESTS
Passed: $PASSED_TESTS
Failed: $FAILED_TESTS
Skipped: $SKIPPED_TESTS
Duration: ${duration}s

Test Results:
EOF
    
    # Add individual test results
    for report in "${PROJECT_ROOT}/test_results"/*_report.md; do
        if [[ -f "$report" ]]; then
            echo "" >> "${PROJECT_ROOT}/test_results/summary.txt"
            echo "--- $(basename "$report") ---" >> "${PROJECT_ROOT}/test_results/summary.txt"
            grep -E "(Total Tests:|Passed:|Failed:|Success Rate:)" "$report" >> "${PROJECT_ROOT}/test_results/summary.txt" || true
        fi
    done
    
    # Exit with appropriate code
    if [[ $FAILED_TESTS -gt 0 ]]; then
        print_status "error" "Some tests failed!"
        exit 1
    else
        print_status "success" "All tests passed!"
        exit 0
    fi
}

# Parse command line options
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            echo "Usage: $0 [protocol1] [protocol2] ..."
            echo "  Run specific protocol tests, or all if none specified"
            echo "  Available protocols: modbus_tcp, modbus_rtu, can, iec104"
            exit 0
            ;;
        *)
            break
            ;;
    esac
done

# Run main function with remaining arguments
main "$@"