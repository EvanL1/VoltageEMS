#!/bin/bash

# Test script for optimized points demo

echo "=== Testing Optimized Points System ==="
echo ""

# Check if Redis is running
if ! redis-cli ping > /dev/null 2>&1; then
    echo "❌ Redis is not running. Please start Redis first:"
    echo "   brew services start redis"
    echo "   or"
    echo "   redis-server"
    exit 1
fi

echo "✅ Redis is running"
echo ""

# Clear existing test data
echo "🧹 Clearing existing test data..."
redis-cli --scan --pattern "comsrv:demo:points:*" | xargs -L 100 redis-cli DEL 2>/dev/null
redis-cli --scan --pattern "comsrv:demo:points:type:*" | xargs -L 100 redis-cli DEL 2>/dev/null

echo ""
echo "🚀 Running optimized points demo..."
echo "   - Generating 10,000 test points"
echo "   - Using HashMap<u32> for O(1) lookups"
echo "   - Batch syncing to Redis"
echo ""

# Run the demo
cargo run --example optimized_points_demo

echo ""
echo "✅ Demo completed"