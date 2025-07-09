#!/bin/bash
# =============================================================================
# Comsrv集成测试脚本
# 功能：运行comsrv服务的完整集成测试
# =============================================================================

set -e

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
SERVICE_DIR="$(dirname "${SCRIPT_DIR}")"

# 测试结果
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

log_info "=========================================="
log_info "Comsrv 集成测试"
log_info "=========================================="

# 启动测试环境
setup_test_environment() {
    log_info "设置测试环境..."
    
    # 启动测试服务器
    "${SCRIPT_DIR}/start-test-servers.sh"
    
    # 设置环境变量
    export RUST_LOG=debug
    export REDIS_URL="redis://localhost:6379"
    export COMSRV_TEST_MODE=true
    export COMSRV_CSV_BASE_PATH="${SERVICE_DIR}/config"
    
    # 创建测试配置
    create_test_config
}

# 创建测试配置文件
create_test_config() {
    log_info "创建测试配置..."
    
    local TEST_CONFIG="${SERVICE_DIR}/test-config.yaml"
    
    cat > "${TEST_CONFIG}" << 'EOF'
service:
  name: "comsrv-test"
  http_port: 3001
  metrics_port: 9091
  logging:
    level: "debug"
    console: true

redis:
  url: "redis://localhost:6379"
  pool_size: 5

channels:
  - name: "test_modbus_tcp"
    enabled: true
    protocol: "modbus"
    transport: "tcp"
    params:
      address: "localhost:5502"
      timeout_ms: 3000
      retry_count: 3
    polling:
      interval_ms: 1000
      batch_size: 10
    csv_path: "Modbus_TCP_Test"

  - name: "test_modbus_tcp_2"
    enabled: true
    protocol: "modbus"  
    transport: "tcp"
    params:
      address: "localhost:5503"
      timeout_ms: 3000
    polling:
      interval_ms: 2000
    csv_path: "Modbus_TCP_Test_2"
EOF
    
    export COMSRV_CONFIG="${TEST_CONFIG}"
}

# 运行测试
run_test() {
    local test_name=$1
    local test_command=$2
    
    ((TOTAL_TESTS++))
    log_test "运行: ${test_name}"
    
    if eval "${test_command}"; then
        ((PASSED_TESTS++))
        log_info "✓ ${test_name} 通过"
        return 0
    else
        ((FAILED_TESTS++))
        log_error "✗ ${test_name} 失败"
        return 1
    fi
}

# 测试Comsrv启动
test_comsrv_startup() {
    cd "${SERVICE_DIR}"
    
    # 启动comsrv
    cargo run -- --config test-config.yaml > comsrv-test.log 2>&1 &
    local COMSRV_PID=$!
    echo $COMSRV_PID > .test-pids/comsrv.pid
    
    # 等待启动
    sleep 5
    
    # 检查进程
    if ps -p $COMSRV_PID > /dev/null; then
        # 检查健康端点
        if curl -s -f "http://localhost:3001/health" | grep -q "ok\|healthy"; then
            return 0
        fi
    fi
    
    # 显示错误日志
    tail -n 20 comsrv-test.log
    return 1
}

# 测试Redis连接
test_redis_connection() {
    # 检查comsrv是否能写入Redis
    sleep 2
    
    # 检查是否有数据写入Redis
    local keys=$(redis-cli keys "comsrv:*" | wc -l)
    if [ "$keys" -gt 0 ]; then
        return 0
    else
        return 1
    fi
}

# 测试Modbus通信
test_modbus_communication() {
    # 等待一些轮询周期
    sleep 5
    
    # 检查是否有遥测数据
    local telemetry_keys=$(redis-cli keys "comsrv:telemetry:*" | wc -l)
    if [ "$telemetry_keys" -gt 0 ]; then
        log_info "找到 ${telemetry_keys} 个遥测点"
        
        # 显示一些数据样本
        log_info "数据样本:"
        redis-cli --scan --pattern "comsrv:telemetry:*" | head -5 | while read key; do
            value=$(redis-cli get "$key")
            echo "  $key = $value"
        done
        
        return 0
    else
        return 1
    fi
}

# 测试API端点
test_api_endpoints() {
    local base_url="http://localhost:3001/api/v1"
    
    # 测试channels端点
    if curl -s -f "${base_url}/channels" | grep -q "test_modbus_tcp"; then
        log_info "Channels API正常"
    else
        return 1
    fi
    
    # 测试points端点
    if curl -s -f "${base_url}/points" > /dev/null; then
        log_info "Points API正常"
    else
        return 1
    fi
    
    return 0
}

# 测试控制命令
test_control_commands() {
    # 发送控制命令
    local response=$(curl -s -X POST "${base_url}/control" \
        -H "Content-Type: application/json" \
        -d '{
            "channel": "test_modbus_tcp",
            "point_id": 1001,
            "value": 100
        }')
    
    if [[ "$response" != *"error"* ]]; then
        return 0
    else
        return 1
    fi
}

# 性能测试
test_performance() {
    log_info "运行简单性能测试..."
    
    # 测试读取性能
    local start_time=$(date +%s%N)
    
    for i in {1..100}; do
        curl -s "http://localhost:3001/api/v1/points" > /dev/null
    done
    
    local end_time=$(date +%s%N)
    local duration=$(( (end_time - start_time) / 1000000 ))
    local avg_time=$((duration / 100))
    
    log_info "100次API调用总耗时: ${duration}ms"
    log_info "平均响应时间: ${avg_time}ms"
    
    # 如果平均响应时间小于50ms，认为性能合格
    if [ $avg_time -lt 50 ]; then
        return 0
    else
        return 1
    fi
}

# 清理函数
cleanup() {
    log_info "清理测试环境..."
    
    # 停止comsrv
    if [ -f ".test-pids/comsrv.pid" ]; then
        kill $(cat .test-pids/comsrv.pid) 2>/dev/null || true
        rm -f .test-pids/comsrv.pid
    fi
    
    # 停止测试服务器
    "${SCRIPT_DIR}/stop-test-servers.sh"
    
    # 清理测试数据
    redis-cli --scan --pattern "comsrv:test:*" | xargs -r redis-cli del 2>/dev/null || true
    
    # 删除测试配置
    rm -f "${SERVICE_DIR}/test-config.yaml"
    rm -f "${SERVICE_DIR}/comsrv-test.log"
}

# 主测试流程
main() {
    # 设置清理钩子
    trap cleanup EXIT
    
    # 设置测试环境
    setup_test_environment
    
    echo ""
    log_info "开始集成测试..."
    echo ""
    
    # 运行测试
    run_test "Comsrv启动测试" test_comsrv_startup || true
    run_test "Redis连接测试" test_redis_connection || true
    run_test "Modbus通信测试" test_modbus_communication || true
    run_test "API端点测试" test_api_endpoints || true
    run_test "控制命令测试" test_control_commands || true
    run_test "性能测试" test_performance || true
    
    # 显示测试结果
    echo ""
    log_info "=========================================="
    log_info "测试结果"
    log_info "总数: ${TOTAL_TESTS}"
    log_info "通过: ${PASSED_TESTS}"
    log_error "失败: ${FAILED_TESTS}"
    log_info "=========================================="
    
    # 生成测试报告
    generate_test_report
    
    # 返回状态
    if [ ${FAILED_TESTS} -eq 0 ]; then
        exit 0
    else
        exit 1
    fi
}

# 生成测试报告
generate_test_report() {
    local report_file="${SERVICE_DIR}/integration-test-report.txt"
    
    cat > "${report_file}" << EOF
Comsrv集成测试报告
生成时间: $(date)
========================================

测试环境:
- Redis: localhost:6379
- Modbus服务器: localhost:5502, localhost:5503
- Comsrv API: localhost:3001

测试结果:
- 总测试数: ${TOTAL_TESTS}
- 通过: ${PASSED_TESTS}
- 失败: ${FAILED_TESTS}
- 成功率: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%

详细日志: comsrv-test.log
EOF
    
    log_info "测试报告已保存到: ${report_file}"
}

# 运行主函数
main