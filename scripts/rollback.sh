#!/bin/bash
# =============================================================================
# VoltageEMS回滚脚本
# 功能：快速回滚到之前的版本
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

# 部署目录
DEPLOY_DIR="/opt/voltageems"
BACKUP_DIR="${DEPLOY_DIR}/backups"

log_info "=========================================="
log_info "VoltageEMS 回滚脚本"
log_info "=========================================="

# 检查权限
if [ "$EUID" -ne 0 ]; then 
    log_error "请使用sudo运行此脚本"
    exit 1
fi

# 检查备份目录
if [ ! -d "$BACKUP_DIR" ] || [ -z "$(ls -A $BACKUP_DIR 2>/dev/null)" ]; then
    log_error "没有找到可用的备份"
    exit 1
fi

# 列出可用的备份
log_step "可用的备份版本:"
echo ""
cd "$BACKUP_DIR"
BACKUPS=($(ls -t))
INDEX=1

for backup in "${BACKUPS[@]}"; do
    # 获取备份信息
    BACKUP_TIME=$(echo $backup | sed 's/backup_//')
    FORMATTED_TIME=$(echo $BACKUP_TIME | sed 's/\([0-9]\{8\}\)_\([0-9]\{6\}\)/\1 \2/')
    
    # 尝试读取部署信息
    VERSION="未知"
    if [ -f "${backup}/deployment_info.txt" ]; then
        VERSION=$(grep "版本:" "${backup}/deployment_info.txt" | cut -d' ' -f2) || VERSION="未知"
    fi
    
    echo "  ${INDEX}. ${FORMATTED_TIME} - 版本: ${VERSION}"
    ((INDEX++))
done

echo ""
read -p "请选择要回滚的版本 (输入序号，0取消): " CHOICE

# 验证输入
if [ "$CHOICE" -eq 0 ] 2>/dev/null; then
    log_info "取消回滚"
    exit 0
fi

if [ "$CHOICE" -lt 1 ] || [ "$CHOICE" -gt "${#BACKUPS[@]}" ] 2>/dev/null; then
    log_error "无效的选择"
    exit 1
fi

# 选择的备份
SELECTED_BACKUP="${BACKUPS[$((CHOICE-1))]}"
BACKUP_PATH="${BACKUP_DIR}/${SELECTED_BACKUP}"

log_info "选择的备份: ${SELECTED_BACKUP}"

# 确认回滚
echo ""
log_warning "警告: 回滚将停止当前运行的服务并恢复到选定版本"
read -p "确认要回滚吗? (输入 'yes' 确认): " CONFIRM

if [ "$CONFIRM" != "yes" ]; then
    log_info "取消回滚"
    exit 0
fi

# 开始回滚
log_step "开始回滚流程..."

# 记录当前状态（用于回滚的回滚）
log_step "记录当前状态..."
ROLLBACK_BACKUP="rollback_backup_$(date +%Y%m%d_%H%M%S)"
mkdir -p "${BACKUP_DIR}/${ROLLBACK_BACKUP}"
cp "${DEPLOY_DIR}/docker-compose.yml" "${BACKUP_DIR}/${ROLLBACK_BACKUP}/" 2>/dev/null || true
cp -r "${DEPLOY_DIR}/config" "${BACKUP_DIR}/${ROLLBACK_BACKUP}/" 2>/dev/null || true
docker-compose -f "${DEPLOY_DIR}/docker-compose.yml" ps > "${BACKUP_DIR}/${ROLLBACK_BACKUP}/running_containers.txt" 2>/dev/null || true

# 停止当前服务
log_step "停止当前服务..."
cd "$DEPLOY_DIR"
docker-compose down || true

# 恢复备份文件
log_step "恢复配置文件..."
cp "${BACKUP_PATH}/docker-compose.yml" "${DEPLOY_DIR}/" || {
    log_error "无法恢复docker-compose.yml"
    exit 1
}

if [ -d "${BACKUP_PATH}/config" ]; then
    rm -rf "${DEPLOY_DIR}/config"
    cp -r "${BACKUP_PATH}/config" "${DEPLOY_DIR}/"
fi

# 启动服务
log_step "启动服务..."
cd "$DEPLOY_DIR"
docker-compose up -d

# 等待服务启动
log_info "等待服务启动..."
sleep 10

# 健康检查
log_step "执行健康检查..."
HEALTH_CHECK_PASSED=true

# 简单的健康检查
for service in comsrv modsrv hissrv netsrv alarmsrv apigateway frontend; do
    if docker-compose ps | grep -E "^${service}" | grep -q "Up"; then
        log_info "${service} 运行正常"
    else
        log_error "${service} 未正常运行"
        HEALTH_CHECK_PASSED=false
    fi
done

# 显示服务状态
log_step "服务状态:"
docker-compose ps

# 记录回滚信息
cat > "${DEPLOY_DIR}/rollback_info.txt" << EOF
回滚时间: $(date)
回滚到版本: ${SELECTED_BACKUP}
执行人: $(whoami)
主机: $(hostname)
EOF

if [ "$HEALTH_CHECK_PASSED" = true ]; then
    log_info "=========================================="
    log_info "回滚成功!"
    log_info "当前版本: ${SELECTED_BACKUP}"
    log_info "=========================================="
else
    log_error "=========================================="
    log_error "回滚完成，但部分服务可能存在问题"
    log_error "请检查服务状态和日志"
    log_error "=========================================="
fi

# 显示日志查看命令
echo ""
log_info "查看日志命令:"
echo "  所有服务: docker-compose logs -f"
echo "  特定服务: docker-compose logs -f [服务名]"
echo ""
log_info "如果需要再次回滚，可以选择 '${ROLLBACK_BACKUP}'"

exit 0