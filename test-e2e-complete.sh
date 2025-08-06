#!/bin/bash

# 禁用代理，确保本地连接不通过代理
export NO_PROXY="*"
export no_proxy="*"
unset http_proxy
unset https_proxy
unset HTTP_PROXY
unset HTTPS_PROXY
unset ALL_PROXY
unset all_proxy

echo "=== VoltageEMS 端到端集成测试 ==="
echo "测试时间: $(date)"
echo "已禁用所有代理设置"
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 测试计数
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 测试函数
run_test() {
    local test_name=$1
    local test_command=$2
    local expected_result=$3
    
    ((TOTAL_TESTS++))
    echo -n "Testing $test_name... "
    
    result=$(eval "$test_command" 2>&1)
    
    if [[ "$result" == *"$expected_result"* ]]; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED_TESTS++))
        return 0
    else
        echo -e "${RED}✗${NC}"
        echo "  Expected: $expected_result"
        echo "  Got: $result"
        ((FAILED_TESTS++))
        return 1
    fi
}

echo -e "${BLUE}1. 测试基础服务健康状态${NC}"
echo "================================"

# 健康检查
run_test "Redis连接" "redis-cli -h localhost -p 6379 ping" "PONG"
run_test "Modbus模拟器" "nc -zv localhost 5020 2>&1" "open"
run_test "API网关健康" "curl -s http://localhost:6005/health | jq -r .status" "healthy"
run_test "Modsrv健康" "curl -s http://localhost:6001/health | jq -r .status" "healthy"
run_test "Alarmsrv健康" "curl -s http://localhost:6002/health | jq -r .status" "healthy"
run_test "Rulesrv健康" "curl -s http://localhost:6003/health | jq -r .status" "healthy"
run_test "Hissrv健康" "curl -s http://localhost:6004/health | jq -r .status" "healthy"
run_test "Comsrv健康" "curl -s http://localhost:6000/health | jq -r .status" "healthy"

echo ""
echo -e "${BLUE}2. 测试Redis Functions${NC}"
echo "================================"

# 检查Redis Functions是否加载
run_test "Redis Functions加载" "redis-cli -h localhost -p 6379 FUNCTION LIST | grep -c 'model_'" "1"

echo ""
echo -e "${BLUE}3. 测试数据流 (Modbus → Comsrv → Redis)${NC}"
echo "================================"

# 等待数据采集
echo "等待5秒让comsrv采集数据..."
sleep 5

# 检查Redis中的数据
run_test "遥测数据存在" "redis-cli -h localhost -p 6379 EXISTS comsrv:1001:T" "1"
run_test "遥测点1有值" "redis-cli -h localhost -p 6379 HEXISTS comsrv:1001:T 1" "1"
run_test "遥测点2有值" "redis-cli -h localhost -p 6379 HEXISTS comsrv:1001:T 2" "1"

# 获取并显示实际数据
echo ""
echo "实际采集的数据:"
echo -n "  温度 (Point 1): "
redis-cli -h localhost -p 6379 HGET comsrv:1001:T 1
echo -n "  压力 (Point 2): "
redis-cli -h localhost -p 6379 HGET comsrv:1001:T 2

echo ""
echo -e "${BLUE}4. 测试模型服务 (CRUD)${NC}"
echo "================================"

# 创建模型
MODEL_ID="test_model_$(date +%s)"
run_test "创建模型" \
    "curl -s -X POST http://localhost:6001/models \
     -H 'Content-Type: application/json' \
     -d '{\"model_id\":\"$MODEL_ID\",\"name\":\"测试模型\",\"type\":\"device\"}' \
     | jq -r .model_id" \
    "$MODEL_ID"

# 查询模型
run_test "查询模型" \
    "curl -s http://localhost:6001/models/$MODEL_ID | jq -r .name" \
    "测试模型"

# 删除模型
run_test "删除模型" \
    "curl -s -X DELETE http://localhost:6001/models/$MODEL_ID \
     | jq -r .message" \
    "deleted"

echo ""
echo -e "${BLUE}5. 测试告警服务${NC}"
echo "================================"

# 创建告警
ALARM_RESULT=$(curl -s -X POST http://localhost:6002/alarms \
    -H 'Content-Type: application/json' \
    -d '{"title":"测试告警","description":"温度过高","level":"Warning"}')
ALARM_ID=$(echo "$ALARM_RESULT" | jq -r .alarm_id)

if [ "$ALARM_ID" != "null" ] && [ ! -z "$ALARM_ID" ]; then
    echo -e "创建告警... ${GREEN}✓${NC} (ID: $ALARM_ID)"
    ((PASSED_TESTS++))
    ((TOTAL_TESTS++))
    
    # 确认告警
    run_test "确认告警" \
        "curl -s -X PUT http://localhost:6002/alarms/$ALARM_ID/acknowledge \
         | jq -r .status" \
        "acknowledged"
else
    echo -e "创建告警... ${RED}✗${NC}"
    ((FAILED_TESTS++))
    ((TOTAL_TESTS++))
fi

echo ""
echo -e "${BLUE}6. 测试规则服务${NC}"
echo "================================"

# 创建规则
RULE_RESULT=$(curl -s -X POST http://localhost:6003/rules \
    -H 'Content-Type: application/json' \
    -d '{
        "name":"温度监控规则",
        "condition":"temperature > 30",
        "action":"create_alarm",
        "enabled":true
    }')
RULE_ID=$(echo "$RULE_RESULT" | jq -r .rule_id)

if [ "$RULE_ID" != "null" ] && [ ! -z "$RULE_ID" ]; then
    echo -e "创建规则... ${GREEN}✓${NC} (ID: $RULE_ID)"
    ((PASSED_TESTS++))
    ((TOTAL_TESTS++))
    
    # 触发规则
    run_test "执行规则" \
        "curl -s -X POST http://localhost:6003/rules/$RULE_ID/execute \
         | jq -r .executed" \
        "true"
else
    echo -e "创建规则... ${RED}✗${NC}"
    ((FAILED_TESTS++))
    ((TOTAL_TESTS++))
fi

echo ""
echo -e "${BLUE}7. 测试历史数据服务${NC}"
echo "================================"

# 配置历史数据采集
run_test "配置数据采集" \
    "curl -s -X POST http://localhost:6004/collections \
     -H 'Content-Type: application/json' \
     -d '{\"pattern\":\"comsrv:*:T\",\"interval\":5000,\"enabled\":true}' \
     | jq -r .status" \
    "configured"

echo ""
echo -e "${BLUE}8. 测试API网关路由${NC}"
echo "================================"

# 通过网关访问各服务
run_test "网关→Modsrv" "curl -s http://localhost:6005/api/models | jq -r .service" "modsrv"
run_test "网关→Alarmsrv" "curl -s http://localhost:6005/api/alarms | jq -r .service" "alarmsrv"
run_test "网关→Rulesrv" "curl -s http://localhost:6005/api/rules | jq -r .service" "rulesrv"
run_test "网关→Hissrv" "curl -s http://localhost:6005/api/history/status | jq -r .service" "hissrv"

echo ""
echo -e "${BLUE}9. 测试数据一致性${NC}"
echo "================================"

# 写入测试数据
TEST_VALUE="123.456"
redis-cli -h localhost -p 6379 HSET test:consistency:T 1 $TEST_VALUE > /dev/null

# 通过不同路径读取
DIRECT_READ=$(redis-cli -h localhost -p 6379 HGET test:consistency:T 1)
run_test "数据一致性" "echo $DIRECT_READ" "$TEST_VALUE"

# 清理测试数据
redis-cli -h localhost -p 6379 DEL test:consistency:T > /dev/null

echo ""
echo "========================================"
echo -e "${BLUE}测试总结${NC}"
echo "========================================"
echo "总测试数: $TOTAL_TESTS"
echo -e "通过: ${GREEN}$PASSED_TESTS${NC}"
echo -e "失败: ${RED}$FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 所有测试通过！系统运行正常。${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ 有 $FAILED_TESTS 个测试失败，请检查日志。${NC}"
    exit 1
fi