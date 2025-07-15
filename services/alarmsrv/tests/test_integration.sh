#!/bin/bash
set -e

# Alarm service integration test script

echo "=== Alarm Service Integration Tests ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test Redis connection
echo "Checking Redis connection..."
if redis-cli -n 15 ping > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis is running${NC}"
else
    echo -e "${RED}✗ Redis is not running${NC}"
    echo "Please start Redis before running tests"
    exit 1
fi

# Clean test database
echo "Cleaning test database..."
redis-cli -n 15 flushdb > /dev/null
echo -e "${GREEN}✓ Test database cleaned${NC}"

# Run unit tests
echo
echo "Running unit tests..."
cargo test --lib -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Unit tests passed${NC}"
else
    echo -e "${RED}✗ Unit tests failed${NC}"
    exit 1
fi

# Run API integration tests
echo
echo "Running API integration tests..."
cargo test api_tests -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ API tests passed${NC}"
else
    echo -e "${RED}✗ API tests failed${NC}"
    exit 1
fi

# Run Redis integration tests
echo
echo "Running Redis integration tests..."
cargo test redis_tests -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Redis tests passed${NC}"
else
    echo -e "${RED}✗ Redis tests failed${NC}"
    exit 1
fi

# Run service integration tests
echo
echo "Running service integration tests..."
cargo test service_tests -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Service tests passed${NC}"
else
    echo -e "${RED}✗ Service tests failed${NC}"
    exit 1
fi

# Run all tests together
echo
echo "Running all tests together..."
cargo test -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed${NC}"
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi

# Test with release build
echo
echo "Testing release build..."
cargo test --release -- --nocapture
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Release tests passed${NC}"
else
    echo -e "${RED}✗ Release tests failed${NC}"
    exit 1
fi

# Clean test database after tests
echo
echo "Cleaning up test data..."
redis-cli -n 15 flushdb > /dev/null
echo -e "${GREEN}✓ Test cleanup complete${NC}"

echo
echo -e "${GREEN}=== All integration tests passed! ===${NC}"