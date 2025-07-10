#!/bin/bash
#
# Docker多协议测试脚本
# 测试多个通道、多种协议的并发运行

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Docker多协议测试 ==="
echo "工作目录: $PROJECT_DIR"
echo

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 测试函数
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    ((TOTAL_TESTS++))
    echo -n "测试 $test_name ... "
    
    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED_TESTS++))
        return 0
    else
        echo -e "${RED}✗${NC}"
        ((FAILED_TESTS++))
        return 1
    fi
}

# 1. 清理旧环境
echo "1. 清理旧环境"
cd "$PROJECT_DIR"
docker-compose -f docker-compose.multi.yml down -v 2>/dev/null || true
echo

# 2. 构建镜像
echo "2. 构建Docker镜像"
./scripts/build-docker.sh
echo

# 3. 启动服务
echo "3. 启动服务"
docker-compose -f docker-compose.multi.yml up -d
echo

# 4. 等待服务启动
echo "4. 等待服务启动 (30秒)..."
sleep 30
echo

# 5. 容器状态检查
echo "5. 容器状态检查"
run_test "Redis容器运行中" "docker ps | grep -q voltage-redis-multi"
run_test "Modbus模拟器1运行中" "docker ps | grep -q voltage-modbus-sim-1"
run_test "Modbus模拟器2运行中" "docker ps | grep -q voltage-modbus-sim-2"
run_test "IEC104模拟器运行中" "docker ps | grep -q voltage-iec104-sim"
run_test "ComSrv服务运行中" "docker ps | grep -q voltage-comsrv-multi"
echo

# 6. API端点检查
echo "6. API端点检查"
API_BASE="http://localhost:8080"

run_test "健康检查端点" "curl -s $API_BASE/api/v1/channels | grep -q 'channels'"
run_test "通道列表端点" "curl -s $API_BASE/api/v1/channels | jq -e '.channels | length > 0'"
run_test "Swagger文档" "curl -s $API_BASE/swagger-ui/ | grep -q 'swagger'"
echo

# 7. 通道连接状态
echo "7. 通道连接状态"
CHANNELS=$(curl -s $API_BASE/api/v1/channels | jq -r '.channels[] | @base64')

for channel in $CHANNELS; do
    CHANNEL_DATA=$(echo "$channel" | base64 -d)
    CHANNEL_ID=$(echo "$CHANNEL_DATA" | jq -r '.id')
    CHANNEL_NAME=$(echo "$CHANNEL_DATA" | jq -r '.name')
    CONNECTED=$(echo "$CHANNEL_DATA" | jq -r '.connected')
    
    if [ "$CONNECTED" = "true" ]; then
        echo -e "  通道 $CHANNEL_ID ($CHANNEL_NAME): ${GREEN}已连接${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "  通道 $CHANNEL_ID ($CHANNEL_NAME): ${RED}未连接${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
done
echo

# 8. CSV点位加载验证
echo "8. CSV点位加载验证"
for channel in $CHANNELS; do
    CHANNEL_DATA=$(echo "$channel" | base64 -d)
    CHANNEL_ID=$(echo "$CHANNEL_DATA" | jq -r '.id')
    POINT_COUNT=$(echo "$CHANNEL_DATA" | jq -r '.point_count')
    
    run_test "通道 $CHANNEL_ID 点位数量 > 0" "[ $POINT_COUNT -gt 0 ]"
done
echo

# 9. 协议多样性检查
echo "9. 协议多样性检查"
PROTOCOLS=$(curl -s $API_BASE/api/v1/channels | jq -r '.channels[].protocol' | sort -u)
PROTOCOL_COUNT=$(echo "$PROTOCOLS" | wc -l | tr -d ' ')

run_test "支持多种协议" "[ $PROTOCOL_COUNT -gt 1 ]"
echo "  检测到的协议: $(echo $PROTOCOLS | tr '\n' ', ')"
echo

# 10. 数据读取测试
echo "10. 数据读取测试"
FIRST_CHANNEL=$(curl -s $API_BASE/api/v1/channels | jq -r '.channels[0].id')
if [ -n "$FIRST_CHANNEL" ]; then
    run_test "读取通道 $FIRST_CHANNEL 数据" "curl -s $API_BASE/api/v1/channels/$FIRST_CHANNEL/points | jq -e '.points | length > 0'"
fi
echo

# 11. 日志检查
echo "11. 日志检查"
run_test "主日志文件存在" "docker exec voltage-comsrv-multi ls /app/logs/comsrv.log"
run_test "日志内容不为空" "docker exec voltage-comsrv-multi test -s /app/logs/comsrv.log"
echo

# 12. Redis数据验证
echo "12. Redis数据验证"
REDIS_KEYS=$(docker exec voltage-redis-multi redis-cli keys "voltage:*" | wc -l)
run_test "Redis中有数据" "[ $REDIS_KEYS -gt 0 ]"
echo "  Redis键数量: $REDIS_KEYS"
echo

# 13. 性能指标
echo "13. 性能指标"
echo -n "  容器内存使用: "
docker stats --no-stream --format "table {{.Container}}\t{{.MemUsage}}" | grep voltage- || true
echo

# 14. 测试报告
echo "=== 测试报告 ==="
echo "总测试数: $TOTAL_TESTS"
echo -e "通过: ${GREEN}$PASSED_TESTS${NC}"
echo -e "失败: ${RED}$FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}所有测试通过！${NC}"
    EXIT_CODE=0
else
    echo -e "\n${RED}有 $FAILED_TESTS 个测试失败${NC}"
    echo
    echo "调试信息："
    echo "1. 查看容器日志: docker logs voltage-comsrv-multi"
    echo "2. 查看容器状态: docker-compose -f docker-compose.multi.yml ps"
    echo "3. 进入容器调试: docker exec -it voltage-comsrv-multi /bin/sh"
    EXIT_CODE=1
fi

# 15. 清理选项
echo
read -p "是否清理测试环境？[y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "清理测试环境..."
    docker-compose -f docker-compose.multi.yml down -v
fi

exit $EXIT_CODE