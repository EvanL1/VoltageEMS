#!/bin/bash
# =============================================================================
# Comsrv构建脚本
# 功能：构建comsrv服务的Docker镜像
# =============================================================================

set -e

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
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

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
SERVICE_DIR="$(dirname "${SCRIPT_DIR}")"
PROJECT_ROOT="$(dirname "$(dirname "${SERVICE_DIR}")")"

# 构建参数
VERSION=${1:-"latest"}
REGISTRY=${2:-"localhost:5000"}
PROJECT="voltageems"
SERVICE="comsrv"

# 切换到服务目录
cd "${SERVICE_DIR}"

log_info "=========================================="
log_info "构建 ${SERVICE} Docker镜像"
log_info "版本: ${VERSION}"
log_info "Registry: ${REGISTRY}"
log_info "=========================================="

# 检查Dockerfile
if [ ! -f "Dockerfile" ]; then
    log_error "Dockerfile不存在"
    exit 1
fi

# 检查必要文件
if [ ! -f "Cargo.toml" ]; then
    log_error "Cargo.toml不存在"
    exit 1
fi

# 创建构建时需要的目录
mkdir -p config

# 如果示例配置文件不存在，创建一个
if [ ! -f "config/comsrv.example.yaml" ]; then
    log_warning "创建示例配置文件..."
    cat > config/comsrv.example.yaml << 'EOF'
# Comsrv示例配置文件
service:
  name: "comsrv"
  http_port: 3000
  metrics_port: 9090
  logging:
    level: "info"
    file: "logs/comsrv.log"

redis:
  url: "redis://localhost:6379"
  
channels: []
EOF
fi

# 构建镜像
log_info "开始构建Docker镜像..."
BUILD_START=$(date +%s)

if docker build \
    -t "${REGISTRY}/${PROJECT}/${SERVICE}:${VERSION}" \
    -t "${REGISTRY}/${PROJECT}/${SERVICE}:latest" \
    --build-arg VERSION="${VERSION}" \
    --build-arg BUILD_DATE="$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
    --build-arg VCS_REF="$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')" \
    . ; then
    
    BUILD_END=$(date +%s)
    BUILD_TIME=$((BUILD_END - BUILD_START))
    
    log_info "构建成功! 耗时: ${BUILD_TIME}秒"
    
    # 显示镜像信息
    log_info "镜像信息:"
    docker images | grep -E "${REGISTRY}/${PROJECT}/${SERVICE}" | grep -E "(${VERSION}|latest)"
    
    # 显示镜像大小
    SIZE=$(docker images --format "{{.Size}}" "${REGISTRY}/${PROJECT}/${SERVICE}:${VERSION}")
    log_info "镜像大小: ${SIZE}"
    
else
    log_error "构建失败"
    exit 1
fi

# 可选：运行构建后的测试
if [ "${RUN_POST_BUILD_TEST:-false}" = "true" ]; then
    log_info "运行构建后测试..."
    docker run --rm "${REGISTRY}/${PROJECT}/${SERVICE}:${VERSION}" /app/bin/comsrv --version
fi

log_info "=========================================="
log_info "构建完成: ${REGISTRY}/${PROJECT}/${SERVICE}:${VERSION}"
log_info "=========================================="

exit 0