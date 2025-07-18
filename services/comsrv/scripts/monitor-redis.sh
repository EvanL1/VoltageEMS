#!/bin/bash
# Monitor Redis data in real-time

echo "=== Real-time Redis Monitoring ==="
echo "Monitoring keys pattern: 1001:*"
echo "Press Ctrl+C to stop"
echo ""

# Function to display data
show_data() {
    echo -e "\n--- $(date '+%Y-%m-%d %H:%M:%S') ---"
    
    # Get all keys for channel 1001
    keys=$(docker exec comsrv-redis-test redis-cli --user comsrv --pass comsrv_secure_password_2025 keys "1001:*" 2>/dev/null | sort)
    
    if [ -z "$keys" ]; then
        echo "No data found for channel 1001"
    else
        echo "Found $(echo "$keys" | wc -w) keys"
        
        # Show telemetry data
        echo -e "\nTelemetry (m) data:"
        echo "$keys" | grep ":m:" | head -5 | while read key; do
            value=$(docker exec comsrv-redis-test redis-cli --user comsrv --pass comsrv_secure_password_2025 get "$key" 2>/dev/null)
            printf "  %-20s = %s\n" "$key" "$value"
        done
        
        # Show signal data
        echo -e "\nSignal (s) data:"
        echo "$keys" | grep ":s:" | head -5 | while read key; do
            value=$(docker exec comsrv-redis-test redis-cli --user comsrv --pass comsrv_secure_password_2025 get "$key" 2>/dev/null)
            printf "  %-20s = %s\n" "$key" "$value"
        done
    fi
}

# Monitor in loop
while true; do
    show_data
    sleep 2
done