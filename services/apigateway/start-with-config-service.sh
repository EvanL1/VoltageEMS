#!/bin/bash

# API Gateway startup script with config service integration

echo "Starting API Gateway with Config Service integration..."

# Set environment variables
export RUST_LOG=info
export CONFIG_SERVICE_URL=${CONFIG_SERVICE_URL:-"http://localhost:8000"}
export JWT_SECRET=${JWT_SECRET:-"your-secret-key-min-32-characters-long!!"}

# Check if config service is available
echo "Checking config service availability at $CONFIG_SERVICE_URL..."
if curl -f -s "$CONFIG_SERVICE_URL/health" > /dev/null; then
    echo "✓ Config service is available"
else
    echo "⚠ Config service is not available at $CONFIG_SERVICE_URL"
    echo "  API Gateway will use local configuration as fallback"
fi

# Start API Gateway
echo "Starting API Gateway..."
cargo run --release

# Alternative: Run the compiled binary
# ./target/release/apigateway