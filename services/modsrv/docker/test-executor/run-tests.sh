#!/bin/bash
# ModSrv完整测试执行器

set -e

echo "🚀 开始ModSrv完整测试流程"

# 环境变量
REDIS_URL=${REDIS_URL:-"redis://redis:6379"}
MODSRV_URL=${MODSRV_URL:-"http://modsrv:8082"}
TEST_OUTPUT=${TEST_OUTPUT:-"/app/results"}
LOG_FILE="$TEST_OUTPUT/test-execution.log"

# 创建结果目录
mkdir -p "$TEST_OUTPUT"

# 日志函数
log() {
    local level=$1
    shift
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [$level] $*" | tee -a "$LOG_FILE"
}

log "INFO" "测试环境配置："
log "INFO" "  - Redis URL: $REDIS_URL"
log "INFO" "  - ModSrv URL: $MODSRV_URL"
log "INFO" "  - 结果目录: $TEST_OUTPUT"

# 等待服务就绪
log "INFO" "等待服务就绪..."

# 等待Redis
for i in {1..60}; do
    if redis-cli -u "$REDIS_URL" ping > /dev/null 2>&1; then
        log "INFO" "Redis服务就绪"
        break
    fi
    if [ $i -eq 60 ]; then
        log "ERROR" "Redis服务启动超时"
        exit 1
    fi
    sleep 1
done

# 等待ModSrv
for i in {1..60}; do
    if curl -f "$MODSRV_URL/health" > /dev/null 2>&1; then
        log "INFO" "ModSrv服务就绪"
        break
    fi
    if [ $i -eq 60 ]; then
        log "ERROR" "ModSrv服务启动超时"
        exit 1
    fi
    sleep 2
done

# 等待ComsRv模拟器产生数据
log "INFO" "等待ComsRv模拟器产生数据..."
sleep 10

# 测试函数
run_test() {
    local test_name=$1
    local test_description=$2
    local test_command=$3
    
    log "INFO" "开始测试: $test_name - $test_description"
    
    local start_time=$(date +%s)
    local test_result_file="$TEST_OUTPUT/${test_name}.result"
    
    if eval "$test_command" > "$test_result_file" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log "INFO" "✅ 测试通过: $test_name (耗时: ${duration}s)"
        echo "PASS" >> "$test_result_file"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log "ERROR" "❌ 测试失败: $test_name (耗时: ${duration}s)"
        echo "FAIL" >> "$test_result_file"
        return 1
    fi
}

# 开始测试执行
total_tests=0
passed_tests=0

# 1. Redis连接测试
total_tests=$((total_tests + 1))
if run_test "redis_connection" "Redis连接测试" "redis-cli -u '$REDIS_URL' ping"; then
    passed_tests=$((passed_tests + 1))
fi

# 2. ModSrv健康检查
total_tests=$((total_tests + 1))
if run_test "modsrv_health" "ModSrv健康检查" "curl -f '$MODSRV_URL/health'"; then
    passed_tests=$((passed_tests + 1))
fi

# 3. ComsRv数据验证
total_tests=$((total_tests + 1))
if run_test "comsrv_data" "ComsRv数据验证" "python3 /app/test-comsrv-data.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 4. API功能完整测试
total_tests=$((total_tests + 1))
if run_test "api_comprehensive" "API功能完整测试" "python3 /app/api_test_suite.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 5. Redis数据格式验证
total_tests=$((total_tests + 1))
if run_test "redis_format" "Redis数据格式验证" "python3 /app/test-redis-format.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 6. 实例创建和管理测试
total_tests=$((total_tests + 1))
if run_test "instance_management" "实例创建和管理测试" "python3 /app/test-instance-management.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 7. 遥测数据获取测试
total_tests=$((total_tests + 1))
if run_test "telemetry_retrieval" "遥测数据获取测试" "python3 /app/test-telemetry.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 8. 命令执行测试
total_tests=$((total_tests + 1))
if run_test "command_execution" "命令执行测试" "python3 /app/test-commands.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 9. 负载测试
total_tests=$((total_tests + 1))
if run_test "load_test" "负载测试" "python3 /app/test-load.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 10. 数据持续性测试
total_tests=$((total_tests + 1))
if run_test "data_persistence" "数据持续性测试" "python3 /app/test-persistence.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 11. 模板系统测试
total_tests=$((total_tests + 1))
if run_test "template_system" "模板系统测试" "python3 /app/test-template-system.py"; then
    passed_tests=$((passed_tests + 1))
fi

# 生成测试报告
report_file="$TEST_OUTPUT/test-report.json"
cat > "$report_file" << EOF
{
    "test_execution": {
        "timestamp": "$(date -Iseconds)",
        "total_tests": $total_tests,
        "passed_tests": $passed_tests,
        "failed_tests": $((total_tests - passed_tests)),
        "success_rate": $(echo "scale=2; $passed_tests * 100 / $total_tests" | bc -l)
    },
    "environment": {
        "redis_url": "$REDIS_URL",
        "modsrv_url": "$MODSRV_URL"
    },
    "test_results": [
EOF

# 添加详细测试结果
first=true
for result_file in "$TEST_OUTPUT"/*.result; do
    if [ -f "$result_file" ]; then
        test_name=$(basename "$result_file" .result)
        result=$(tail -n 1 "$result_file")
        
        if [ "$first" = false ]; then
            echo "," >> "$report_file"
        fi
        first=false
        
        echo "        {" >> "$report_file"
        echo "            \"name\": \"$test_name\"," >> "$report_file"
        echo "            \"result\": \"$result\"" >> "$report_file"
        echo -n "        }" >> "$report_file"
    fi
done

cat >> "$report_file" << EOF

    ]
}
EOF

# 生成测试摘要报告
log "INFO" "生成测试摘要报告..."
if python3 /app/generate-test-summary.py --results-dir "$TEST_OUTPUT" --output "$TEST_OUTPUT/final_test_summary.json" > /dev/null 2>&1; then
    log "INFO" "✅ 测试摘要报告生成成功"
else
    log "WARN" "⚠️  测试摘要报告生成失败，但不影响测试结果"
fi

# 输出最终结果
log "INFO" "测试执行完成"
log "INFO" "总测试数: $total_tests"
log "INFO" "通过测试: $passed_tests"
log "INFO" "失败测试: $((total_tests - passed_tests))"
log "INFO" "成功率: $(echo "scale=1; $passed_tests * 100 / $total_tests" | bc -l)%"

if [ $passed_tests -eq $total_tests ]; then
    log "INFO" "🎉 所有测试通过！"
    exit 0
else
    log "ERROR" "⚠️  部分测试失败，请检查日志"
    exit 1
fi