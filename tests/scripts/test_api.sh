#!/bin/bash
# API Gateway Test Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
API_TEST_DIR="${PROJECT_ROOT}/tests/api_tests"

echo "================================"
echo "API Gateway Tests"
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

# Create virtual environment if it doesn't exist
if [[ ! -d "${API_TEST_DIR}/venv" ]]; then
    print_status "info" "Creating Python virtual environment..."
    python3 -m venv "${API_TEST_DIR}/venv"
fi

# Activate virtual environment
source "${API_TEST_DIR}/venv/bin/activate"

# Install dependencies
print_status "info" "Installing test dependencies..."
pip install -q -r "${API_TEST_DIR}/requirements.txt"

# Check if services are running
print_status "info" "Checking if services are running..."

# Check Redis
if ! redis-cli ping &> /dev/null; then
    print_status "error" "Redis is not running. Please start Redis first."
    exit 1
fi

# Check API Gateway
if ! curl -s -f http://localhost:8080/api/v1/health > /dev/null; then
    print_status "error" "API Gateway is not running at http://localhost:8080"
    print_status "info" "Please start the API Gateway service first"
    exit 1
fi

print_status "success" "All required services are running"

# Run tests based on argument
TEST_TYPE="${1:-all}"

cd "${API_TEST_DIR}"

case $TEST_TYPE in
    "unit")
        print_status "info" "Running API unit tests..."
        pytest test_apigateway.py -v -m "not integration and not performance"
        ;;
    "integration")
        print_status "info" "Running API integration tests..."
        pytest test_apigateway.py -v -m "integration"
        ;;
    "e2e")
        print_status "info" "Running end-to-end tests..."
        pytest test_e2e_integration.py -v
        ;;
    "performance")
        print_status "info" "Running performance tests..."
        pytest test_apigateway.py test_e2e_integration.py -v -m "performance"
        ;;
    "all")
        print_status "info" "Running all API tests..."
        pytest -v
        ;;
    *)
        print_status "error" "Unknown test type: $TEST_TYPE"
        echo "Usage: $0 [unit|integration|e2e|performance|all]"
        exit 1
        ;;
esac

TEST_RESULT=$?

# Generate test report
if [[ -f ".coverage" ]]; then
    print_status "info" "Generating coverage report..."
    coverage report
    coverage html
    print_status "info" "Coverage report generated in htmlcov/"
fi

# Deactivate virtual environment
deactivate

if [[ $TEST_RESULT -eq 0 ]]; then
    print_status "success" "All API tests passed!"
else
    print_status "error" "Some API tests failed"
fi

exit $TEST_RESULT