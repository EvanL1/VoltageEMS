#!/bin/sh
set -e

# Start Redis in background
redis-server --daemonize yes

# Wait for Redis to be ready
echo "Waiting for Redis to start..."
until redis-cli ping > /dev/null 2>&1; do
    sleep 0.1
done
echo "Redis is ready!"

# Load all Lua functions
echo "Loading Lua functions..."
/usr/local/bin/init-functions.sh

# Stop background Redis
redis-cli shutdown

# Start Redis in foreground
echo "Starting Redis server..."
if [ "$#" -eq 0 ]; then
    # No arguments provided, start with default config
    exec redis-server
else
    # Pass through any arguments
    exec "$@"
fi