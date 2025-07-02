#!/bin/bash

# HisSrv Startup Script

set -e

# Configuration
CONFIG_FILE="hissrv.yaml"
LOG_LEVEL="info"
RUST_LOG="hissrv=info"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting HisSrv - Historical Data Service${NC}"

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${YELLOW}⚠️  Configuration file $CONFIG_FILE not found. Creating default...${NC}"
    cat > "$CONFIG_FILE" << 'EOF'
# HisSrv Configuration File
service:
  name: "hissrv"
  version: "0.2.0"
  port: 8080
  host: "0.0.0.0"

redis:
  connection:
    host: "127.0.0.1"
    port: 6379
    password: ""
    socket: ""
    database: 0
    pool_size: 10
    timeout: 5
  subscription:
    channels:
      - "data:*"
      - "events:*"
    key_patterns:
      - "*"

storage:
  default: "influxdb"
  backends:
    influxdb:
      enabled: true
      url: "http://localhost:8086"
      database: "hissrv_data"
      username: ""
      password: ""
      retention_days: 30
      batch_size: 1000
      flush_interval: 10
    postgresql:
      enabled: false
      host: "localhost"
      port: 5432
      database: "hissrv"
      username: "postgres"
      password: ""
      pool_size: 10
    mongodb:
      enabled: false
      uri: "mongodb://localhost:27017"
      database: "hissrv"
      collection: "data"

data:
  filters:
    default_policy: "store"
    rules:
      - pattern: "temp:*"
        action: "store"
        storage: "influxdb"
      - pattern: "log:*"
        action: "ignore"
      - pattern: "alarm:*"
        action: "store"
        storage: "influxdb"
  transformations: []

api:
  enabled: true
  prefix: "/api/v1"
  swagger_ui: true
  cors:
    enabled: true
    origins: ["*"]
    methods: ["GET", "POST", "PUT", "DELETE"]

monitoring:
  enabled: true
  metrics_port: 9090
  health_check: true

logging:
  level: "info"
  format: "text"
  file: "logs/hissrv.log"
  max_size: "100MB"
  max_files: 10

performance:
  worker_threads: 4
  max_concurrent_requests: 1000
  queue_size: 10000
  batch_processing: true
EOF
    echo -e "${GREEN}✅ Default configuration created at $CONFIG_FILE${NC}"
fi

# Create logs directory
mkdir -p logs

# Check dependencies
echo -e "${YELLOW}🔍 Checking dependencies...${NC}"

# Check if Redis is available
if ! command -v redis-cli &> /dev/null; then
    echo -e "${YELLOW}⚠️  redis-cli not found. Make sure Redis is installed and running.${NC}"
else
    if redis-cli ping &> /dev/null; then
        echo -e "${GREEN}✅ Redis is running${NC}"
    else
        echo -e "${RED}❌ Redis is not responding. Please start Redis server.${NC}"
        echo -e "${YELLOW}   Try: redis-server${NC}"
    fi
fi

# Check if InfluxDB is available (optional)
if command -v influx &> /dev/null; then
    echo -e "${GREEN}✅ InfluxDB CLI found${NC}"
else
    echo -e "${YELLOW}⚠️  InfluxDB CLI not found. InfluxDB storage may not work.${NC}"
fi

# Build the project
echo -e "${YELLOW}🔨 Building HisSrv...${NC}"
if cargo build --release; then
    echo -e "${GREEN}✅ Build successful${NC}"
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

# Set environment variables
export RUST_LOG="$RUST_LOG"

# Start the service
echo -e "${GREEN}🚀 Starting HisSrv with configuration: $CONFIG_FILE${NC}"
echo -e "${GREEN}📊 API Documentation will be available at: http://localhost:8080/api/v1/swagger-ui${NC}"
echo -e "${GREEN}🔍 Health check endpoint: http://localhost:8080/api/v1/health${NC}"
echo -e "${YELLOW}   Press Ctrl+C to stop${NC}"
echo ""

# Run the service
exec ./target/release/hissrv-rust --config "$CONFIG_FILE" --log-level "$LOG_LEVEL"