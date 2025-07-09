#!/bin/bash
# =============================================================================
# 启动测试服务器脚本
# 功能：启动comsrv测试所需的各种模拟服务器
# =============================================================================

set -e

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
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

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
SERVICE_DIR="$(dirname "${SCRIPT_DIR}")"

# PID文件目录
PID_DIR="${SERVICE_DIR}/.test-pids"
mkdir -p "${PID_DIR}"

log_info "=========================================="
log_info "启动Comsrv测试服务器"
log_info "=========================================="

# 启动Redis测试实例
start_redis() {
    log_info "启动Redis测试实例..."
    
    # 检查是否已在运行
    if nc -z localhost 6379 2>/dev/null; then
        log_warning "Redis已在运行 (端口 6379)"
        return
    fi
    
    # 使用Docker启动Redis
    if command -v docker &> /dev/null; then
        docker run -d \
            --name comsrv-test-redis \
            -p 6379:6379 \
            redis:7-alpine \
            redis-server --appendonly no
        
        echo "docker:comsrv-test-redis" > "${PID_DIR}/redis.pid"
        log_info "Redis启动成功 (Docker)"
    else
        log_error "Docker未安装，无法启动Redis"
        return 1
    fi
}

# 启动Modbus服务器
start_modbus_server() {
    log_info "启动Modbus测试服务器..."
    
    # 检查Python环境
    if ! command -v python3 &> /dev/null; then
        log_error "Python3未安装"
        return 1
    fi
    
    # 检查是否有Modbus服务器脚本
    if [ -f "${SERVICE_DIR}/tests/modbus_server_simulator.py" ]; then
        # 启动多个Modbus服务器模拟不同设备
        python3 "${SERVICE_DIR}/tests/modbus_server_simulator.py" \
            --port 5502 \
            --slave-id 1 \
            --device-type "temperature" \
            > "${PID_DIR}/modbus_5502.log" 2>&1 &
        
        echo $! > "${PID_DIR}/modbus_5502.pid"
        log_info "Modbus服务器启动 (端口: 5502, 从站ID: 1)"
        
        # 启动第二个Modbus服务器
        python3 "${SERVICE_DIR}/tests/modbus_server_simulator.py" \
            --port 5503 \
            --slave-id 2 \
            --device-type "power" \
            > "${PID_DIR}/modbus_5503.log" 2>&1 &
        
        echo $! > "${PID_DIR}/modbus_5503.pid"
        log_info "Modbus服务器启动 (端口: 5503, 从站ID: 2)"
        
    elif [ -f "${SERVICE_DIR}/scripts/modbus_multi_server.py" ]; then
        # 使用多服务器脚本
        python3 "${SERVICE_DIR}/scripts/modbus_multi_server.py" \
            > "${PID_DIR}/modbus_multi.log" 2>&1 &
        
        echo $! > "${PID_DIR}/modbus_multi.pid"
        log_info "Modbus多服务器启动"
    else
        log_warning "未找到Modbus服务器脚本"
    fi
}

# 启动MQTT测试服务器
start_mqtt_broker() {
    log_info "启动MQTT测试服务器..."
    
    # 检查是否需要MQTT
    if [ "${ENABLE_MQTT_TEST:-false}" != "true" ]; then
        log_info "跳过MQTT服务器 (未启用)"
        return
    fi
    
    # 使用Docker启动Mosquitto
    if command -v docker &> /dev/null; then
        docker run -d \
            --name comsrv-test-mqtt \
            -p 1883:1883 \
            eclipse-mosquitto:latest
        
        echo "docker:comsrv-test-mqtt" > "${PID_DIR}/mqtt.pid"
        log_info "MQTT Broker启动成功 (端口: 1883)"
    fi
}

# 等待服务就绪
wait_for_services() {
    log_info "等待服务就绪..."
    
    # 等待Redis
    local attempts=0
    while ! nc -z localhost 6379 2>/dev/null; do
        if [ $attempts -gt 10 ]; then
            log_error "Redis启动超时"
            return 1
        fi
        sleep 1
        ((attempts++))
    done
    log_info "Redis就绪"
    
    # 等待Modbus服务器
    sleep 2
    if [ -f "${PID_DIR}/modbus_5502.pid" ]; then
        if ps -p $(cat "${PID_DIR}/modbus_5502.pid") > /dev/null 2>&1; then
            log_info "Modbus服务器就绪"
        else
            log_error "Modbus服务器启动失败"
            cat "${PID_DIR}/modbus_5502.log" 2>/dev/null || true
        fi
    fi
}

# 显示服务状态
show_status() {
    log_info "测试服务状态:"
    
    # Redis状态
    if [ -f "${PID_DIR}/redis.pid" ]; then
        if [[ $(cat "${PID_DIR}/redis.pid") == docker:* ]]; then
            container_name=$(cat "${PID_DIR}/redis.pid" | cut -d: -f2)
            if docker ps | grep -q "${container_name}"; then
                echo "  Redis: 运行中 (Docker)"
            else
                echo "  Redis: 已停止"
            fi
        fi
    fi
    
    # Modbus状态
    for pid_file in "${PID_DIR}"/modbus_*.pid; do
        if [ -f "$pid_file" ]; then
            port=$(basename "$pid_file" | sed 's/modbus_\(.*\)\.pid/\1/')
            pid=$(cat "$pid_file")
            if ps -p "$pid" > /dev/null 2>&1; then
                echo "  Modbus (端口 $port): 运行中 (PID: $pid)"
            else
                echo "  Modbus (端口 $port): 已停止"
            fi
        fi
    done
}

# 主函数
main() {
    # 清理可能存在的旧进程
    if [ -f "${SCRIPT_DIR}/stop-test-servers.sh" ]; then
        "${SCRIPT_DIR}/stop-test-servers.sh" 2>/dev/null || true
    fi
    
    # 启动各个服务
    start_redis
    start_modbus_server
    start_mqtt_broker
    
    # 等待服务就绪
    wait_for_services
    
    # 显示状态
    echo ""
    show_status
    
    log_info "=========================================="
    log_info "测试服务器启动完成"
    log_info "PID文件位置: ${PID_DIR}"
    log_info "=========================================="
}

# 运行主函数
main