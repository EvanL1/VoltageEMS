#!/bin/bash
set -e

# 启动 Redis 服务器（后台运行）
redis-server --save 60 1 --loglevel warning &
REDIS_PID=$!

# 等待 Redis 启动
echo "Waiting for Redis to start..."
sleep 2
until redis-cli ping; do
    echo "Waiting for Redis to start..."
    sleep 1
done

# 加载 Redis Functions
echo "Loading Redis functions..."
cd /scripts && bash load_all_functions.sh

echo "Redis functions loaded successfully!"

# 保持 Redis 运行
wait $REDIS_PID