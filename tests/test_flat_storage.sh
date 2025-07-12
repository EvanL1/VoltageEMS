#!/bin/bash
# test_flat_storage.sh - 测试扁平化存储架构

echo "=== VoltageEMS 扁平化存储测试 ==="
echo "开始时间: $(date)"
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# 测试计数器
PASSED=0
FAILED=0

# 测试函数
test_case() {
    local name="$1"
    local command="$2"
    local expected="$3"
    
    echo -n "测试: $name ... "
    result=$(eval "$command" 2>&1)
    
    if [[ "$result" == *"$expected"* ]]; then
        echo -e "${GREEN}✓ 通过${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗ 失败${NC}"
        echo "  期望: $expected"
        echo "  实际: $result"
        ((FAILED++))
    fi
}

# 1. 清理测试数据
echo "清理测试数据..."
redis-cli --scan --pattern "1001:*" | xargs -r redis-cli del >/dev/null 2>&1
redis-cli --scan --pattern "cfg:1001:*" | xargs -r redis-cli del >/dev/null 2>&1
redis-cli --scan --pattern "2001:*" | xargs -r redis-cli del >/dev/null 2>&1
echo ""

# 2. 测试单点写入和读取
echo "=== 单点操作测试 ==="

# 写入测温数据
test_case "写入温度数据" \
    "redis-cli set '1001:m:10001' '25.6:1704956400000' && echo OK" \
    "OK"

# 读取温度数据
test_case "读取温度数据" \
    "redis-cli get '1001:m:10001'" \
    "25.6:1704956400000"

# 写入开关状态
test_case "写入开关状态" \
    "redis-cli set '1001:s:20001' '1:1704956401000' && echo OK" \
    "OK"

# 读取开关状态
test_case "读取开关状态" \
    "redis-cli get '1001:s:20001'" \
    "1:1704956401000"

echo ""

# 3. 测试批量操作
echo "=== 批量操作测试 ==="

# 批量写入
test_case "批量写入数据" \
    "redis-cli mset '1001:m:10002' '380.5:1704956402000' '1001:m:10003' '220.3:1704956403000' '1001:m:10004' '15.8:1704956404000' && echo OK" \
    "OK"

# 批量读取
test_case "批量读取数据" \
    "redis-cli mget '1001:m:10002' '1001:m:10003' '1001:m:10004' | grep -c '380.5'" \
    "1"

echo ""

# 4. 测试配置数据
echo "=== 配置数据测试 ==="

# 写入配置
CONFIG_JSON='{"name":"温度传感器1","unit":"°C","scale":0.1,"offset":0,"address":"1:3:100"}'
test_case "写入点位配置" \
    "redis-cli set 'cfg:1001:m:10001' '$CONFIG_JSON' && echo OK" \
    "OK"

# 读取配置
test_case "读取点位配置" \
    "redis-cli get 'cfg:1001:m:10001' | grep -o '温度传感器1'" \
    "温度传感器1"

echo ""

# 5. 测试模式匹配查询
echo "=== 模式匹配测试 ==="

# 查询通道下所有测量点
test_case "查询通道1001的测量点" \
    "redis-cli --scan --pattern '1001:m:*' | wc -l | xargs" \
    "4"

# 查询所有配置
test_case "查询所有配置数据" \
    "redis-cli --scan --pattern 'cfg:*' | wc -l | xargs" \
    "1"

echo ""

# 6. 测试跨服务数据流
echo "=== 跨服务数据流测试 ==="

# 模拟comsrv写入
timestamp=$(date +%s)000
redis-cli set "2001:m:30001" "100.5:$timestamp" >/dev/null
redis-cli set "2001:m:30002" "200.8:$timestamp" >/dev/null

# 模拟modsrv计算（求和）
val1=$(redis-cli get "2001:m:30001" | cut -d: -f1)
val2=$(redis-cli get "2001:m:30002" | cut -d: -f1)
sum=$(echo "$val1 + $val2" | bc)
redis-cli set "2001:m:30003" "$sum:$timestamp" >/dev/null

test_case "计算结果验证" \
    "redis-cli get '2001:m:30003' | cut -d: -f1" \
    "301.3"

echo ""

# 7. 性能测试
echo "=== 性能测试 ==="

# 写入1000个点
START_TIME=$(date +%s.%N)
for i in {1..1000}; do
    redis-cli set "1001:m:$((40000+i))" "$i.5:$timestamp" >/dev/null 2>&1
done &
wait
END_TIME=$(date +%s.%N)
WRITE_TIME=$(echo "$END_TIME - $START_TIME" | bc)

test_case "批量写入1000点耗时<5秒" \
    "echo '$WRITE_TIME < 5' | bc" \
    "1"

# 读取100个点
START_TIME=$(date +%s.%N)
keys=""
for i in {1..100}; do
    keys="$keys 1001:m:$((40000+i))"
done
redis-cli mget $keys >/dev/null 2>&1
END_TIME=$(date +%s.%N)
READ_TIME=$(echo "$END_TIME - $START_TIME" | bc)

test_case "批量读取100点耗时<0.5秒" \
    "echo '$READ_TIME < 0.5' | bc" \
    "1"

echo ""

# 8. 数据完整性测试
echo "=== 数据完整性测试 ==="

# 验证写入的数据
MISSING=0
for i in {1..10}; do
    val=$(redis-cli get "1001:m:$((40000+i))" 2>/dev/null)
    if [[ -z "$val" ]]; then
        ((MISSING++))
    fi
done

test_case "数据完整性检查" \
    "echo $MISSING" \
    "0"

echo ""

# 总结
echo "=== 测试总结 ==="
echo "通过: $PASSED"
echo "失败: $FAILED"
echo "结束时间: $(date)"
echo ""

# 清理测试数据
echo "清理测试数据..."
redis-cli --scan --pattern "1001:*" | xargs -r redis-cli del >/dev/null 2>&1
redis-cli --scan --pattern "cfg:1001:*" | xargs -r redis-cli del >/dev/null 2>&1
redis-cli --scan --pattern "2001:*" | xargs -r redis-cli del >/dev/null 2>&1

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}所有测试通过！${NC}"
    exit 0
else
    echo -e "${RED}有 $FAILED 个测试失败！${NC}"
    exit 1
fi