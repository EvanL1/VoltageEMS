#!/bin/bash
set -e

# Start Redis server (in background)
redis-server --save 60 1 --loglevel warning &
REDIS_PID=$!

# Wait for Redis to start
echo "Waiting for Redis to start..."
sleep 2
until redis-cli ping; do
    echo "Waiting for Redis to start..."
    sleep 1
done

# Load Redis Functions
echo "Loading Redis functions..."
cd /scripts && bash load_functions.sh

echo "Redis functions loaded successfully!"

# Keep Redis running
wait $REDIS_PID