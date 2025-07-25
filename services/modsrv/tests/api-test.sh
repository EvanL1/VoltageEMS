#!/bin/bash
# API测试脚本 - 测试ModSrv的REST API接口

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
MODSRV_URL="${MODSRV_URL:-http://modsrv:8092}"
RESULT_DIR="${TEST_OUTPUT:-/app/results}"
API_MESSAGES_DIR="${RESULT_DIR}/api-messages"
LOG_FILE="${LOG_FILE:-${RESULT_DIR}/api_test.log}"

# 创建目录
mkdir -p "${API_MESSAGES_DIR}/health_check"
mkdir -p "${API_MESSAGES_DIR}/model_list"
mkdir -p "${API_MESSAGES_DIR}/model_detail"
mkdir -p "${API_MESSAGES_DIR}/control_commands"
mkdir -p "${API_MESSAGES_DIR}/performance"

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOG_FILE"
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1" | tee -a "$LOG_FILE"
}

log_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}[PASS]${NC} $2" | tee -a "$LOG_FILE"
    else
        echo -e "${RED}[FAIL]${NC} $2" | tee -a "$LOG_FILE"
    fi
}

# 保存API响应
save_response() {
    local category=$1
    local endpoint=$2
    local timestamp=$3
    local response=$4
    
    # 清理endpoint名称用作文件名
    local clean_endpoint=$(echo "$endpoint" | sed 's/\//_/g' | sed 's/^_//')
    local filename="${API_MESSAGES_DIR}/${category}/${clean_endpoint}_${timestamp}.json"
    
    echo "$response" | jq '.' > "$filename" 2>/dev/null || echo "$response" > "$filename"
}

# 等待服务就绪
wait_for_service() {
    log_info "等待ModSrv服务就绪..."
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -sf "${MODSRV_URL}/health" > /dev/null 2>&1; then
            log_info "ModSrv服务已就绪"
            return 0
        fi
        attempt=$((attempt + 1))
        log_info "等待服务启动... ($attempt/$max_attempts)"
        sleep 2
    done
    
    log_error "ModSrv服务未能在预期时间内启动"
    return 1
}

# 测试健康检查接口
test_health_check() {
    log_test "测试健康检查接口"
    local timestamp=$(date +%s%3N)
    
    response=$(curl -s -X GET "${MODSRV_URL}/health")
    status_code=$?
    
    save_response "health_check" "get__health" "$timestamp" "$response"
    
    if [ $status_code -eq 0 ] && echo "$response" | jq -e '.status == "healthy"' > /dev/null 2>&1; then
        log_result 0 "健康检查接口正常"
        return 0
    else
        log_result 1 "健康检查接口异常"
        return 1
    fi
}

# 测试获取模型列表
test_get_models() {
    log_test "测试获取模型列表"
    local timestamp=$(date +%s%3N)
    
    response=$(curl -s -X GET "${MODSRV_URL}/models")
    status_code=$?
    
    save_response "model_list" "get__models" "$timestamp" "$response"
    
    if [ $status_code -eq 0 ] && echo "$response" | jq -e '.models | type == "array"' > /dev/null 2>&1; then
        local model_count=$(echo "$response" | jq '.models | length')
        log_result 0 "获取模型列表成功，共有 $model_count 个模型"
        return 0
    else
        log_result 1 "获取模型列表失败"
        return 1
    fi
}

# 测试获取单个模型详情
test_get_model_detail() {
    local model_id=$1
    log_test "测试获取模型详情: $model_id"
    local timestamp=$(date +%s%3N)
    
    response=$(curl -s -X GET "${MODSRV_URL}/models/${model_id}")
    status_code=$?
    
    save_response "model_detail" "get__models_${model_id}" "$timestamp" "$response"
    
    if [ $status_code -eq 0 ] && echo "$response" | jq -e '.id' > /dev/null 2>&1; then
        log_result 0 "获取模型 $model_id 详情成功"
        return 0
    else
        log_result 1 "获取模型 $model_id 详情失败"
        return 1
    fi
}

# 测试控制命令
test_control_command() {
    local model_id=$1
    local control_id=$2
    local value=$3
    
    log_test "测试控制命令: $model_id/$control_id"
    local timestamp=$(date +%s%3N)
    
    local payload=$(cat <<EOF
{
    "value": $value,
    "user": "api_test",
    "reason": "API测试控制命令"
}
EOF
)
    
    response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$payload" \
        "${MODSRV_URL}/models/${model_id}/controls/${control_id}")
    status_code=$?
    
    save_response "control_commands" "post__models_${model_id}_controls_${control_id}" "$timestamp" "$response"
    
    if [ $status_code -eq 0 ] && echo "$response" | jq -e '.success == true' > /dev/null 2>&1; then
        log_result 0 "控制命令 $model_id/$control_id 执行成功"
        return 0
    else
        log_result 1 "控制命令 $model_id/$control_id 执行失败"
        return 1
    fi
}

# 批量测试API性能
test_api_performance() {
    log_test "测试API性能"
    local timestamp=$(date +%s%3N)
    local num_requests=100
    local concurrent=10
    
    # 测试健康检查接口性能
    log_info "测试健康检查接口性能 ($num_requests 请求, $concurrent 并发)"
    
    performance_result=$(ab -n $num_requests -c $concurrent -g /tmp/health_perf.tsv "${MODSRV_URL}/health" 2>&1)
    
    # 提取关键性能指标
    local requests_per_sec=$(echo "$performance_result" | grep "Requests per second" | awk '{print $4}')
    local time_per_request=$(echo "$performance_result" | grep "Time per request" | grep "(mean)" | awk '{print $4}')
    
    # 保存性能测试结果
    local perf_summary=$(cat <<EOF
{
    "endpoint": "/health",
    "timestamp": "$timestamp",
    "total_requests": $num_requests,
    "concurrency": $concurrent,
    "requests_per_second": "$requests_per_sec",
    "mean_time_per_request_ms": "$time_per_request"
}
EOF
)
    
    save_response "performance" "performance_test" "$timestamp" "$perf_summary"
    
    log_info "性能测试结果: $requests_per_sec req/s, 平均响应时间: ${time_per_request}ms"
}

# 生成测试报告
generate_report() {
    local report_file="${RESULT_DIR}/api_test_report_$(date +%s).json"
    
    # 统计各类API消息
    local health_count=$(find "${API_MESSAGES_DIR}/health_check" -name "*.json" | wc -l)
    local model_list_count=$(find "${API_MESSAGES_DIR}/model_list" -name "*.json" | wc -l)
    local model_detail_count=$(find "${API_MESSAGES_DIR}/model_detail" -name "*.json" | wc -l)
    local control_count=$(find "${API_MESSAGES_DIR}/control_commands" -name "*.json" | wc -l)
    
    cat <<EOF > "$report_file"
{
    "test_time": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "api_messages": {
        "health_check": $health_count,
        "model_list": $model_list_count,
        "model_detail": $model_detail_count,
        "control_commands": $control_count
    },
    "test_results": {
        "total_tests": $((TESTS_PASSED + TESTS_FAILED)),
        "passed": $TESTS_PASSED,
        "failed": $TESTS_FAILED
    }
}
EOF
    
    log_info "测试报告已生成: $report_file"
}

# 主测试流程
main() {
    log_info "开始ModSrv API测试"
    log_info "目标URL: $MODSRV_URL"
    log_info "结果目录: $RESULT_DIR"
    
    TESTS_PASSED=0
    TESTS_FAILED=0
    
    # 等待服务就绪
    if ! wait_for_service; then
        exit 1
    fi
    
    # 执行测试
    if test_health_check; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    if test_get_models; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # 测试特定模型
    if test_get_model_detail "power_meter_demo"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # 测试控制命令
    if test_control_command "power_meter_demo" "main_switch" "1"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    if test_control_command "power_meter_demo" "power_limit" "5000.0"; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # 性能测试（仅在安装了ab工具时执行）
    if command -v ab > /dev/null 2>&1; then
        test_api_performance
    else
        log_info "跳过性能测试（未安装Apache Bench工具）"
    fi
    
    # 生成报告
    generate_report
    
    # 输出总结
    log_info "========================================="
    log_info "API测试完成"
    log_info "通过: $TESTS_PASSED"
    log_info "失败: $TESTS_FAILED"
    log_info "========================================="
    
    if [ $TESTS_FAILED -gt 0 ]; then
        return 1
    else
        return 0
    fi
}

# 执行主函数
main
exit $?