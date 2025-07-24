#!/bin/bash
set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查配置文件
if [ ! -f "${MODSRV_CONFIG_FILE}" ]; then
    log_error "Configuration file not found: ${MODSRV_CONFIG_FILE}"
    exit 1
fi

log_info "Using configuration: ${MODSRV_CONFIG_FILE}"

# 等待Redis可用
REDIS_HOST=$(echo ${MODSRV_REDIS_URL} | sed -n 's|.*//\([^:]*\).*|\1|p')
REDIS_PORT=$(echo ${MODSRV_REDIS_URL} | sed -n 's|.*:\([0-9]*\).*|\1|p')

if [ -z "$REDIS_HOST" ]; then
    REDIS_HOST="redis"
fi

if [ -z "$REDIS_PORT" ]; then
    REDIS_PORT="6379"
fi

log_info "Waiting for Redis at ${REDIS_HOST}:${REDIS_PORT}..."

max_attempts=30
attempt=0

while ! nc -z ${REDIS_HOST} ${REDIS_PORT}; do
    attempt=$((attempt + 1))
    if [ $attempt -ge $max_attempts ]; then
        log_error "Redis is not available after ${max_attempts} attempts"
        exit 1
    fi
    log_warn "Redis is not ready yet, waiting... (attempt ${attempt}/${max_attempts})"
    sleep 2
done

log_info "Redis is ready!"

# 创建必要的目录
if [ ! -d "/logs" ]; then
    log_warn "/logs directory not found, creating..."
    mkdir -p /logs
fi

# 打印环境信息
log_info "Environment:"
log_info "  RUST_LOG: ${RUST_LOG}"
log_info "  Redis URL: ${MODSRV_REDIS_URL}"
log_info "  Config File: ${MODSRV_CONFIG_FILE}"
log_info "  Log Directory: ${MODSRV_LOG_DIR:-/logs}"

# 处理信号
trap_handler() {
    log_info "Received shutdown signal, stopping modsrv..."
    kill -TERM "$child" 2>/dev/null
    wait "$child"
    exit 0
}

trap trap_handler SIGTERM SIGINT

# 启动modsrv
log_info "Starting modsrv..."

if [ "$1" = "modsrv" ] && [ "$2" = "service" ]; then
    exec /usr/local/bin/modsrv --config "${MODSRV_CONFIG_FILE}" service &
    child=$!
    wait "$child"
elif [ "$1" = "modsrv" ]; then
    exec /usr/local/bin/modsrv --config "${MODSRV_CONFIG_FILE}"
else
    exec "$@"
fi