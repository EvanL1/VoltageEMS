#!/bin/bash
# VoltageEMS 演示环境停止脚本

echo "=== 停止 VoltageEMS 演示环境 ==="

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# 停止模拟数据生成器
if [ -f .mock-data.pid ]; then
    PID=$(cat .mock-data.pid)
    if ps -p $PID > /dev/null 2>&1; then
        echo -e "${YELLOW}停止模拟数据生成器 (PID: $PID)...${NC}"
        kill $PID
        rm .mock-data.pid
        echo -e "${GREEN}✓ 模拟数据生成器已停止${NC}"
    else
        echo "模拟数据生成器未运行"
        rm .mock-data.pid
    fi
else
    # 尝试通过进程名停止
    pkill -f "node mock-data-generator.js" 2>/dev/null || true
fi

# 停止 Docker 服务
echo -e "${YELLOW}停止 Docker 服务...${NC}"
docker-compose -f docker-compose.grafana.yml down

# 停止 Docker Redis（如果存在）
docker stop voltage-redis 2>/dev/null && docker rm voltage-redis 2>/dev/null || true

echo ""
echo -e "${GREEN}✨ VoltageEMS 演示环境已停止${NC}"