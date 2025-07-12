#!/bin/bash
# start.sh - Start comsrv service

set -e

echo "Starting comsrv..."

# Set environment variables
export RUST_LOG=${RUST_LOG:-info}
export COMSRV_CONFIG=${COMSRV_CONFIG:-/app/config/comsrv-docker.yaml}

# Create necessary directories
mkdir -p /app/logs /app/data /app/logs/channels

# Wait for dependencies
echo "Waiting for Redis..."
for i in {1..30}; do
    if nc -z redis 6379 2>/dev/null; then
        echo "Redis is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "Redis timeout!"
        exit 1
    fi
    sleep 1
done

# Wait for Modbus simulator
echo "Waiting for Modbus simulator..."
sleep 3

# Start the service
echo "Starting comsrv with config: $COMSRV_CONFIG"
exec /app/bin/comsrv --config "$COMSRV_CONFIG" 2>&1