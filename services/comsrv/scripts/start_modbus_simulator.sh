#!/bin/bash

# Start Modbus TCP Server Simulator

echo "=== Modbus TCP Server Simulator for comsrv ==="
echo ""
echo "This simulator provides:"
echo "  - 遥测 (YC): Analog measurements with real-time updates"
echo "  - 遥信 (YX): Digital status signals"
echo "  - 遥控 (YK): Writable digital outputs"
echo "  - 遥调 (YT): Writable analog setpoints (float32)"
echo ""
echo "Configuration matches comsrv mapping files:"
echo "  - Slave ID 1 & 2"
echo "  - Addresses from mapping_*.csv files"
echo ""

# Check if Python 3 is installed
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is required but not installed."
    echo "   Please install Python 3 first."
    exit 1
fi

# Default parameters
PORT=5020
HOST=0.0.0.0

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --port)
            PORT="$2"
            shift 2
            ;;
        --host)
            HOST="$2"
            shift 2
            ;;
        --debug)
            DEBUG="--debug"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--port PORT] [--host HOST] [--debug]"
            exit 1
            ;;
    esac
done

echo "🚀 Starting Modbus server on $HOST:$PORT"
echo "   Press Ctrl+C to stop"
echo ""

# Run the simulator
cd "$(dirname "$0")/.." || exit
python3 tests/modbus_server_simulator.py --host "$HOST" --port "$PORT" $DEBUG