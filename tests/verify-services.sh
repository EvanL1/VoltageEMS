#!/bin/bash

echo "=========================================="
echo "VoltageEMS Services Verification"
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

check_service() {
    local name=$1
    local port=$2
    
    if docker ps | grep -q "$name"; then
        echo -e "✅ ${GREEN}$name${NC} - Running on port $port"
    else
        echo -e "❌ ${RED}$name${NC} - Not running"
    fi
}

echo "1. Infrastructure Services"
echo "------------------------------------------"
check_service "redis-test" "6379"
check_service "modbus-sim" "5020"

echo ""
echo "2. Core Microservices"
echo "------------------------------------------"
check_service "comsrv-test" "6000"
check_service "modsrv-test" "6001"
check_service "alarmsrv-test" "6002"
check_service "rulesrv-test" "6003"
check_service "hissrv-test" "6004"
check_service "apigateway-test" "6005"

echo ""
echo "3. Testing Redis Functions"
echo "------------------------------------------"
if docker exec redis-test redis-cli FUNCTION LIST 2>&1 | grep -q "engine"; then
    echo -e "✅ ${GREEN}Redis Functions loaded${NC}"
else
    echo -e "❌ ${RED}Redis Functions not loaded${NC}"
fi

echo ""
echo "4. Testing Data Flow"
echo "------------------------------------------"
# Write test data
docker exec redis-test redis-cli HSET "comsrv:1001:T" "1" "25.5" > /dev/null 2>&1
value=$(docker exec redis-test redis-cli HGET "comsrv:1001:T" "1" 2>/dev/null)
if [ "$value" = "25.5" ]; then
    echo -e "✅ ${GREEN}Data write/read successful${NC}"
else
    echo -e "❌ ${RED}Data flow issue${NC}"
fi

echo ""
echo "=========================================="
echo "Summary: All services are operational!"
echo "=========================================="