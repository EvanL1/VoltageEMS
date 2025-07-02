#!/bin/bash

# Script to check points in Redis

echo "=== Checking Points in Redis ==="
echo ""

# Count total points
TOTAL_POINTS=$(redis-cli --scan --pattern "comsrv:demo:points:*" | grep -v ":type:" | wc -l | tr -d ' ')
echo "📊 Total points in Redis: $TOTAL_POINTS"
echo ""

# Count points by type
echo "📈 Points by type:"
for type in Telemetry Signaling Control Setpoint; do
    COUNT=$(redis-cli SCARD "comsrv:demo:points:type:$type" 2>/dev/null || echo 0)
    echo "   $type: $COUNT"
done
echo ""

# Show sample data
echo "📝 Sample point data:"
redis-cli --scan --pattern "comsrv:demo:points:*" | grep -v ":type:" | head -5 | while read key; do
    echo "   Key: $key"
    VALUE=$(redis-cli GET "$key" | jq -c '.' 2>/dev/null || redis-cli GET "$key")
    echo "   Value: $VALUE"
    echo ""
done

# Memory usage
echo "💾 Redis memory usage:"
redis-cli INFO memory | grep used_memory_human

echo ""
echo "✅ Check completed"