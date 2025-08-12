#!/bin/sh
set -e

# Start Redis server in background
redis-server --save 60 1 --loglevel warning &
REDIS_PID=$!

# Wait for Redis to be ready
echo "Waiting for Redis to start..."
until redis-cli ping > /dev/null 2>&1; do
    sleep 1
done
echo "Redis started successfully"

# Load all Lua functions
echo "Loading Redis functions..."
for lua_file in /data/*.lua; do
    if [ -f "$lua_file" ]; then
        echo "Loading $(basename $lua_file)..."
        redis-cli FUNCTION LOAD REPLACE < "$lua_file" || echo "Warning: Failed to load $(basename $lua_file)"
    fi
done
echo "Redis functions loaded"

# Keep Redis running in foreground
wait $REDIS_PID