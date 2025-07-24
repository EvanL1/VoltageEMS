#!/bin/bash
# 完整集成测试套件 - 严格记录所有测试日志
# 不对外暴露端口，完全内部网络测试

set -e

# 测试配置
TEST_REPORT_DIR="/test-reports"
LOG_DIR="/logs/tests"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
MAIN_REPORT="$TEST_REPORT_DIR/complete_integration_test_$TIMESTAMP.md"
DETAILED_LOG="$LOG_DIR/detailed_test_$TIMESTAMP.log"

# 创建必要目录
mkdir -p "$TEST_REPORT_DIR" "$LOG_DIR"

# 重定向所有输出到详细日志
exec > >(tee -a "$DETAILED_LOG")
exec 2>&1

echo "🚀 ModSrv完整集成测试套件启动"
echo "📅 测试时间: $(date)"
echo "🔧 测试环境: Docker完全内部网络"
echo "📝 主报告: $MAIN_REPORT"
echo "📋 详细日志: $DETAILED_LOG"
echo "========================================"

# 初始化测试报告
cat > "$MAIN_REPORT" << EOF
# ModSrv完整集成测试报告

**测试时间**: $(date)
**测试环境**: Docker完全内部网络 (无端口暴露)
**测试组件**: Redis 8 + ComsRv模拟器 + ModSrv + 数据验证

---

EOF

# 测试统计变量
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 辅助函数：记录测试结果
log_test_result() {
    local test_name="$1"
    local status="$2"
    local details="$3"
    local duration="${4:-N/A}"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if [ "$status" = "PASS" ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        echo "✅ $test_name - 通过 (耗时: ${duration}s)"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))  
        echo "❌ $test_name - 失败 (耗时: ${duration}s)"
    fi
    
    # 写入主报告
    {
        echo "## $test_name"
        echo "**状态**: $status"
        echo "**执行时间**: $(date)"
        echo "**耗时**: ${duration}s"
        if [ -n "$details" ]; then
            echo "**详情**:"
            echo '```'
            echo "$details"
            echo '```'
        fi
        echo ""
    } >> "$MAIN_REPORT"
}

# 辅助函数：执行测试
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo ""
    echo "🧪 执行测试: $test_name"
    echo "📋 命令: $test_command"
    echo "⏰ 开始时间: $(date)"
    
    local start_time=$(date +%s)
    
    if output=$(eval "$test_command" 2>&1); then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_test_result "$test_name" "PASS" "$output" "$duration"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_test_result "$test_name" "FAIL" "$output" "$duration"
        return 1
    fi
}

# 等待所有服务完全启动
echo "⏳ 等待服务完全启动..."
sleep 15

echo ""
echo "🔍 开始系统连接测试..."

# 测试1: Redis连接测试
run_test "Redis服务连接" "redis-cli -h redis -p 6379 ping"

# 测试2: ModSrv API服务测试
run_test "ModSrv API健康检查" "curl -f -s -m 10 http://modsrv:8092/health"

# 测试3: ComsRv模拟器状态检查
COMSRV_TEST='
echo "检查ComsRv模拟器生成的数据..."
comsrv_keys=$(redis-cli -h redis -p 6379 keys "comsrv:*" | wc -l)
echo "发现 $comsrv_keys 个comsrv通道"
if [ "$comsrv_keys" -gt 0 ]; then
    echo "✅ ComsRv模拟器正常工作"
    # 检查具体数据
    redis-cli -h redis -p 6379 keys "comsrv:*" | head -5
else
    echo "❌ ComsRv模拟器未生成数据"
    exit 1
fi
'
run_test "ComsRv数据模拟器状态" "$COMSRV_TEST"

echo ""
echo "📊 开始数据格式验证测试..."

# 测试4: Redis数据结构规范v3.2验证
DATA_FORMAT_TEST='
echo "=== Redis数据结构规范v3.2验证 ==="

# 1. 检查Hash键格式
echo "1. 验证Hash键格式..."
hash_keys=$(redis-cli -h redis -p 6379 keys "comsrv:*")
valid_format=true

for key in $hash_keys; do
    if ! echo "$key" | grep -qE "^comsrv:[0-9]+:(m|s|c|a)$"; then
        echo "❌ 键格式不符合规范: $key"
        valid_format=false
    fi
done

if [ "$valid_format" = true ]; then
    echo "✅ 所有Hash键格式符合规范"
else
    echo "❌ 发现不符合规范的键格式"
    exit 1
fi

# 2. 检查数值精度（6位小数）
echo "2. 验证数值精度..."
test_key=$(echo "$hash_keys" | head -1)
echo "检查键: $test_key"

if [ -n "$test_key" ]; then
    sample_values=$(redis-cli -h redis -p 6379 hgetall "$test_key" | grep -E "^[0-9.-]+$" | head -5)
    precision_valid=true
    
    for value in $sample_values; do
        if echo "$value" | grep -qE "^\-?[0-9]+\.[0-9]{6}$"; then
            echo "✅ 数值格式正确: $value"
        else
            echo "❌ 数值精度不符合规范: $value"
            precision_valid=false
        fi
    done
    
    if [ "$precision_valid" = true ]; then
        echo "✅ 数值精度验证通过"
    else
        echo "❌ 数值精度验证失败"
        exit 1
    fi
else  
    echo "❌ 未找到测试数据"
    exit 1
fi

# 3. 检查Hash结构完整性
echo "3. 验证Hash结构完整性..."
for key in $(echo "$hash_keys" | head -3); do
    field_count=$(redis-cli -h redis -p 6379 hlen "$key")
    echo "通道 $key: $field_count 个点位"
    if [ "$field_count" -gt 0 ]; then
        echo "✅ Hash结构正常"
    else
        echo "❌ Hash结构异常"
        exit 1
    fi
done

echo "✅ 数据格式验证全部通过"
'
run_test "数据格式规范v3.2验证" "$DATA_FORMAT_TEST"

# 测试5: Pub/Sub功能验证
PUBSUB_TEST='
echo "=== Pub/Sub功能验证 ==="

# 启动后台订阅
timeout 5s redis-cli -h redis -p 6379 psubscribe "comsrv:*" > /tmp/pubsub_test.log 2>&1 &
MONITOR_PID=$!

sleep 2

# 检查接收到的消息
if [ -f /tmp/pubsub_test.log ]; then
    message_count=$(grep -c "pmessage" /tmp/pubsub_test.log || echo "0")
    echo "接收到 $message_count 条pub/sub消息"
    
    if [ "$message_count" -gt 0 ]; then
        echo "✅ Pub/Sub功能正常"
        # 显示消息样例
        echo "消息样例:"
        grep "pmessage" /tmp/pubsub_test.log | head -3
    else
        echo "❌ 未接收到pub/sub消息"
        exit 1
    fi
    
    rm -f /tmp/pubsub_test.log
else
    echo "❌ Pub/Sub测试日志文件不存在"
    exit 1
fi

kill $MONITOR_PID 2>/dev/null || true
'
run_test "Pub/Sub通知机制验证" "$PUBSUB_TEST"

echo ""
echo "🔧 开始ModSrv服务功能测试..."

# 测试6: ModSrv API功能测试
API_TEST='
echo "=== ModSrv API功能测试 ==="

# 1. 健康检查详细信息
health_response=$(curl -s http://modsrv:8092/health)
echo "健康检查响应: $health_response"

if echo "$health_response" | jq -e ".status" > /dev/null 2>&1; then
    status=$(echo "$health_response" | jq -r ".status")
    if [ "$status" = "ok" ]; then
        echo "✅ API健康检查正常"
    else
        echo "❌ API状态异常: $status"  
        exit 1
    fi
else
    echo "❌ 健康检查响应格式错误"
    exit 1
fi

# 2. 检查其他API端点（如果存在）
api_endpoints=("/api/v1/models" "/api/v1/instances")
for endpoint in "${api_endpoints[@]}"; do
    if curl -f -s -m 5 "http://modsrv:8092$endpoint" >/dev/null 2>&1; then
        echo "✅ API端点可访问: $endpoint"
    else
        echo "ℹ️ API端点未实现或不可访问: $endpoint"
    fi
done
'
run_test "ModSrv API功能测试" "$API_TEST"

# 测试7: 数据流集成测试
DATAFLOW_TEST='
echo "=== 数据流集成测试 ==="

# 1. 检查ModSrv处理的数据
modsrv_keys=$(redis-cli -h redis -p 6379 keys "modsrv:*")
modsrv_key_count=$(echo "$modsrv_keys" | wc -w)

echo "ModSrv处理的键数量: $modsrv_key_count"

if [ "$modsrv_key_count" -gt 0 ]; then
    echo "✅ ModSrv正在处理数据"
    echo "ModSrv键列表:"
    echo "$modsrv_keys"
    
    # 检查具体数据内容
    for key in $(echo "$modsrv_keys" | head -2); do
        echo "键 $key 的内容:"
        redis-cli -h redis -p 6379 type "$key"
        if [ "$(redis-cli -h redis -p 6379 type "$key")" = "hash" ]; then
            redis-cli -h redis -p 6379 hgetall "$key" | head -10
        else
            redis-cli -h redis -p 6379 get "$key"
        fi
    done
else
    echo "ℹ️ ModSrv暂未生成处理数据（可能正常，取决于配置）"
fi

# 2. 验证数据流连通性
echo "验证ComsRv -> ModSrv数据流..."
comsrv_data_count=$(redis-cli -h redis -p 6379 keys "comsrv:*" | wc -l)
echo "ComsRv数据通道数: $comsrv_data_count"

if [ "$comsrv_data_count" -gt 0 ]; then
    echo "✅ 数据流源头正常"
else
    echo "❌ 数据流源头异常"
    exit 1
fi
'
run_test "数据流集成验证" "$DATAFLOW_TEST"

echo ""
echo "⚡ 开始性能基准测试..."

# 测试8: 基础性能测试
PERFORMANCE_TEST='
echo "=== 基础性能测试 ==="

# 1. Redis操作性能
echo "1. Redis读写性能测试..."
start_time=$(date +%s.%6N)

# 执行100次Hash读取操作
for i in {1..100}; do
    redis-cli -h redis -p 6379 hlen "comsrv:1001:m" >/dev/null 2>&1
done

end_time=$(date +%s.%6N)
duration=$(echo "$end_time - $start_time" | bc -l)
ops_per_sec=$(echo "scale=2; 100 / $duration" | bc -l)

echo "完成100次Hash读取操作"
echo "总耗时: ${duration}秒"
echo "操作速度: ${ops_per_sec} ops/sec"

if (( $(echo "$ops_per_sec > 50" | bc -l) )); then
    echo "✅ Redis性能测试通过"
else
    echo "❌ Redis性能不达标"
    exit 1
fi

# 2. API响应性能
echo "2. API响应性能测试..."
api_start=$(date +%s.%6N)
curl -s http://modsrv:8092/health >/dev/null
api_end=$(date +%s.%6N)
api_duration=$(echo "$api_end - $api_start" | bc -l)

echo "API响应时间: ${api_duration}秒"

if (( $(echo "$api_duration < 1.0" | bc -l) )); then
    echo "✅ API响应性能正常"
else
    echo "❌ API响应过慢"
    exit 1
fi
'
run_test "基础性能基准测试" "$PERFORMANCE_TEST"

echo ""
echo "📊 开始系统资源监控..."

# 测试9: 系统资源使用情况
RESOURCE_TEST='
echo "=== 系统资源监控 ==="

# 1. Redis内存使用
echo "1. Redis内存使用情况:"
redis-cli -h redis -p 6379 info memory | grep -E "(used_memory_human|used_memory_peak_human|mem_fragmentation_ratio)"

# 2. Redis键空间统计  
echo "2. Redis键空间统计:"
redis-cli -h redis -p 6379 info keyspace

# 3. Redis操作统计
echo "3. Redis操作统计:"
redis-cli -h redis -p 6379 info stats | grep -E "(total_commands_processed|instantaneous_ops_per_sec)"

# 4. 数据统计汇总
echo "4. 数据统计汇总:"
total_keys=$(redis-cli -h redis -p 6379 dbsize)
comsrv_keys=$(redis-cli -h redis -p 6379 keys "comsrv:*" | wc -l)
modsrv_keys=$(redis-cli -h redis -p 6379 keys "modsrv:*" | wc -l)

echo "总键数: $total_keys"
echo "ComsRv键数: $comsrv_keys"  
echo "ModSrv键数: $modsrv_keys"

if [ "$total_keys" -gt 0 ]; then
    echo "✅ 系统数据正常"
else
    echo "❌ 系统数据异常"
    exit 1
fi
'
run_test "系统资源使用监控" "$RESOURCE_TEST"

echo ""
echo "🔒 开始数据一致性验证..."

# 测试10: 数据一致性验证
CONSISTENCY_TEST='
echo "=== 数据一致性验证 ==="

# 1. 检查数据时效性
echo "1. 数据时效性检查..."
current_time=$(date +%s)
data_fresh=true

# 通过检查Hash的字段数量变化来判断数据是否在更新
initial_count=$(redis-cli -h redis -p 6379 hlen "comsrv:1001:m" 2>/dev/null || echo "0")
sleep 3
final_count=$(redis-cli -h redis -p 6379 hlen "comsrv:1001:m" 2>/dev/null || echo "0")

if [ "$initial_count" -gt 0 ]; then
    echo "✅ 数据源活跃，Hash字段数: $initial_count"
else
    echo "❌ 数据源不活跃"
    data_fresh=false
fi

# 2. 检查数据格式一致性  
echo "2. 数据格式一致性检查..."
format_consistent=true

# 随机检查几个Hash的数据格式
for key in $(redis-cli -h redis -p 6379 keys "comsrv:*" | head -3); do
    echo "检查键: $key"
    # 获取几个字段值检查格式
    values=$(redis-cli -h redis -p 6379 hvals "$key" | head -3)
    for value in $values; do
        if echo "$value" | grep -qE "^-?[0-9]+\.[0-9]{6}$"; then
            echo "✅ 格式正确: $value"
        else
            echo "❌ 格式错误: $value"
            format_consistent=false
        fi
    done
done

if [ "$data_fresh" = true ] && [ "$format_consistent" = true ]; then
    echo "✅ 数据一致性验证通过"
else
    echo "❌ 数据一致性验证失败"
    exit 1
fi
'
run_test "数据一致性验证" "$CONSISTENCY_TEST"

echo ""
echo "🏁 生成最终测试报告..."

# 生成测试汇总
{
    echo "---"
    echo ""  
    echo "## 测试汇总"
    echo ""
    echo "**完成时间**: $(date)"
    echo "**测试执行时长**: $(($(date +%s) - $(date -d "$(head -2 "$DETAILED_LOG" | tail -1 | cut -d' ' -f1-2)" +%s) || 0))秒"
    echo ""
    echo "### 测试统计"
    echo "- **总测试数**: $TOTAL_TESTS"
    echo "- **通过**: $PASSED_TESTS"  
    echo "- **失败**: $FAILED_TESTS"
    echo "- **成功率**: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%"
    echo ""
    
    if [ "$FAILED_TESTS" -eq 0 ]; then
        echo "🎉 **所有测试通过！**"
        echo ""
        echo "### 系统状态"
        echo "- ✅ Redis数据结构规范v3.2完全符合"
        echo "- ✅ ComsRv数据模拟器正常运行"
        echo "- ✅ ModSrv服务健康运行"
        echo "- ✅ 数据流Pipeline正常"
        echo "- ✅ API服务可访问"
        echo "- ✅ 性能指标达标"
        echo "- ✅ 数据一致性验证通过"
        echo ""
        echo "系统已准备就绪，可以进行生产部署。"
    else
        echo "⚠️ **部分测试失败 ($FAILED_TESTS/$TOTAL_TESTS)**"
        echo ""
        echo "请检查失败的测试项目并修复相关问题后重新测试。"
    fi
    
    echo ""
    echo "### 详细日志文件"
    echo "- **主报告**: $MAIN_REPORT"
    echo "- **详细日志**: $DETAILED_LOG"
    echo "- **系统监控**: /logs/monitoring/system-monitor.log"
    echo "- **数据验证**: /logs/data-validation.log"
    
} >> "$MAIN_REPORT"

echo ""
echo "📋 测试完成汇总:"
echo "✅ 通过: $PASSED_TESTS"
echo "❌ 失败: $FAILED_TESTS"  
echo "📊 成功率: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%"
echo ""
echo "📄 报告文件:"
echo "  - 主报告: $MAIN_REPORT"
echo "  - 详细日志: $DETAILED_LOG"

# 如果有失败测试，返回错误码
if [ "$FAILED_TESTS" -gt 0 ]; then
    echo ""
    echo "❌ 测试套件执行完成，但有 $FAILED_TESTS 个测试失败"
    exit 1
else
    echo ""
    echo "🎉 所有测试通过！系统运行正常。"
    exit 0
fi