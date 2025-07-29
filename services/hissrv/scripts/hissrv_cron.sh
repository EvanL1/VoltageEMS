#!/bin/bash
# hissrv 定时任务脚本
# 用于 cron 调度，定期触发数据聚合

# 配置
REDIS_CLI=${REDIS_CLI:-redis-cli}
REDIS_HOST=${REDIS_HOST:-localhost}
REDIS_PORT=${REDIS_PORT:-6379}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 加载脚本 SHA
if [ -f "$SCRIPT_DIR/script_sha.env" ]; then
    source "$SCRIPT_DIR/script_sha.env"
else
    echo "Error: script_sha.env not found. Please run init_scripts.sh first."
    exit 1
fi

# 获取当前时间戳
TIMESTAMP=$(date +%s)

# 执行聚合任务
case "$1" in
    "1m")
        # 1分钟聚合
        echo "[$(date)] Running 1-minute aggregation..."
        $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT EVALSHA $AGGREGATE_SCRIPT_SHA 0 aggregate_1m $TIMESTAMP
        ;;
    
    "5m")
        # 5分钟聚合
        echo "[$(date)] Running 5-minute aggregation..."
        $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT EVALSHA $AGGREGATE_SCRIPT_SHA 0 aggregate_5m $TIMESTAMP
        ;;
    
    "test")
        # 测试：推送 JSON 数据
        echo "[$(date)] Pushing test JSON data..."
        $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT EVALSHA $AGGREGATE_SCRIPT_SHA 0 push_json $TIMESTAMP
        ;;
    
    *)
        echo "Usage: $0 {1m|5m|test}"
        exit 1
        ;;
esac