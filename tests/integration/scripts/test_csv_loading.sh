#!/bin/bash
# Test CSV loading functionality

set -e

echo "=== Testing CSV Loading Functionality ==="

# Configuration
CONFIG_DIR="/Users/lyf/dev/VoltageEMS/tests/integration/config"
CONTAINER_NAME="comsrv_csv_test"

# Create test configuration
cat > $CONFIG_DIR/test_csv.yaml <<EOF
version: "1.0"
service:
  name: "comsrv_csv_test"
  api:
    enabled: true
    bind_address: "0.0.0.0:3000"
  redis:
    url: "redis://localhost:6379"
    prefix: "test:"
  logging:
    level: "debug"
    file: "/app/logs/service/comsrv.log"
    console: true

channels:
  - id: 1
    name: "Modbus TCP Test Channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
      timeout: 5000
    table_config:
      four_telemetry_route: "Modbus_TCP_Test_1"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        adjustment_file: "adjustment.csv"
        control_file: "control.csv"
      protocol_mapping_route: "Modbus_TCP_Test_1"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        adjustment_mapping: "mapping_adjustment.csv"
        control_mapping: "mapping_control.csv"
    logging:
      enabled: true
      level: "debug"
      log_dir: "/app/logs/channels/channel_1"
EOF

# Stop any existing container
docker stop $CONTAINER_NAME 2>/dev/null || true
docker rm $CONTAINER_NAME 2>/dev/null || true

# Run comsrv with CSV configuration
echo "Starting comsrv with CSV configuration..."
docker run -d \
    --name $CONTAINER_NAME \
    -v $CONFIG_DIR:/app/config \
    -e RUST_LOG=comsrv=debug,comsrv::core::config=trace \
    -e COMSRV_CSV_BASE_PATH=/app/config \
    -p 3001:3000 \
    comsrv:integration

# Wait for startup
echo "Waiting for comsrv to start..."
sleep 5

# Check logs for CSV loading
echo "Checking logs for CSV loading..."
docker logs $CONTAINER_NAME 2>&1 | grep -E "(CSV|points|loaded)" | tail -20

# Check if API is responding
echo "Checking API health..."
curl -s http://localhost:3001/api/v1/health | jq .

# Check channels
echo "Checking channels..."
curl -s http://localhost:3001/api/v1/comsrv/channels | jq .

# Check if points were loaded
echo "Checking points for channel 1..."
curl -s http://localhost:3001/api/v1/comsrv/points/1 | jq length 2>/dev/null || echo "Points endpoint not available"

# Stop container
echo "Stopping test container..."
docker stop $CONTAINER_NAME
docker rm $CONTAINER_NAME

echo "=== CSV Loading Test Complete ==="