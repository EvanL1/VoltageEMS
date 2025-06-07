#!/bin/bash

# COMSRV pressure test monitoring script
# Monitor comsrv logs under heavy load

echo "🔍 COMSRV pressure test log monitor"
echo "=" 
echo "This script monitors comsrv logs in real time"
echo "Press Ctrl+C to stop"
echo ""

# Create monitoring session
LOG_DIR="./logs"
CHANNELS_DIR="$LOG_DIR/channels"
MAIN_LOG="$LOG_DIR/comsrv_pressure.log"

# Ensure log directory exists
if [ ! -d "$LOG_DIR" ]; then
    echo "⚠️  Log directory not found, creating..."
    mkdir -p "$LOG_DIR"
fi

echo "📁 Log directory: $LOG_DIR"
echo "📊 Main log file: $MAIN_LOG"
echo "📂 Channel log directory: $CHANNELS_DIR"
echo ""

# Start monitoring processes
monitor_main_log() {
    echo "🔍 [Main log] Monitoring main log file..."
    if [ -f "$MAIN_LOG" ]; then
        tail -f "$MAIN_LOG" | while read line; do
            echo "[main] $line"
        done
    else
        echo "⚠️  Main log file not created yet: $MAIN_LOG"
        while [ ! -f "$MAIN_LOG" ]; do
            sleep 1
        done
        echo "✅ Main log file created, start monitoring..."
        tail -f "$MAIN_LOG" | while read line; do
            echo "[main] $line"
        done
    fi
}

monitor_channel_logs() {
    echo "🔍 [Channel log] Monitoring channel logs..."
    
    # Wait for channel directory creation
    while [ ! -d "$CHANNELS_DIR" ]; do
        sleep 1
    done
    
    # Monitor today's logs for all channels
    TODAY=$(date +"%Y-%m-%d")
    
    # Use inotify to watch for new or modified files
    if command -v fswatch >/dev/null 2>&1; then
        # Use fswatch on macOS
        fswatch -o "$CHANNELS_DIR" | while read f; do
            echo "📝 Channel logs updated..."
            find "$CHANNELS_DIR" -name "*$TODAY.log" -newer /tmp/last_check 2>/dev/null | while read logfile; do
                channel_name=$(basename $(dirname "$logfile"))
                tail -n 1 "$logfile" | sed "s/^/[channel:$channel_name] /"
            done
            touch /tmp/last_check
        done
    else
        # Fallback to polling
        while true; do
            find "$CHANNELS_DIR" -name "*$TODAY.log" -type f 2>/dev/null | while read logfile; do
                if [ -f "$logfile" ]; then
                    channel_name=$(basename $(dirname "$logfile"))
                    tail -n 5 "$logfile" | tail -n 1 | sed "s/^/[channel:$channel_name] /"
                fi
            done
            sleep 2
        done
    fi
}

show_pressure_stats() {
    echo "📊 [Stats] Starting performance monitoring..."
    
    while true; do
        sleep 10
        
        echo ""
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "📊 Pressure test stats ($(date))"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        
        # Process information
        if pgrep -f "comsrv.*pressure_test_config" > /dev/null; then
            comsrv_pid=$(pgrep -f "comsrv.*pressure_test_config")
            echo "🟢 COMSRV running (PID: $comsrv_pid)"
            
            # Memory usage
            if command -v ps >/dev/null 2>&1; then
                memory_usage=$(ps -p $comsrv_pid -o rss= 2>/dev/null | awk '{print $1/1024}')
                if [ ! -z "$memory_usage" ]; then
                    echo "💾 Memory usage: ${memory_usage} MB"
                fi
            fi
        else
            echo "🔴 COMSRV not running"
        fi
        
        # Channel log count
        if [ -d "$CHANNELS_DIR" ]; then
            channel_count=$(find "$CHANNELS_DIR" -maxdepth 1 -type d | wc -l)
            channel_count=$((channel_count - 1))  # Subtract parent directory
            echo "📂 Active channels: $channel_count"
            
            # Count today's log entries
            TODAY=$(date +"%Y-%m-%d")
            total_lines=0
            find "$CHANNELS_DIR" -name "*$TODAY.log" -type f 2>/dev/null | while read logfile; do
                lines=$(wc -l < "$logfile" 2>/dev/null || echo "0")
                total_lines=$((total_lines + lines))
            done
            echo "📝 Log entries today: counting..."
        fi
        
        # Main log size
        if [ -f "$MAIN_LOG" ]; then
            log_size=$(du -h "$MAIN_LOG" 2>/dev/null | cut -f1)
            echo "📋 Main log size: $log_size"
        fi
        
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo ""
    done
}

# Catch exit signals
cleanup() {
    echo ""
    echo "🛑 Stopping monitoring..."
    kill $(jobs -p) 2>/dev/null
    exit 0
}

trap cleanup SIGINT SIGTERM

# Start all monitoring processes
echo "🚀 Starting monitoring processes..."

# Launch background monitors
show_pressure_stats &
STATS_PID=$!

# Foreground monitoring of channel logs (main output)
monitor_channel_logs &
CHANNEL_PID=$!

# Wait for user interruption
wait $CHANNEL_PID $STATS_PID 