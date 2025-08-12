#!/bin/bash
# 加载修复后的 Redis Lua 函数

set -e

echo "Loading Redis Lua functions..."

# 只加载修复后的版本
echo "Loading services_fixed.lua..."
cat /Users/lyf/dev/VoltageEMS/scripts/redis-functions/services_fixed.lua | docker exec -i voltageems-redis redis-cli -x FUNCTION LOAD REPLACE

if [ $? -eq 0 ]; then
    echo "✅ Successfully loaded Redis functions"
    
    # 验证函数
    echo "Verifying loaded functions..."
    docker exec voltageems-redis redis-cli FUNCTION LIST | grep -E "modsrv_|hissrv_" | head -5
else
    echo "❌ Failed to load Redis functions"
    exit 1
fi