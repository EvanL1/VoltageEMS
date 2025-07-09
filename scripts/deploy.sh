#!/bin/bash
# =============================================================================
# VoltageEMS智能部署脚本
# 功能：部署服务到目标环境，支持备份和健康检查
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
    echo -e "${GREEN}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# 参数检查
if [ $# -lt 2 ]; then
    echo "使用方法: $0 <环境> <版本号>"
    echo "示例: $0 production 1.2.3"
    echo "环境: production, staging, development"
    exit 1
fi

ENVIRONMENT=$1
VERSION=$2
REGISTRY=${REGISTRY:-"localhost:5000"}
PROJECT="voltageems"

# 部署目录
DEPLOY_DIR="/opt/voltageems"
BACKUP_DIR="${DEPLOY_DIR}/backups"
LOG_DIR="${DEPLOY_DIR}/logs"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

log_info "=========================================="
log_info "VoltageEMS 部署脚本"
log_info "环境: ${ENVIRONMENT}"
log_info "版本: ${VERSION}"
log_info "Registry: ${REGISTRY}"
log_info "=========================================="

# 检查权限
if [ "$EUID" -ne 0 ]; then 
    log_error "请使用sudo运行此脚本"
    exit 1
fi

# 创建必要的目录
log_step "创建部署目录..."
mkdir -p "${DEPLOY_DIR}"
mkdir -p "${BACKUP_DIR}"
mkdir -p "${LOG_DIR}"
mkdir -p "${DEPLOY_DIR}/config"

# 备份当前部署（如果存在）
if [ -f "${DEPLOY_DIR}/docker-compose.yml" ]; then
    log_step "备份当前部署..."
    BACKUP_NAME="backup_${TIMESTAMP}"
    mkdir -p "${BACKUP_DIR}/${BACKUP_NAME}"
    
    # 备份配置文件
    cp "${DEPLOY_DIR}/docker-compose.yml" "${BACKUP_DIR}/${BACKUP_NAME}/" || true
    cp -r "${DEPLOY_DIR}/config" "${BACKUP_DIR}/${BACKUP_NAME}/" || true
    
    # 记录当前运行的容器
    docker-compose -f "${DEPLOY_DIR}/docker-compose.yml" ps > "${BACKUP_DIR}/${BACKUP_NAME}/running_containers.txt" || true
    
    log_info "备份完成: ${BACKUP_DIR}/${BACKUP_NAME}"
    
    # 保留最近10个备份
    log_info "清理旧备份..."
    cd "${BACKUP_DIR}"
    ls -t | tail -n +11 | xargs rm -rf || true
fi

# 复制部署文件
log_step "准备部署文件..."
cp docker-compose.prod.yml "${DEPLOY_DIR}/docker-compose.yml"

# 更新镜像版本
log_step "更新镜像版本..."
sed -i.bak "s|:latest|:${VERSION}|g" "${DEPLOY_DIR}/docker-compose.yml"

# 拉取新镜像
log_step "拉取Docker镜像..."
cd "${DEPLOY_DIR}"
docker-compose pull

# 停止旧服务
log_step "停止当前服务..."
docker-compose down || true

# 启动新服务
log_step "启动新服务..."
docker-compose up -d

# 等待服务启动
log_info "等待服务启动..."
sleep 10

# 健康检查函数
health_check() {
    local service=$1
    local port=$2
    local max_attempts=30
    local attempt=0
    
    log_info "检查 ${service} 健康状态..."
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -f "http://localhost:${port}/health" > /dev/null 2>&1; then
            log_info "${service} 健康检查通过"
            return 0
        fi
        
        attempt=$((attempt + 1))
        if [ $attempt -lt $max_attempts ]; then
            echo -n "."
            sleep 2
        fi
    done
    
    log_error "${service} 健康检查失败"
    return 1
}

# 执行健康检查
log_step "执行健康检查..."
HEALTH_CHECK_FAILED=false

# 检查各个服务
health_check "comsrv" 8081 || HEALTH_CHECK_FAILED=true
health_check "modsrv" 8082 || HEALTH_CHECK_FAILED=true
health_check "hissrv" 8083 || HEALTH_CHECK_FAILED=true
health_check "netsrv" 8084 || HEALTH_CHECK_FAILED=true
health_check "alarmsrv" 8085 || HEALTH_CHECK_FAILED=true
health_check "apigateway" 8080 || HEALTH_CHECK_FAILED=true
health_check "frontend" 80 || HEALTH_CHECK_FAILED=true

# 显示服务状态
log_step "服务状态:"
docker-compose ps

# 如果健康检查失败
if [ "$HEALTH_CHECK_FAILED" = true ]; then
    log_error "部分服务健康检查失败"
    log_warning "查看日志: docker-compose logs [服务名]"
    
    # 询问是否回滚
    read -p "是否回滚到上一个版本? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "执行回滚..."
        ./scripts/rollback.sh
        exit 1
    fi
fi

# 记录部署信息
log_step "记录部署信息..."
cat > "${DEPLOY_DIR}/deployment_info.txt" << EOF
部署时间: $(date)
环境: ${ENVIRONMENT}
版本: ${VERSION}
部署人: $(whoami)
主机: $(hostname)
EOF

# 清理
log_step "清理无用资源..."
docker system prune -f

log_info "=========================================="
log_info "部署成功!"
log_info "版本: ${VERSION}"
log_info "环境: ${ENVIRONMENT}"
log_info "访问地址: http://$(hostname)"
log_info "=========================================="

# 显示日志查看命令
echo ""
log_info "查看日志命令:"
echo "  所有服务: docker-compose logs -f"
echo "  特定服务: docker-compose logs -f [服务名]"
echo "  最近100行: docker-compose logs --tail=100 [服务名]"

exit 0