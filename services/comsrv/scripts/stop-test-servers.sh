#!/bin/bash
# =============================================================================
# 停止测试服务器脚本
# 功能：停止comsrv测试所需的各种模拟服务器
# =============================================================================

set +e  # 允许命令失败，继续执行

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

log_info "=========================================="
log_info "停止Comsrv测试服务器"
log_info "=========================================="

# 停止函数
stop_service() {
    local pid_file=$1
    local service_name=$2
    
    if [ ! -f "$pid_file" ]; then
        return
    fi
    
    local pid_or_container=$(cat "$pid_file")
    
    # 检查是否是Docker容器
    if [[ $pid_or_container == docker:* ]]; then
        local container_name=$(echo $pid_or_container | cut -d: -f2)
        log_info "停止Docker容器: ${container_name}"
        
        docker stop "${container_name}" 2>/dev/null
        docker rm "${container_name}" 2>/dev/null
    else
        # 普通进程
        if ps -p "$pid_or_container" > /dev/null 2>&1; then
            log_info "停止${service_name} (PID: ${pid_or_container})"
            kill "$pid_or_container" 2>/dev/null
            
            # 等待进程结束
            local count=0
            while ps -p "$pid_or_container" > /dev/null 2>&1 && [ $count -lt 5 ]; do
                sleep 1
                ((count++))
            done
            
            # 如果还在运行，强制终止
            if ps -p "$pid_or_container" > /dev/null 2>&1; then
                log_warning "强制终止${service_name}"
                kill -9 "$pid_or_container" 2>/dev/null
            fi
        fi
    fi
    
    # 删除PID文件
    rm -f "$pid_file"
}

# 停止所有服务
if [ -d "$PID_DIR" ]; then
    # 停止Redis
    if [ -f "${PID_DIR}/redis.pid" ]; then
        stop_service "${PID_DIR}/redis.pid" "Redis"
    fi
    
    # 停止所有Modbus服务器
    for pid_file in "${PID_DIR}"/modbus_*.pid; do
        if [ -f "$pid_file" ]; then
            port=$(basename "$pid_file" | sed 's/modbus_\(.*\)\.pid/\1/')
            stop_service "$pid_file" "Modbus服务器(端口: $port)"
        fi
    done
    
    # 停止MQTT
    if [ -f "${PID_DIR}/mqtt.pid" ]; then
        stop_service "${PID_DIR}/mqtt.pid" "MQTT Broker"
    fi
    
    # 清理日志文件
    rm -f "${PID_DIR}"/*.log
fi

# 额外的清理：查找并停止可能遗留的进程
cleanup_orphaned_processes() {
    log_info "清理可能的遗留进程..."
    
    # 查找Python测试服务器
    for pid in $(ps aux | grep -E "modbus_server_simulator|modbus_multi_server" | grep -v grep | awk '{print $2}'); do
        log_warning "发现遗留的Modbus服务器进程: $pid"
        kill "$pid" 2>/dev/null
    done
    
    # 清理测试用的Docker容器
    for container in $(docker ps -a --filter "name=comsrv-test-" --format "{{.Names}}"); do
        log_warning "清理Docker容器: $container"
        docker stop "$container" 2>/dev/null
        docker rm "$container" 2>/dev/null
    done
}

# 执行额外清理
cleanup_orphaned_processes

# 清理PID目录
if [ -d "$PID_DIR" ] && [ -z "$(ls -A "$PID_DIR" 2>/dev/null)" ]; then
    rmdir "$PID_DIR" 2>/dev/null
fi

log_info "=========================================="
log_info "测试服务器已停止"
log_info "=========================================="

exit 0