#!/bin/bash

set -e

echo "Testing Lightweight Services"
echo "============================"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Kill any existing processes
echo "Cleaning up existing processes..."
pkill -f rulesrv-lightweight 2>/dev/null || true
pkill -f modsrv-lightweight 2>/dev/null || true
sleep 1

# Start rulesrv
echo -e "\nStarting rulesrv..."
RUST_LOG=info ./target/debug/rulesrv-lightweight services/rulesrv/config/rules.yaml > /tmp/rulesrv.log 2>&1 &
RULESRV_PID=$!

# Start modsrv
echo "Starting modsrv..."
RUST_LOG=info ./target/debug/modsrv-lightweight services/modsrv/config/models.yaml > /tmp/modsrv.log 2>&1 &
MODSRV_PID=$!

# Wait for services to start
echo "Waiting for services to start..."
sleep 3

# Function to test endpoint
test_endpoint() {
    local url=$1
    local desc=$2
    
    echo -n "Testing $desc: "
    
    # Use wget as alternative to curl
    if command -v wget >/dev/null 2>&1; then
        if wget -q -O - --timeout=2 "$url" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ OK${NC}"
            wget -q -O - "$url" 2>/dev/null | jq '.' 2>/dev/null || echo "Response received"
        else
            echo -e "${RED}✗ FAILED${NC}"
        fi
    else
        # Use nc as last resort
        echo -e "\nUsing nc to test $url"
        echo -e "GET ${url#*:*:*} HTTP/1.0\r\nHost: localhost\r\n\r\n" | nc 127.0.0.1 ${url:17:4} | head -20
    fi
}

# Test endpoints
echo -e "\n--- Testing rulesrv (port 6003) ---"
test_endpoint "http://127.0.0.1:6003/health" "Health check"
test_endpoint "http://127.0.0.1:6003/api/v1/rules" "List rules"
test_endpoint "http://127.0.0.1:6003/api/v1/stats" "Statistics"

echo -e "\n--- Testing modsrv (port 6001) ---"
test_endpoint "http://127.0.0.1:6001/health" "Health check"
test_endpoint "http://127.0.0.1:6001/api/v1/templates" "List templates"
test_endpoint "http://127.0.0.1:6001/api/v1/models" "List models"

# Test with Python if available
if command -v python3 >/dev/null 2>&1; then
    echo -e "\n--- Testing with Python ---"
    python3 -c "
import urllib.request
import json

try:
    with urllib.request.urlopen('http://127.0.0.1:6003/health') as resp:
        data = json.loads(resp.read())
        print('rulesrv health:', data['status'])
except Exception as e:
    print('rulesrv error:', e)

try:
    with urllib.request.urlopen('http://127.0.0.1:6001/health') as resp:
        data = json.loads(resp.read())
        print('modsrv health:', data['status'])
except Exception as e:
    print('modsrv error:', e)
"
fi

# Check processes
echo -e "\n--- Process Status ---"
if ps -p $RULESRV_PID > /dev/null; then
    echo -e "rulesrv: ${GREEN}Running (PID: $RULESRV_PID)${NC}"
else
    echo -e "rulesrv: ${RED}Not running${NC}"
fi

if ps -p $MODSRV_PID > /dev/null; then
    echo -e "modsrv: ${GREEN}Running (PID: $MODSRV_PID)${NC}"
else
    echo -e "modsrv: ${RED}Not running${NC}"
fi

# Show logs
echo -e "\n--- Recent Logs ---"
echo "rulesrv:"
tail -5 /tmp/rulesrv.log | grep -v "Executed"
echo -e "\nmodsrv:"
tail -5 /tmp/modsrv.log

# Cleanup
echo -e "\n--- Cleanup ---"
echo "Stopping services..."
kill $RULESRV_PID $MODSRV_PID 2>/dev/null || true

echo -e "\n${GREEN}Test completed!${NC}"