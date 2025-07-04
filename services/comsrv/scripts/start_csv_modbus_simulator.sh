#!/bin/bash
# Start Modbus TCP Server Simulator based on CSV configuration

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

echo "Starting CSV-based Modbus TCP Server Simulator..."
echo "Configuration directory: $PROJECT_DIR/config/test_points/ModbusTCP_Demo"
echo ""

# Change to project directory
cd "$PROJECT_DIR"

# Check if CSV files exist
if [ ! -d "config/test_points/ModbusTCP_Demo" ]; then
    echo "ERROR: Configuration directory not found!"
    echo "Expected: config/test_points/ModbusTCP_Demo"
    exit 1
fi

# Check for required CSV files
REQUIRED_FILES=(
    "telemetry.csv"
    "signal.csv"
    "control.csv"
    "adjustment.csv"
    "mapping_telemetry.csv"
    "mapping_signal.csv"
    "mapping_control.csv"
    "mapping_adjustment.csv"
)

echo "Checking CSV configuration files..."
for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "config/test_points/ModbusTCP_Demo/$file" ]; then
        echo "  ✓ $file"
    else
        echo "  ✗ $file (missing)"
    fi
done
echo ""

# Run the simulator
echo "Starting server on 0.0.0.0:5020..."
echo "Press Ctrl+C to stop"
echo ""

python3 tests/modbus_csv_simulator.py \
    --host 0.0.0.0 \
    --port 5020 \
    --config-dir config/test_points/ModbusTCP_Demo \
    --debug