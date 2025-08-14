#!/bin/bash

# Test script for verifying channel-specific logging
set -e

echo "=== Testing Channel-Specific Logging ==="
echo

# Function to check log files
check_logs() {
    echo "Checking log files..."
    
    # Check main comsrv log
    if [ -f "logs/comsrv.log.$(date +%Y-%m-%d)" ]; then
        echo "✓ Main comsrv log exists: $(wc -l < "logs/comsrv.log.$(date +%Y-%m-%d)") lines"
    else
        echo "✗ Main comsrv log not found"
    fi
    
    # Check channel logs directory
    if [ -d "logs/channels" ]; then
        echo "✓ Channels log directory exists"
        echo "Channel log files:"
        ls -la logs/channels/ | grep -E "channel_.*\.log"
        
        # Check if any channel logs have content
        for logfile in logs/channels/channel_*.log.*; do
            if [ -f "$logfile" ]; then
                lines=$(wc -l < "$logfile" 2>/dev/null || echo "0")
                echo "  $(basename "$logfile"): $lines lines"
                
                if [ "$lines" -gt 0 ]; then
                    echo "  Sample content:"
                    head -3 "$logfile" | sed 's/^/    /'
                fi
            fi
        done
    else
        echo "✗ Channels log directory not found"
    fi
}

# Function to monitor logs in real-time
monitor_logs() {
    echo "Monitoring channel logs in real-time..."
    echo "Look for protocol messages (TX/RX) and parsed data..."
    echo "Press Ctrl+C to stop monitoring"
    
    # Monitor all channel logs
    if [ -d "logs/channels" ]; then
        tail -f logs/channels/channel_*.log.* 2>/dev/null || {
            echo "No channel log files found to monitor"
            return 1
        }
    else
        echo "No channels directory found"
        return 1
    fi
}

# Main script
echo "1. Current log status:"
check_logs

echo
echo "2. Available actions:"
echo "  1) Monitor logs in real-time"
echo "  2) Check logs again"
echo "  3) Clear all logs and restart"
echo "  4) Exit"
echo

read -p "Select action [1-4]: " choice

case $choice in
    1)
        monitor_logs
        ;;
    2)
        check_logs
        ;;
    3)
        echo "Clearing logs..."
        rm -f logs/comsrv.log.*
        rm -rf logs/channels/
        echo "Logs cleared. You may restart comsrv now."
        ;;
    4)
        echo "Exiting..."
        exit 0
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo
echo "Test completed!"