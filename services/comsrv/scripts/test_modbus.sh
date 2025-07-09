#!/bin/bash
# Modbus集成测试脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "======================================"
echo "Modbus TCP 集成测试"
echo "======================================"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 函数：打印成功消息
success() {
    echo -e "${GREEN}✓ $1${NC}"
}

# 函数：打印错误消息
error() {
    echo -e "${RED}✗ $1${NC}"
}

# 函数：打印信息消息
info() {
    echo -e "${YELLOW}➜ $1${NC}"
}

# 检查依赖
check_dependencies() {
    info "检查依赖..."
    
    # 检查 uv
    if ! command -v uv &> /dev/null; then
        error "未找到 uv，请先安装 uv"
        exit 1
    fi
    success "找到 uv"
    
    # 检查 cargo
    if ! command -v cargo &> /dev/null; then
        error "未找到 cargo，请先安装 Rust"
        exit 1
    fi
    success "找到 cargo"
    
    # 检查 Redis
    if ! command -v redis-cli &> /dev/null; then
        error "未找到 redis-cli，请先安装 Redis"
        exit 1
    fi
    
    # 检查 Redis 是否运行
    if ! redis-cli ping &> /dev/null; then
        error "Redis 未运行，请先启动 Redis"
        exit 1
    fi
    success "Redis 正在运行"
}

# 安装 Python 依赖
install_python_deps() {
    info "安装 Python 依赖..."
    cd "$PROJECT_ROOT/tests"
    
    # 使用 uv 安装 pymodbus
    uv pip install pymodbus[serial] --quiet
    success "Python 依赖安装完成"
}

# 启动 Modbus 服务器模拟器
start_modbus_server() {
    info "启动 Modbus 服务器模拟器..."
    
    cd "$PROJECT_ROOT/tests"
    uv run python modbus_server_simulator.py --port 5502 &
    SERVER_PID=$!
    
    # 等待服务器启动
    sleep 2
    
    # 检查服务器是否在运行
    if ! ps -p $SERVER_PID > /dev/null; then
        error "Modbus 服务器启动失败"
        exit 1
    fi
    
    success "Modbus 服务器已启动 (PID: $SERVER_PID)"
    echo $SERVER_PID > /tmp/modbus_server.pid
}

# 构建 comsrv
build_comsrv() {
    info "构建 comsrv..."
    cd "$PROJECT_ROOT"
    
    cargo build --release
    success "comsrv 构建完成"
}

# 启动 comsrv
start_comsrv() {
    info "启动 comsrv..."
    cd "$PROJECT_ROOT"
    
    # 设置环境变量
    export RUST_LOG=debug
    export COMSRV_CSV_BASE_PATH="$PROJECT_ROOT/config"
    
    # 启动 comsrv
    cargo run --release -- --config config/modbus_test.yaml &
    COMSRV_PID=$!
    
    # 等待 comsrv 启动
    sleep 3
    
    # 检查 comsrv 是否在运行
    if ! ps -p $COMSRV_PID > /dev/null; then
        error "comsrv 启动失败"
        cat logs/comsrv-modbus.log
        cleanup
        exit 1
    fi
    
    success "comsrv 已启动 (PID: $COMSRV_PID)"
    echo $COMSRV_PID > /tmp/comsrv.pid
}

# 运行测试客户端
run_test_client() {
    info "运行 Modbus 测试客户端..."
    cd "$PROJECT_ROOT/tests"
    
    # 运行测试
    uv run python test_modbus_client.py --port 5502
    TEST_RESULT=$?
    
    if [ $TEST_RESULT -eq 0 ]; then
        success "Modbus 功能测试通过"
    else
        error "Modbus 功能测试失败"
        return 1
    fi
}

# 检查 Redis 数据
check_redis_data() {
    info "检查 Redis 数据..."
    
    # 检查遥测数据
    TELEMETRY_COUNT=$(redis-cli --scan --pattern "voltage:telemetry:*" | wc -l)
    if [ $TELEMETRY_COUNT -gt 0 ]; then
        success "找到 $TELEMETRY_COUNT 个遥测点"
        
        # 显示示例数据
        echo "示例遥测数据："
        redis-cli get "voltage:telemetry:1:value" | head -5
    else
        error "未找到遥测数据"
    fi
    
    # 检查遥信数据
    SIGNAL_COUNT=$(redis-cli --scan --pattern "voltage:signal:*" | wc -l)
    if [ $SIGNAL_COUNT -gt 0 ]; then
        success "找到 $SIGNAL_COUNT 个遥信点"
    fi
}

# 清理函数
cleanup() {
    info "清理测试环境..."
    
    # 停止 comsrv
    if [ -f /tmp/comsrv.pid ]; then
        COMSRV_PID=$(cat /tmp/comsrv.pid)
        if ps -p $COMSRV_PID > /dev/null 2>&1; then
            kill $COMSRV_PID
            success "停止 comsrv"
        fi
        rm /tmp/comsrv.pid
    fi
    
    # 停止 Modbus 服务器
    if [ -f /tmp/modbus_server.pid ]; then
        SERVER_PID=$(cat /tmp/modbus_server.pid)
        if ps -p $SERVER_PID > /dev/null 2>&1; then
            kill $SERVER_PID
            success "停止 Modbus 服务器"
        fi
        rm /tmp/modbus_server.pid
    fi
}

# 主函数
main() {
    # 设置清理钩子
    trap cleanup EXIT
    
    echo "开始时间: $(date)"
    echo ""
    
    # 执行测试步骤
    check_dependencies
    install_python_deps
    start_modbus_server
    build_comsrv
    start_comsrv
    
    # 等待数据收集
    info "等待数据收集..."
    sleep 5
    
    # 运行测试
    run_test_client
    check_redis_data
    
    echo ""
    echo "======================================"
    echo "测试完成"
    echo "结束时间: $(date)"
    echo "======================================"
}

# 执行主函数
main