#!/bin/bash
# ModSrv集成测试套件
# 包含comsrv数据模拟器、Redis数据流、API功能测试

set -e

# 测试配置
TEST_REPORT_DIR="/test-reports"
LOG_DIR="/logs"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="$TEST_REPORT_DIR/integration_test_$TIMESTAMP.md"

# 创建报告目录
mkdir -p "$TEST_REPORT_DIR"

# 初始化测试报告
cat > "$REPORT_FILE" << EOF
# ModSrv集成测试报告

**测试时间**: $(date)
**测试环境**: Docker内部网络
**测试组件**: ComsRv模拟器 + Redis + ModSrv + API

---

EOF

echo "🚀 开始ModSrv集成测试套件"
echo "📝 测试报告将保存到: $REPORT_FILE"

# 辅助函数：添加测试结果到报告
add_test_result() {
    local test_name="$1"
    local status="$2" 
    local details="$3"
    
    {
        echo "## $test_name"
        echo "**状态**: $status"
        echo "**时间**: $(date)"
        if [ -n "$details" ]; then
            echo "**详情**:"
            echo '```'
            echo "$details"
            echo '```'
        fi
        echo ""
    } >> "$REPORT_FILE"
}

# 辅助函数：运行测试并记录结果
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo "🧪 运行测试: $test_name"
    
    if output=$(eval "$test_command" 2>&1); then
        echo "✅ $test_name - 通过"  
        add_test_result "$test_name" "✅ 通过" "$output"
        return 0
    else
        echo "❌ $test_name - 失败"
        add_test_result "$test_name" "❌ 失败" "$output"
        return 1
    fi
}

# 等待所有服务准备就绪
echo "⏳ 等待服务启动..."
sleep 10

# 测试1: 基础连接测试
run_test "Redis连接测试" "redis-cli -h redis -p 6379 ping"

run_test "ModSrv API健康检查" "curl -f -s http://modsrv:8092/health"

# 测试2: ComsRv数据模拟器测试
run_test "ComsRv数据生成验证" "/scripts/test-data-flow.sh"

# 测试3: ModSrv订阅功能测试  
run_test "ModSrv数据订阅验证" "/scripts/verify-modsrv-subscription.sh"

# 测试4: API功能测试
echo "🔌 测试API功能..."

# 4.1 健康检查端点
run_test "API健康检查端点" "curl -f -s http://modsrv:8092/health | jq '.status'"

# 4.2 尝试获取设备模型信息（如果有的话）
if curl -f -s http://modsrv:8092/api/v1/models > /dev/null 2>&1; then
    run_test "设备模型API" "curl -f -s http://modsrv:8092/api/v1/models"
fi

# 4.3 尝试获取实例信息
if curl -f -s http://modsrv:8092/api/v1/instances > /dev/null 2>&1; then
    run_test "设备实例API" "curl -f -s http://modsrv:8092/api/v1/instances"
fi

# 测试5: 数据一致性测试
echo "🔍 运行数据一致性测试..."

# 检查comsrv生成的数据格式
CONSISTENCY_TEST=$(cat << 'EOF'
# 检查数据格式一致性
echo "检查Hash键格式..."
redis-cli -h redis -p 6379 keys "comsrv:*" | head -5

echo "检查数值精度..."  
value=$(redis-cli -h redis -p 6379 hget "comsrv:1001:m" "10001")
if echo "$value" | grep -qE '^[0-9]+\.[0-9]{6}$'; then
    echo "✅ 数值格式正确: $value"
else
    echo "❌ 数值格式错误: $value"
    exit 1
fi

echo "检查通道数量..."
channel_count=$(redis-cli -h redis -p 6379 keys "comsrv:*" | wc -l)
echo "发现 $channel_count 个通道"

if [ "$channel_count" -ge 6 ]; then
    echo "✅ 通道数量正常"
else
    echo "❌ 通道数量不足"
    exit 1
fi
EOF
)

run_test "数据一致性验证" "$CONSISTENCY_TEST"

# 测试6: 性能基准测试
echo "⚡ 运行基础性能测试..."

PERFORMANCE_TEST=$(cat << 'EOF'
# 简单的性能测试
echo "测试Redis操作性能..."
start_time=$(date +%s.%6N)

# 执行100次Redis操作
for i in {1..100}; do
    redis-cli -h redis -p 6379 hget "comsrv:1001:m" "10001" > /dev/null
done

end_time=$(date +%s.%6N) 
duration=$(echo "$end_time - $start_time" | bc -l)
ops_per_sec=$(echo "scale=2; 100 / $duration" | bc -l)

echo "完成100次Redis读取操作"
echo "耗时: ${duration}秒"
echo "性能: ${ops_per_sec} ops/sec"

# 基本性能要求：至少100 ops/sec
if (( $(echo "$ops_per_sec > 100" | bc -l) )); then
    echo "✅ 性能测试通过"
else
    echo "❌ 性能测试失败"  
    exit 1
fi
EOF
)

run_test "基础性能测试" "$PERFORMANCE_TEST"

# 测试7: 资源使用情况
echo "📊 收集系统资源信息..."

RESOURCE_TEST=$(cat << 'EOF'
echo "Docker容器资源使用情况:"
echo "Redis内存使用:"
redis-cli -h redis -p 6379 info memory | grep "used_memory_human"

echo "Redis键统计:"
redis-cli -h redis -p 6379 info keyspace

echo "进程信息:"
ps aux | grep -E "(redis|modsrv|python)" | grep -v grep
EOF
)

run_test "资源使用情况" "$RESOURCE_TEST"

# 生成测试总结
echo "📋 生成测试总结..."

{
    echo "---"
    echo ""
    echo "## 测试总结"
    echo ""
    echo "**完成时间**: $(date)"
    echo ""
    
    # 统计测试结果
    passed=$(grep -c "✅ 通过" "$REPORT_FILE" || echo "0")
    failed=$(grep -c "❌ 失败" "$REPORT_FILE" || echo "0")
    total=$((passed + failed))
    
    echo "**测试统计**:"
    echo "- 总测试数: $total"
    echo "- 通过: $passed"
    echo "- 失败: $failed"
    echo "- 成功率: $(( passed * 100 / total ))%"
    echo ""
    
    if [ "$failed" -eq 0 ]; then
        echo "🎉 **所有测试通过！**"
        echo ""
        echo "系统状态良好，可以进行下一步开发。"
    else
        echo "⚠️ **部分测试失败**"
        echo ""
        echo "请检查失败的测试项目并修复相关问题。"
    fi
    
} >> "$REPORT_FILE"

echo "✅ 集成测试完成"
echo "📄 详细报告: $REPORT_FILE"

# 显示测试概要
echo ""
echo "🏁 测试概要:"
echo "通过: $(grep -c "✅ 通过" "$REPORT_FILE" || echo "0")"
echo "失败: $(grep -c "❌ 失败" "$REPORT_FILE" || echo "0")"

# 如果有失败，退出码为1
if grep -q "❌ 失败" "$REPORT_FILE"; then
    exit 1
fi

exit 0