#!/bin/bash
# 初始化 Lua 脚本到 Redis

REDIS_CLI=${REDIS_CLI:-redis-cli}
REDIS_HOST=${REDIS_HOST:-localhost}
REDIS_PORT=${REDIS_PORT:-6379}

echo "Loading Lua scripts to Redis..."

# 加载聚合脚本
AGGREGATE_SHA=$($REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT SCRIPT LOAD "$(cat archive_aggregator.lua)")
echo "Archive aggregator script loaded with SHA: $AGGREGATE_SHA"

# 保存 SHA 到文件，方便其他脚本使用
echo "export AGGREGATE_SCRIPT_SHA=$AGGREGATE_SHA" > script_sha.env

echo "Done!"
echo ""
echo "To use the scripts:"
echo "1. Source the environment: source script_sha.env"
echo "2. Run aggregation: redis-cli EVALSHA \$AGGREGATE_SCRIPT_SHA 0 aggregate_1m"
echo ""
echo "Or use cron to schedule:"
echo "* * * * * redis-cli EVALSHA $AGGREGATE_SHA 0 aggregate_1m"
echo "*/5 * * * * redis-cli EVALSHA $AGGREGATE_SHA 0 aggregate_5m"