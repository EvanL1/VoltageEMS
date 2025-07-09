#!/bin/bash
# =============================================================================
# VoltageEMS集成测试脚本
# 功能：运行所有服务的集成测试
# =============================================================================

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
FAILED_LIST=()

# 测试环境配置
TEST_REDIS_URL="redis://localhost:6380"
TEST_API_URL="http://localhost:18080"
TEST_TIMEOUT=60

log_info "=========================================="
log_info "VoltageEMS 集成测试"
log_info "=========================================="

# 检查测试环境是否就绪
check_test_environment() {
    log_test "检查测试环境..."
    
    # 检查Redis
    if ! nc -z localhost 6380 2>/dev/null; then
        log_error "测试Redis未运行 (端口 6380)"
        return 1
    fi
    
    # 检查API网关
    if ! curl -s -f "${TEST_API_URL}/health" > /dev/null 2>&1; then
        log_warning "API网关未就绪，等待..."
        sleep 5
        if ! curl -s -f "${TEST_API_URL}/health" > /dev/null 2>&1; then
            log_error "API网关未运行"
            return 1
        fi
    fi
    
    log_info "测试环境就绪"
    return 0
}

# 运行单个测试
run_test() {
    local test_name=$1
    local test_command=$2
    
    ((TOTAL_TESTS++))
    log_test "运行测试: ${test_name}"
    
    if eval "${test_command}"; then
        ((PASSED_TESTS++))
        log_info "✓ ${test_name} 通过"
        return 0
    else
        ((FAILED_TESTS++))
        FAILED_LIST+=("${test_name}")
        log_error "✗ ${test_name} 失败"
        return 1
    fi
}

# API健康检查测试
test_api_health() {
    # 测试所有服务的健康端点
    local services=("comsrv:18081" "apigateway:18080")
    
    for service_port in "${services[@]}"; do
        service=$(echo $service_port | cut -d: -f1)
        port=$(echo $service_port | cut -d: -f2)
        
        if curl -s -f "http://localhost:${port}/health" | grep -q "ok\|healthy"; then
            return 0
        else
            return 1
        fi
    done
}

# Redis连接测试
test_redis_connection() {
    # 使用redis-cli测试连接
    if command -v redis-cli > /dev/null 2>&1; then
        redis-cli -p 6380 ping > /dev/null 2>&1
    else
        # 如果没有redis-cli，使用nc检查端口
        nc -z localhost 6380
    fi
}

# Comsrv通信测试
test_comsrv_communication() {
    # 发送测试请求到comsrv
    response=$(curl -s -X GET "http://localhost:18081/api/v1/channels" 2>/dev/null || echo "failed")
    
    if [[ "$response" != "failed" ]] && [[ "$response" != *"error"* ]]; then
        return 0
    else
        return 1
    fi
}

# 数据流测试
test_data_flow() {
    # 测试数据从comsrv到Redis的流转
    # 这里可以添加具体的数据流测试逻辑
    
    # 模拟写入测试数据
    if command -v redis-cli > /dev/null 2>&1; then
        redis-cli -p 6380 SET "test:integration:key" "test_value" EX 60 > /dev/null 2>&1
        value=$(redis-cli -p 6380 GET "test:integration:key" 2>/dev/null)
        
        if [[ "$value" == "test_value" ]]; then
            redis-cli -p 6380 DEL "test:integration:key" > /dev/null 2>&1
            return 0
        fi
    fi
    
    return 1
}

# API网关路由测试
test_api_gateway_routing() {
    # 测试API网关是否正确路由请求
    endpoints=(
        "/api/v1/health"
        "/api/v1/system/info"
    )
    
    for endpoint in "${endpoints[@]}"; do
        if ! curl -s -f "${TEST_API_URL}${endpoint}" > /dev/null 2>&1; then
            return 1
        fi
    done
    
    return 0
}

# 性能测试（简单版）
test_basic_performance() {
    # 测试基本的响应时间
    start_time=$(date +%s%N)
    
    for i in {1..10}; do
        curl -s "${TEST_API_URL}/health" > /dev/null 2>&1
    done
    
    end_time=$(date +%s%N)
    duration=$(( (end_time - start_time) / 1000000 )) # 转换为毫秒
    avg_duration=$((duration / 10))
    
    log_info "平均响应时间: ${avg_duration}ms"
    
    # 如果平均响应时间超过1000ms，认为性能有问题
    if [ $avg_duration -lt 1000 ]; then
        return 0
    else
        return 1
    fi
}

# 主测试流程
main() {
    # 检查测试环境
    if ! check_test_environment; then
        log_error "测试环境未就绪"
        exit 1
    fi
    
    log_info "开始运行集成测试..."
    echo ""
    
    # 运行各项测试
    run_test "API健康检查" test_api_health || true
    run_test "Redis连接测试" test_redis_connection || true
    run_test "Comsrv通信测试" test_comsrv_communication || true
    run_test "数据流测试" test_data_flow || true
    run_test "API网关路由测试" test_api_gateway_routing || true
    run_test "基础性能测试" test_basic_performance || true
    
    # 运行服务特定的集成测试
    if [ -f "services/comsrv/scripts/run-integration-test.sh" ]; then
        log_info "运行comsrv集成测试..."
        ./services/comsrv/scripts/run-integration-test.sh || true
    fi
    
    # 显示测试结果
    echo ""
    log_info "=========================================="
    log_info "测试结果汇总"
    log_info "总测试数: ${TOTAL_TESTS}"
    log_info "通过: ${PASSED_TESTS}"
    log_error "失败: ${FAILED_TESTS}"
    
    if [ ${FAILED_TESTS} -gt 0 ]; then
        log_error "失败的测试:"
        for failed in "${FAILED_LIST[@]}"; do
            echo "  - ${failed}"
        done
    fi
    
    log_info "=========================================="
    
    # 生成测试报告
    REPORT_FILE="test-report-$(date +%Y%m%d-%H%M%S).txt"
    cat > "${REPORT_FILE}" << EOF
VoltageEMS 集成测试报告
生成时间: $(date)
========================================

测试环境:
- Redis URL: ${TEST_REDIS_URL}
- API URL: ${TEST_API_URL}

测试结果:
- 总测试数: ${TOTAL_TESTS}
- 通过: ${PASSED_TESTS}
- 失败: ${FAILED_TESTS}
- 成功率: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%

失败的测试:
EOF
    
    if [ ${FAILED_TESTS} -gt 0 ]; then
        for failed in "${FAILED_LIST[@]}"; do
            echo "- ${failed}" >> "${REPORT_FILE}"
        done
    else
        echo "无" >> "${REPORT_FILE}"
    fi
    
    log_info "测试报告已保存到: ${REPORT_FILE}"
    
    # 返回状态码
    if [ ${FAILED_TESTS} -eq 0 ]; then
        exit 0
    else
        exit 1
    fi
}

# 运行主函数
main