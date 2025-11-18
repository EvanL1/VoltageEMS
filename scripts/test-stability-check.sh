#!/bin/bash
# Test Stability Check Script
#
# Runs comsrv integration tests multiple times to verify stability
# after fixing port conflict issues.
#
# Usage:
#   ./scripts/test-stability-check.sh [iterations]
#
# Default: 10 iterations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ITERATIONS=${1:-10}
TEST_PACKAGE="comsrv"
TEST_NAME="service_integration_test"

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}  Test Stability Check${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""
echo "Package: ${TEST_PACKAGE}"
echo "Test: ${TEST_NAME}"
echo "Iterations: ${ITERATIONS}"
echo ""

# Statistics
PASSED=0
FAILED=0
START_TIME=$(date +%s)

# Create temporary log directory
LOG_DIR="/tmp/voltage_test_stability_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$LOG_DIR"
echo -e "Logs will be saved to: ${YELLOW}${LOG_DIR}${NC}"
echo ""

# Run tests
for i in $(seq 1 $ITERATIONS); do
    echo -e "${BLUE}=== Run $i/${ITERATIONS} ===${NC}"

    # Run test and capture output
    if cargo test --package "$TEST_PACKAGE" --test "$TEST_NAME" \
        > "$LOG_DIR/run_${i}.log" 2>&1; then
        echo -e "${GREEN}✓ PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC}"
        ((FAILED++))

        # Show last 20 lines of failed test
        echo -e "${YELLOW}Last 20 lines of output:${NC}"
        tail -20 "$LOG_DIR/run_${i}.log"
        echo ""

        # Ask if we should continue
        if [ $i -lt $ITERATIONS ]; then
            read -p "Continue testing? (y/N) " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                echo -e "${YELLOW}Stopping test run${NC}"
                break
            fi
        fi
    fi

    # Small delay between runs to ensure cleanup
    if [ $i -lt $ITERATIONS ]; then
        sleep 1
    fi

    echo ""
done

# Calculate statistics
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
SUCCESS_RATE=$(awk "BEGIN {printf \"%.1f\", ($PASSED / ($PASSED + $FAILED)) * 100}")

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}  Test Results${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""
echo -e "Total runs:     $(($PASSED + $FAILED))"
echo -e "Passed:         ${GREEN}${PASSED}${NC}"
echo -e "Failed:         ${RED}${FAILED}${NC}"
echo -e "Success rate:   ${SUCCESS_RATE}%"
echo -e "Duration:       ${DURATION}s"
echo -e "Logs:           ${LOG_DIR}"
echo ""

# Final verdict
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed! Integration tests are stable.${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed. Review logs for details.${NC}"
    echo ""
    echo "Failed test logs:"
    for log in "$LOG_DIR"/run_*.log; do
        if grep -q "FAILED" "$log" 2>/dev/null; then
            echo "  - $log"
        fi
    done
    exit 1
fi
