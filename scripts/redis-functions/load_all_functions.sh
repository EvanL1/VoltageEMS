#!/bin/sh

# 加载所有 VoltageEMS Redis Functions

echo "=== 加载 VoltageEMS Redis Functions ==="

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Redis 连接
REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-redis}"
REDIS_PORT="${REDIS_PORT:-6379}"

# 函数文件目录
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# 检查 Redis 连接
echo -n "检查 Redis 连接 ($REDIS_HOST:$REDIS_PORT)... "
if $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT ping > /dev/null 2>&1; then
    printf "${GREEN}成功${NC}\n"
else
    printf "${RED}失败${NC}\n"
    echo "请确保 Redis 已启动"
    exit 1
fi

# 加载整合后的函数文件
for func in core specific domain services; do
    if [ -f "$SCRIPT_DIR/${func}.lua" ]; then
        echo -n "加载 ${func}.lua... "
        if cat "$SCRIPT_DIR/${func}.lua" | $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT -x FUNCTION LOAD REPLACE > /dev/null 2>&1; then
            printf "${GREEN}成功${NC}\n"
        else
            printf "${RED}失败${NC}\n"
            echo "错误信息:"
            cat "$SCRIPT_DIR/${func}.lua" | $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT -x FUNCTION LOAD REPLACE
        fi
    fi
done

# 列出已加载的函数
echo ""
echo "已加载的函数库:"
$REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST | grep -E "library_name|functions" | head -30

echo ""
echo -e "${GREEN}Redis Functions 加载完成！${NC}"
echo ""
echo "可用的通用函数:"
echo "  - generic_store         : 通用实体存储"
echo "  - generic_batch_sync    : 通用批量同步"
echo "  - generic_query         : 通用查询"
echo "  - generic_state_machine : 通用状态机"
echo "  - generic_multi_index   : 多维索引管理"
echo "  - generic_condition_eval: 条件评估器"
echo "  - generic_batch_collect : 批量数据收集"
echo "  - generic_event_publish : 事件发布器"
echo "  - generic_statistics    : 统计引擎"
echo ""
echo "可用的特定函数:"
echo "  - dag_executor            : DAG执行器 (rulesrv)"
echo "  - line_protocol_converter : Line Protocol转换器 (hissrv)"
echo ""
echo -e "${YELLOW}提示: 使用相应的测试脚本测试各服务函数${NC}"