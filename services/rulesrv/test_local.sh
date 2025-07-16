#!/bin/bash
set -e

echo "=== Local Rulesrv Test ==="
echo "1. Building rulesrv..."

# Build the service
cargo build -p rulesrv

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful!"

# Check if Redis is running
echo "2. Checking Redis..."
redis-cli ping > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "❌ Redis is not running. Please start Redis first."
    echo "   Run: docker run -d --name redis-test -p 6379:6379 redis:7-alpine"
    exit 1
fi

echo "✅ Redis is running"

# Create test rules
echo "3. Creating test rules in Redis..."

# Temperature threshold rule
redis-cli SET "rulesrv:rule:temp_high" '{
  "id": "temp_high",
  "name": "High Temperature Alert",
  "description": "Triggers when temperature exceeds threshold",
  "group_id": null,
  "condition": "temperature > 80",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:high",
      "message": "Temperature exceeded 80°C"
    }
  ],
  "enabled": true,
  "priority": 10
}' > /dev/null

# Add rule to set
redis-cli SADD "rulesrv:rules" "temp_high" > /dev/null

echo "✅ Test rules created"

# Run the service
echo "4. Starting rulesrv..."
echo "   Service will run at http://localhost:8083"
echo "   Press Ctrl+C to stop"
echo ""

RUST_LOG=info,rulesrv=debug cargo run -p rulesrv -- service