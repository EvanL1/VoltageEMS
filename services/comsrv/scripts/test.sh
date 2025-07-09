#!/bin/bash
# =============================================================================
# Comsrv测试脚本
# 功能：运行comsrv的单元测试和集成测试
# =============================================================================

set -e

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
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

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
SERVICE_DIR="$(dirname "${SCRIPT_DIR}")"

# 测试类型
TEST_TYPE=${1:-"all"}  # all, unit, integration

# 切换到服务目录
cd "${SERVICE_DIR}"

log_info "=========================================="
log_info "Comsrv 测试"
log_info "测试类型: ${TEST_TYPE}"
log_info "=========================================="

# 环境准备
export RUST_BACKTRACE=1
export RUST_LOG=debug

# 运行单元测试
run_unit_tests() {
    log_test "运行单元测试..."
    
    # 运行所有单元测试
    if cargo test --lib --bins -- --nocapture; then
        log_info "单元测试通过"
        return 0
    else
        log_error "单元测试失败"
        return 1
    fi
}

# 运行集成测试
run_integration_tests() {
    log_test "运行集成测试..."
    
    # 检查是否需要启动测试服务
    if [ -f "scripts/start-test-servers.sh" ]; then
        log_info "启动测试服务器..."
        ./scripts/start-test-servers.sh
        
        # 等待服务启动
        sleep 5
    fi
    
    # 设置测试环境变量
    export TEST_REDIS_URL="redis://localhost:6379"
    export TEST_MODBUS_SERVER="localhost:5502"
    
    # 运行集成测试
    local test_result=0
    
    if cargo test --test '*' -- --nocapture; then
        log_info "集成测试通过"
    else
        log_error "集成测试失败"
        test_result=1
    fi
    
    # 停止测试服务
    if [ -f "scripts/stop-test-servers.sh" ]; then
        log_info "停止测试服务器..."
        ./scripts/stop-test-servers.sh
    fi
    
    return $test_result
}

# 运行特定的测试模块
run_module_tests() {
    local module=$1
    log_test "运行模块测试: ${module}"
    
    if cargo test --lib "${module}" -- --nocapture; then
        log_info "模块测试 ${module} 通过"
        return 0
    else
        log_error "模块测试 ${module} 失败"
        return 1
    fi
}

# 运行协议测试
run_protocol_tests() {
    log_test "运行协议测试..."
    
    # Modbus测试
    if [ -f "tests/test_modbus_client.py" ]; then
        log_info "运行Modbus协议测试..."
        python3 tests/test_modbus_client.py || true
    fi
    
    # 其他协议测试可以在这里添加
}

# 生成测试覆盖率报告
generate_coverage() {
    log_info "生成测试覆盖率报告..."
    
    # 检查是否安装了tarpaulin
    if ! command -v cargo-tarpaulin &> /dev/null; then
        log_warning "cargo-tarpaulin未安装，跳过覆盖率报告"
        log_info "安装命令: cargo install cargo-tarpaulin"
        return
    fi
    
    # 生成覆盖率报告
    cargo tarpaulin --out Html --output-dir coverage
    log_info "覆盖率报告已生成: coverage/tarpaulin-report.html"
}

# 主测试流程
main() {
    local exit_code=0
    
    case "${TEST_TYPE}" in
        "unit")
            run_unit_tests || exit_code=$?
            ;;
        "integration")
            run_integration_tests || exit_code=$?
            ;;
        "protocol")
            run_protocol_tests || exit_code=$?
            ;;
        "coverage")
            generate_coverage
            ;;
        "all")
            log_info "运行所有测试..."
            
            # 单元测试
            if ! run_unit_tests; then
                exit_code=1
            fi
            
            echo ""
            
            # 集成测试
            if ! run_integration_tests; then
                exit_code=1
            fi
            
            echo ""
            
            # 协议测试
            run_protocol_tests
            ;;
        *)
            # 运行特定模块测试
            run_module_tests "${TEST_TYPE}" || exit_code=$?
            ;;
    esac
    
    # 显示测试结果
    echo ""
    log_info "=========================================="
    if [ $exit_code -eq 0 ]; then
        log_info "测试完成: 全部通过"
    else
        log_error "测试完成: 有失败的测试"
    fi
    log_info "=========================================="
    
    exit $exit_code
}

# 运行主函数
main