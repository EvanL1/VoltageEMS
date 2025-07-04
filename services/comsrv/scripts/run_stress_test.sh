#!/bin/bash

# Run 1000+ points stress test

echo "=== Modbus 1000+ Points Stress Test ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if Redis is running
echo -e "${YELLOW}Checking Redis...${NC}"
if ! pgrep -x "redis-server" > /dev/null; then
    echo -e "${RED}Redis is not running. Starting Redis...${NC}"
    redis-server --daemonize yes
    sleep 2
fi
echo -e "${GREEN}✓ Redis is running${NC}"

# Kill any existing Modbus simulators
echo -e "${YELLOW}Cleaning up existing simulators...${NC}"
pkill -f "modbus_csv_simulator.py" 2>/dev/null || true
sleep 1

# Generate CSV files for different test sizes
echo -e "${YELLOW}Generating test configurations...${NC}"

# Small test (100 points)
python3 scripts/generate_large_csv.py config/test_points/StressTest100 100 50
echo -e "${GREEN}✓ Generated 150 points configuration${NC}"

# Medium test (500 points)
python3 scripts/generate_large_csv.py config/test_points/StressTest500 500 200
echo -e "${GREEN}✓ Generated 700 points configuration${NC}"

# Large test (1000 points)
python3 scripts/generate_large_csv.py config/test_points/StressTest1000 1000 500
echo -e "${GREEN}✓ Generated 1500 points configuration${NC}"

# Extra large test (2000 points)
python3 scripts/generate_large_csv.py config/test_points/StressTest2000 2000 1000
echo -e "${GREEN}✓ Generated 3000 points configuration${NC}"

echo ""
echo -e "${YELLOW}Starting stress test...${NC}"
echo "This will test performance with increasing number of points"
echo ""

# Run the stress test
cargo run --release --example stress_test_1000_points

# Cleanup
echo ""
echo -e "${YELLOW}Cleaning up...${NC}"
pkill -f "modbus_csv_simulator.py" 2>/dev/null || true

echo -e "${GREEN}✓ Stress test completed!${NC}"