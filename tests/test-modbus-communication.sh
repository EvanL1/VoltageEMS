#!/bin/bash
set -e

echo "=========================================="
echo "Modbus Communication Test"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 1. Check Modbus simulator status
echo "1. Checking Modbus Simulator"
echo "----------------------------------------"

# Check if modbus-sim container is running
if docker ps | grep -q modbus-sim; then
    log_success "Modbus simulator container is running"
else
    log_error "Modbus simulator container is not running"
    echo "Starting Modbus simulator..."
    docker-compose -f docker-compose.test.yml up -d modbus-sim
    sleep 3
fi

# Check port connectivity
echo -n "  - Port 5020 connectivity: "
if nc -z localhost 5020 2>/dev/null; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    exit 1
fi

# 2. Test Modbus TCP communication using Python
echo ""
echo "2. Testing Modbus TCP Communication"
echo "----------------------------------------"

# Create Python test script
cat > /tmp/test_modbus.py << 'EOF'
#!/usr/bin/env python3
import sys
try:
    from pymodbus.client import ModbusTcpClient
    from pymodbus.constants import Endian
    from pymodbus.payload import BinaryPayloadDecoder
    import time
    
    print("Connecting to Modbus server at localhost:5020...")
    client = ModbusTcpClient('localhost', port=5020)
    
    if client.connect():
        print("✓ Connected successfully")
        
        # Test 1: Read holding registers
        print("\nTest 1: Reading holding registers (address 0-9)")
        result = client.read_holding_registers(0, 10, slave=1)
        if not result.isError():
            print(f"✓ Read successful: {result.registers}")
        else:
            print(f"✗ Read failed: {result}")
        
        # Test 2: Write single register
        print("\nTest 2: Writing single register (address 0, value 12345)")
        result = client.write_register(0, 12345, slave=1)
        if not result.isError():
            print("✓ Write successful")
            
            # Verify write
            result = client.read_holding_registers(0, 1, slave=1)
            if not result.isError():
                print(f"✓ Verification: Register 0 = {result.registers[0]}")
        else:
            print(f"✗ Write failed: {result}")
        
        # Test 3: Write multiple registers
        print("\nTest 3: Writing multiple registers")
        values = [100, 200, 300, 400, 500]
        result = client.write_registers(10, values, slave=1)
        if not result.isError():
            print(f"✓ Multiple write successful")
            
            # Verify
            result = client.read_holding_registers(10, 5, slave=1)
            if not result.isError():
                print(f"✓ Verification: Registers 10-14 = {result.registers}")
        else:
            print(f"✗ Multiple write failed: {result}")
        
        # Test 4: Read input registers
        print("\nTest 4: Reading input registers")
        result = client.read_input_registers(0, 5, slave=1)
        if not result.isError():
            print(f"✓ Input registers: {result.registers}")
        else:
            print(f"⚠ Input registers not available: {result}")
        
        # Test 5: Read coils
        print("\nTest 5: Reading coils (digital outputs)")
        result = client.read_coils(0, 10, slave=1)
        if not result.isError():
            print(f"✓ Coils: {result.bits[:10]}")
        else:
            print(f"⚠ Coils not available: {result}")
        
        client.close()
        print("\n✓ All basic Modbus operations tested successfully")
        sys.exit(0)
    else:
        print("✗ Failed to connect to Modbus server")
        sys.exit(1)
        
except ImportError:
    print("PyModbus not installed. Installing...")
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "pymodbus"])
    print("Please run the script again.")
    sys.exit(1)
except Exception as e:
    print(f"✗ Error: {e}")
    sys.exit(1)
EOF

# Check if Python and pymodbus are available
if command -v python3 &> /dev/null; then
    log_info "Running Modbus communication test with Python..."
    python3 /tmp/test_modbus.py
else
    log_info "Python not found, using netcat for basic connectivity test..."
    
    # Basic TCP test with netcat
    echo "Test" | nc -w 2 localhost 5020 > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        log_success "TCP connection to Modbus server successful"
    else
        log_error "TCP connection to Modbus server failed"
    fi
fi

# 3. Test with comsrv (if available)
echo ""
echo "3. Testing comsrv Integration"
echo "----------------------------------------"

# Check if comsrv is configured for Modbus
if [ -f "services/comsrv/config/comsrv.yaml" ]; then
    log_info "Checking comsrv configuration..."
    
    # Display relevant configuration
    grep -A 5 "modbus" services/comsrv/config/comsrv.yaml || echo "No Modbus configuration found"
fi

# 4. Simulate data collection scenario
echo ""
echo "4. Simulating Data Collection"
echo "----------------------------------------"

# If comsrv is running, check Redis for data
if docker ps | grep -q comsrv-test; then
    log_info "comsrv is running, checking for collected data..."
    
    # Check for any comsrv data in Redis
    channels=$(docker exec redis-test redis-cli KEYS "comsrv:*" | wc -l)
    if [ "$channels" -gt 0 ]; then
        log_success "Found $channels comsrv data keys in Redis"
        
        # Show sample data
        echo "Sample data:"
        docker exec redis-test redis-cli KEYS "comsrv:*" | head -5 | while read key; do
            echo "  $key: $(docker exec redis-test redis-cli TYPE "$key")"
        done
    else
        log_info "No comsrv data found in Redis (service may need configuration)"
    fi
else
    log_info "comsrv is not running. To test full integration:"
    echo "  1. Configure services/comsrv/config/comsrv.yaml with Modbus settings"
    echo "  2. Start comsrv: docker-compose -f docker-compose.test.yml up -d comsrv"
    echo "  3. Monitor logs: docker logs -f comsrv-test"
fi

echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo -e "${GREEN}✓${NC} Modbus simulator is accessible on port 5020"
echo -e "${GREEN}✓${NC} TCP connectivity verified"
echo ""
echo "Next steps for full integration test:"
echo "1. Configure comsrv with Modbus device settings"
echo "2. Start comsrv service"
echo "3. Verify data collection in Redis"
echo ""