#!/bin/bash

# ComsRv Lua 同步监控脚本
# 实时监控 Lua 同步的执行情况

REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"
INTERVAL="${INTERVAL:-5}"  # 监控间隔（秒）

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Redis 连接命令
REDIS_CMD="$REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT"

# 清屏函数
clear_screen() {
    printf "\033c"
}

# 获取时间戳
get_timestamp() {
    date "+%Y-%m-%d %H:%M:%S"
}

# 监控函数
monitor_sync() {
    clear_screen
    
    echo -e "${GREEN}=== ComsRv Lua 同步监控 ===${NC}"
    echo -e "时间: $(get_timestamp)"
    echo -e "刷新间隔: ${INTERVAL}秒\n"
    
    # 1. 映射统计
    echo -e "${BLUE}映射配置:${NC}"
    FORWARD_COUNT=$($REDIS_CMD --scan --pattern "mapping:comsrv:*" 2>/dev/null | wc -l)
    REVERSE_COUNT=$($REDIS_CMD --scan --pattern "mapping:reverse:*" 2>/dev/null | wc -l)
    echo "  正向映射: $FORWARD_COUNT 个"
    echo "  反向映射: $REVERSE_COUNT 个"
    
    # 2. 同步数据统计
    echo -e "\n${BLUE}同步数据:${NC}"
    
    # ModSrv 数据点统计
    MODSRV_MODELS=$($REDIS_CMD --scan --pattern "modsrv:*:measurement" 2>/dev/null | wc -l)
    echo "  ModSrv 模型数: $MODSRV_MODELS"
    
    # 显示最近更新的数据
    echo -e "\n${CYAN}最近更新的数据点:${NC}"
    for key in $($REDIS_CMD --scan --pattern "modsrv:*:measurement" 2>/dev/null | head -5); do
        MODEL=$(echo $key | cut -d: -f2)
        FIELDS=$($REDIS_CMD HLEN $key 2>/dev/null)
        echo "  $MODEL: $FIELDS 个测点"
    done
    
    # 3. 命令队列统计
    echo -e "\n${BLUE}命令队列:${NC}"
    CMD_QUEUES=$($REDIS_CMD --scan --pattern "cmd:*" 2>/dev/null | wc -l)
    echo "  活跃命令队列: $CMD_QUEUES 个"
    
    # 显示待处理的命令
    for key in $($REDIS_CMD --scan --pattern "cmd:*" 2>/dev/null | head -3); do
        CMD_COUNT=$($REDIS_CMD HLEN $key 2>/dev/null)
        if [ "$CMD_COUNT" -gt 0 ]; then
            echo "  $key: $CMD_COUNT 条待处理命令"
        fi
    done
    
    # 4. 告警队列
    echo -e "\n${BLUE}告警队列:${NC}"
    ALARM_COUNT=$($REDIS_CMD LLEN "alarm:queue" 2>/dev/null)
    if [ -z "$ALARM_COUNT" ]; then
        ALARM_COUNT=0
    fi
    echo "  待处理告警: $ALARM_COUNT 条"
    
    # 5. 性能指标（如果有）
    echo -e "\n${BLUE}性能指标:${NC}"
    
    # Redis 内存使用
    MEMORY_USED=$($REDIS_CMD INFO memory 2>/dev/null | grep "used_memory_human" | cut -d: -f2 | tr -d '\r')
    echo "  Redis 内存使用: $MEMORY_USED"
    
    # 命令执行统计
    OPS=$($REDIS_CMD INFO stats 2>/dev/null | grep "instantaneous_ops_per_sec" | cut -d: -f2 | tr -d '\r')
    echo "  每秒操作数: $OPS"
    
    # 脚本执行统计
    EVALSHA_CALLS=$($REDIS_CMD INFO stats 2>/dev/null | grep "evalsha_calls" | cut -d: -f2 | tr -d '\r')
    if [ ! -z "$EVALSHA_CALLS" ]; then
        echo "  Lua 脚本调用: $EVALSHA_CALLS 次"
    fi
    
    echo -e "\n按 Ctrl+C 退出监控"
}

# 实时监控模式
if [ "$1" == "--live" ]; then
    echo -e "${YELLOW}进入实时监控模式...${NC}"
    sleep 1
    
    # 使用 Redis MONITOR 命令
    $REDIS_CMD MONITOR | grep -E "(EVALSHA|sync_measurement|sync_control|batch_sync)" | while read line; do
        TIMESTAMP=$(echo $line | cut -d' ' -f1)
        if echo $line | grep -q "EVALSHA"; then
            echo -e "${GREEN}[$TIMESTAMP] Lua 同步执行${NC}"
        elif echo $line | grep -q "sync_measurement"; then
            echo -e "${BLUE}[$TIMESTAMP] 测量数据同步${NC}"
        elif echo $line | grep -q "sync_control"; then
            echo -e "${YELLOW}[$TIMESTAMP] 控制命令同步${NC}"
        elif echo $line | grep -q "batch_sync"; then
            echo -e "${CYAN}[$TIMESTAMP] 批量同步${NC}"
        fi
    done
else
    # 周期监控模式
    trap 'echo -e "\n${GREEN}监控结束${NC}"; exit 0' INT
    
    while true; do
        monitor_sync
        sleep $INTERVAL
    done
fi