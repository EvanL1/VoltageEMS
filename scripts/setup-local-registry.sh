#!/bin/bash
# =============================================================================
# 本地Docker Registry设置脚本
# 功能：配置和启动本地Docker镜像仓库
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

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# 配置参数
REGISTRY_PORT=${REGISTRY_PORT:-5000}
REGISTRY_NAME="voltageems-registry"
REGISTRY_DATA_DIR="/var/lib/registry"
REGISTRY_CONFIG_DIR="/etc/docker/registry"

log_info "=========================================="
log_info "VoltageEMS 本地Docker Registry设置"
log_info "=========================================="

# 检查Docker是否运行
if ! docker info > /dev/null 2>&1; then
    log_error "Docker未运行，请先启动Docker"
    exit 1
fi

# 检查是否已有运行的Registry
if docker ps -a | grep -q "${REGISTRY_NAME}"; then
    log_warning "发现已存在的Registry容器"
    read -p "是否删除并重新创建? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_step "停止并删除现有Registry..."
        docker stop ${REGISTRY_NAME} 2>/dev/null || true
        docker rm ${REGISTRY_NAME} 2>/dev/null || true
    else
        log_info "保留现有Registry"
        
        # 检查是否在运行
        if ! docker ps | grep -q "${REGISTRY_NAME}"; then
            log_step "启动现有Registry..."
            docker start ${REGISTRY_NAME}
        fi
        
        log_info "Registry已在运行"
        exit 0
    fi
fi

# 创建Registry配置
log_step "创建Registry配置..."
TEMP_CONFIG=$(mktemp)
cat > "${TEMP_CONFIG}" << EOF
version: 0.1
log:
  fields:
    service: registry
storage:
  cache:
    blobdescriptor: inmemory
  filesystem:
    rootdirectory: /var/lib/registry
  delete:
    enabled: true
http:
  addr: :5000
  headers:
    X-Content-Type-Options: [nosniff]
health:
  storagedriver:
    enabled: true
    interval: 10s
    threshold: 3
EOF

# 创建数据目录（如果需要持久化到主机）
if [ ! -d "${HOME}/.voltageems/registry" ]; then
    log_step "创建Registry数据目录..."
    mkdir -p "${HOME}/.voltageems/registry/data"
    mkdir -p "${HOME}/.voltageems/registry/config"
    cp "${TEMP_CONFIG}" "${HOME}/.voltageems/registry/config/config.yml"
fi

# 启动Registry容器
log_step "启动Docker Registry..."
docker run -d \
    --name ${REGISTRY_NAME} \
    --restart=always \
    -p ${REGISTRY_PORT}:5000 \
    -v "${HOME}/.voltageems/registry/data:/var/lib/registry" \
    -v "${HOME}/.voltageems/registry/config/config.yml:/etc/docker/registry/config.yml" \
    registry:2

# 等待Registry启动
log_info "等待Registry启动..."
sleep 3

# 检查Registry状态
if ! docker ps | grep -q "${REGISTRY_NAME}"; then
    log_error "Registry启动失败"
    docker logs ${REGISTRY_NAME}
    exit 1
fi

# 测试Registry
log_step "测试Registry连接..."
if curl -s -f "http://localhost:${REGISTRY_PORT}/v2/" > /dev/null 2>&1; then
    log_info "Registry连接成功"
else
    log_error "无法连接到Registry"
    exit 1
fi

# 配置Docker daemon（如果需要）
log_step "检查Docker daemon配置..."
DOCKER_CONFIG_FILE="/etc/docker/daemon.json"
NEED_RESTART=false

# 检查是否需要添加insecure-registries
if [ -f "$DOCKER_CONFIG_FILE" ]; then
    if ! grep -q "localhost:${REGISTRY_PORT}" "$DOCKER_CONFIG_FILE"; then
        log_warning "需要更新Docker daemon配置以信任本地Registry"
        NEED_RESTART=true
    fi
else
    log_warning "Docker daemon配置文件不存在，需要创建"
    NEED_RESTART=true
fi

if [ "$NEED_RESTART" = true ]; then
    log_info "请手动添加以下配置到 ${DOCKER_CONFIG_FILE}:"
    echo ""
    cat << EOF
{
  "insecure-registries": ["localhost:${REGISTRY_PORT}"]
}
EOF
    echo ""
    log_info "然后重启Docker服务:"
    echo "  sudo systemctl restart docker  # Linux"
    echo "  或在Docker Desktop中重启      # macOS/Windows"
fi

# 显示Registry信息
log_info "=========================================="
log_info "Registry设置完成!"
log_info "地址: localhost:${REGISTRY_PORT}"
log_info "数据目录: ${HOME}/.voltageems/registry/data"
log_info "=========================================="

# 显示使用示例
log_info "使用示例:"
echo "  # 标记镜像"
echo "  docker tag myimage:latest localhost:${REGISTRY_PORT}/myimage:latest"
echo ""
echo "  # 推送镜像"
echo "  docker push localhost:${REGISTRY_PORT}/myimage:latest"
echo ""
echo "  # 拉取镜像"
echo "  docker pull localhost:${REGISTRY_PORT}/myimage:latest"
echo ""
echo "  # 查看Registry中的镜像"
echo "  curl http://localhost:${REGISTRY_PORT}/v2/_catalog"

# 清理临时文件
rm -f "${TEMP_CONFIG}"

exit 0