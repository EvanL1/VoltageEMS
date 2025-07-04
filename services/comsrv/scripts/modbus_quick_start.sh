#!/bin/bash

# Modbus Quick Start Script
# This script helps users quickly test Modbus functionality

echo "=== Modbus Quick Start ==="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to show usage
show_usage() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  demo      - Run a simple demonstration"
    echo "  test      - Run all tests"
    echo "  stress    - Run stress test with 1000+ points"
    echo "  example   - Run complete example"
    echo "  help      - Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 demo    # Run simple demo"
    echo "  $0 test    # Run all tests"
}

# Function to check dependencies
check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}"
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}Error: Python 3 is not installed${NC}"
        exit 1
    fi
    
    # Check Redis
    if ! command -v redis-cli &> /dev/null; then
        echo -e "${RED}Error: Redis is not installed${NC}"
        exit 1
    fi
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: Rust/Cargo is not installed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ All dependencies satisfied${NC}"
}

# Function to start Redis if needed
ensure_redis() {
    if ! pgrep -x "redis-server" > /dev/null; then
        echo -e "${YELLOW}Starting Redis...${NC}"
        redis-server --daemonize yes
        sleep 2
    fi
    echo -e "${GREEN}✓ Redis is running${NC}"
}

# Function to run demo
run_demo() {
    echo -e "\n${BLUE}=== Running Modbus Demo ===${NC}\n"
    
    # Start simulator
    echo -e "${YELLOW}Starting Modbus simulator...${NC}"
    python3 tests/modbus_server_simulator.py &
    SIMULATOR_PID=$!
    sleep 2
    
    # Run simple test
    echo -e "\n${YELLOW}Running simple Modbus test...${NC}"
    cargo run --example simple_modbus_test
    
    # Show Redis data
    echo -e "\n${YELLOW}Checking data in Redis...${NC}"
    echo "Sample telemetry values:"
    for i in 1001 1002 1003; do
        value=$(redis-cli get point:$i 2>/dev/null)
        if [ ! -z "$value" ]; then
            echo "  Point $i: $value"
        fi
    done
    
    # Cleanup
    kill $SIMULATOR_PID 2>/dev/null
    
    echo -e "\n${GREEN}✓ Demo completed!${NC}"
}

# Function to run all tests
run_tests() {
    echo -e "\n${BLUE}=== Running All Modbus Tests ===${NC}\n"
    
    # Unit tests
    echo -e "${YELLOW}Running unit tests...${NC}"
    cargo test modbus
    
    # Integration tests
    echo -e "\n${YELLOW}Running integration tests...${NC}"
    ./scripts/run_e2e_csv_test.sh
    
    # RTU tests
    echo -e "\n${YELLOW}Running RTU tests...${NC}"
    cargo run --example modbus_rtu_test
    
    # Batch optimization tests
    echo -e "\n${YELLOW}Running batch optimization tests...${NC}"
    cargo run --example batch_optimization_test
    
    echo -e "\n${GREEN}✓ All tests completed!${NC}"
}

# Function to run stress test
run_stress() {
    echo -e "\n${BLUE}=== Running Stress Test ===${NC}\n"
    ./scripts/run_stress_test.sh
}

# Function to run complete example
run_example() {
    echo -e "\n${BLUE}=== Running Complete Example ===${NC}\n"
    
    # Start simulator
    echo -e "${YELLOW}Starting Modbus simulator...${NC}"
    python3 tests/modbus_server_simulator.py &
    SIMULATOR_PID=$!
    sleep 2
    
    # Run example
    cargo run --example modbus_complete_example
    
    # Cleanup
    kill $SIMULATOR_PID 2>/dev/null
    
    echo -e "\n${GREEN}✓ Example completed!${NC}"
}

# Main script
check_dependencies
ensure_redis

case "${1:-help}" in
    demo)
        run_demo
        ;;
    test)
        run_tests
        ;;
    stress)
        run_stress
        ;;
    example)
        run_example
        ;;
    help|*)
        show_usage
        ;;
esac

echo ""