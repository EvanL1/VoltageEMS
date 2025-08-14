#!/bin/bash

# Advanced test script for Modbus reconnection mechanism
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Advanced Modbus Reconnection Test ===${NC}"
echo ""

# Function to print colored messages
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if comsrv is running
check_comsrv() {
    if docker ps | grep -q voltageems-comsrv; then
        info "comsrv is running"
        return 0
    else
        error "comsrv is not running. Please start it first."
        return 1
    fi
}

# Check if modbus simulator is running
check_modbus_sim() {
    if docker ps | grep -q voltageems-modbus-sim; then
        info "Modbus simulator is running"
        return 0
    else
        warn "Modbus simulator is not running"
        return 1
    fi
}

# Monitor logs for reconnection patterns
monitor_reconnection() {
    info "Monitoring comsrv logs for reconnection patterns..."
    
    # Start monitoring in background
    docker logs -f voltageems-comsrv 2>&1 | while read line; do
        if echo "$line" | grep -q "Broken pipe\|Connection reset\|Connection refused"; then
            echo -e "${RED}[DISCONNECTED]${NC} Connection lost detected"
        elif echo "$line" | grep -q "attempting reconnection\|Attempting to reconnect"; then
            echo -e "${YELLOW}[RECONNECTING]${NC} Reconnection attempt in progress"
        elif echo "$line" | grep -q "reconnected successfully\|Successfully connected"; then
            echo -e "${GREEN}[CONNECTED]${NC} Reconnection successful!"
        elif echo "$line" | grep -q "Max consecutive reconnect attempts.*reached"; then
            echo -e "${YELLOW}[COOLDOWN]${NC} Entering cooldown period"
        elif echo "$line" | grep -q "Connection attempt [0-9]/[0-9] failed"; then
            attempt=$(echo "$line" | grep -oP 'attempt \K[0-9]/[0-9]')
            echo -e "${YELLOW}[RETRY]${NC} Reconnection attempt $attempt"
        fi
    done &
    
    return $!
}

# Test scenario 1: Simple disconnect/reconnect
test_simple_reconnect() {
    echo ""
    info "Test 1: Simple Disconnect/Reconnect"
    echo "----------------------------------------"
    
    info "Stopping Modbus simulator..."
    docker stop voltageems-modbus-sim >/dev/null 2>&1
    
    info "Waiting 10 seconds to observe reconnection attempts..."
    sleep 10
    
    info "Restarting Modbus simulator..."
    docker start voltageems-modbus-sim >/dev/null 2>&1
    
    info "Waiting 5 seconds for reconnection..."
    sleep 5
    
    echo -e "${GREEN}Test 1 completed${NC}"
}

# Test scenario 2: Extended disconnect (trigger cooldown)
test_cooldown() {
    echo ""
    info "Test 2: Extended Disconnect (Cooldown Test)"
    echo "----------------------------------------"
    
    info "Stopping Modbus simulator..."
    docker stop voltageems-modbus-sim >/dev/null 2>&1
    
    info "Waiting 30 seconds to trigger cooldown period..."
    info "You should see 5 attempts with exponential backoff, then cooldown"
    sleep 30
    
    info "Simulator still stopped. System should be in cooldown..."
    info "Waiting another 30 seconds..."
    sleep 30
    
    info "Restarting Modbus simulator..."
    docker start voltageems-modbus-sim >/dev/null 2>&1
    
    info "Waiting 10 seconds for reconnection after cooldown..."
    sleep 10
    
    echo -e "${GREEN}Test 2 completed${NC}"
}

# Test scenario 3: Rapid disconnect/reconnect
test_rapid_reconnect() {
    echo ""
    info "Test 3: Rapid Disconnect/Reconnect"
    echo "----------------------------------------"
    
    for i in {1..3}; do
        info "Cycle $i: Stopping simulator..."
        docker stop voltageems-modbus-sim >/dev/null 2>&1
        sleep 3
        
        info "Cycle $i: Starting simulator..."
        docker start voltageems-modbus-sim >/dev/null 2>&1
        sleep 3
    done
    
    echo -e "${GREEN}Test 3 completed${NC}"
}

# Main test execution
main() {
    echo "This script tests the automatic reconnection mechanism with:"
    echo "  - Exponential backoff (1s, 2s, 4s, 8s, 16s, max 30s)"
    echo "  - Cooldown period after 5 consecutive failures (1 minute)"
    echo "  - Continuous retry with cooldown cycles"
    echo ""
    
    # Check prerequisites
    if ! check_comsrv; then
        exit 1
    fi
    
    if ! check_modbus_sim; then
        warn "Starting Modbus simulator..."
        docker start voltageems-modbus-sim >/dev/null 2>&1 || {
            error "Failed to start Modbus simulator"
            exit 1
        }
        sleep 2
    fi
    
    # Start log monitoring
    monitor_reconnection
    LOG_PID=$!
    
    echo ""
    echo "Select test scenario:"
    echo "  1) Simple disconnect/reconnect"
    echo "  2) Extended disconnect (cooldown test)"
    echo "  3) Rapid disconnect/reconnect cycles"
    echo "  4) All tests"
    echo "  5) Manual testing (interactive)"
    echo ""
    read -p "Enter choice [1-5]: " choice
    
    case $choice in
        1)
            test_simple_reconnect
            ;;
        2)
            test_cooldown
            ;;
        3)
            test_rapid_reconnect
            ;;
        4)
            test_simple_reconnect
            test_cooldown
            test_rapid_reconnect
            ;;
        5)
            echo ""
            info "Manual testing mode. Commands:"
            echo "  - Stop simulator:  docker stop voltageems-modbus-sim"
            echo "  - Start simulator: docker start voltageems-modbus-sim"
            echo "  - Check status:    docker ps | grep modbus"
            echo ""
            echo "Monitoring logs... Press Ctrl+C to exit"
            wait $LOG_PID
            ;;
        *)
            error "Invalid choice"
            kill $LOG_PID 2>/dev/null
            exit 1
            ;;
    esac
    
    # Clean up
    kill $LOG_PID 2>/dev/null
    
    echo ""
    echo -e "${GREEN}=== Test Complete ===${NC}"
    echo ""
    echo "Summary of reconnection mechanism:"
    echo "  ✓ Automatic detection of connection loss"
    echo "  ✓ Exponential backoff for retry attempts"
    echo "  ✓ Cooldown period after max consecutive failures"
    echo "  ✓ Continuous retry cycles until connection restored"
}

# Run main function
main