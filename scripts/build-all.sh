#!/bin/bash
# =============================================================================
# VoltageEMS批量构建脚本
# 功能：构建所有微服务的Docker镜像
# =============================================================================

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

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

# 获取脚本参数
VERSION=${1:-"latest"}
REGISTRY=${2:-"localhost:5000"}
PROJECT="voltageems"

# 服务列表
SERVICES=(
    "comsrv"
    "modsrv"
    "hissrv"
    "netsrv"
    "alarmsrv"
    "apigateway"
    "frontend"
)

# 构建开始时间
START_TIME=$(date +%s)

log_info "=========================================="
log_info "VoltageEMS Docker镜像批量构建"
log_info "版本: ${VERSION}"
log_info "Registry: ${REGISTRY}"
log_info "=========================================="

# 检查Docker是否运行
if ! docker info > /dev/null 2>&1; then
    log_error "Docker未运行，请先启动Docker"
    exit 1
fi

# 构建结果统计
SUCCESS_COUNT=0
FAILED_COUNT=0
FAILED_SERVICES=()

# 构建函数
build_service() {
    local service=$1
    local service_path=""
    
    # 确定服务路径
    if [ "$service" == "frontend" ]; then
        service_path="frontend"
    else
        service_path="services/${service}"
    fi
    
    log_info "开始构建 ${service}..."
    
    # 检查服务目录是否存在
    if [ ! -d "${service_path}" ]; then
        log_error "服务目录不存在: ${service_path}"
        return 1
    fi
    
    # 检查Dockerfile是否存在
    if [ ! -f "${service_path}/Dockerfile" ]; then
        log_warning "${service} 没有Dockerfile，尝试生成通用Dockerfile..."
        # 这里可以添加生成通用Dockerfile的逻辑
        return 1
    fi
    
    # 构建镜像
    if docker build \
        -t "${REGISTRY}/${PROJECT}/${service}:${VERSION}" \
        -t "${REGISTRY}/${PROJECT}/${service}:latest" \
        --build-arg VERSION="${VERSION}" \
        "${service_path}"; then
        
        log_info "${service} 构建成功"
        return 0
    else
        log_error "${service} 构建失败"
        return 1
    fi
}

# 遍历所有服务进行构建
for service in "${SERVICES[@]}"; do
    echo ""
    if build_service "$service"; then
        ((SUCCESS_COUNT++))
    else
        ((FAILED_COUNT++))
        FAILED_SERVICES+=("$service")
    fi
done

# 计算构建时间
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# 显示构建结果
echo ""
log_info "=========================================="
log_info "构建完成"
log_info "成功: ${SUCCESS_COUNT}个服务"
if [ ${FAILED_COUNT} -gt 0 ]; then
    log_error "失败: ${FAILED_COUNT}个服务"
    log_error "失败的服务: ${FAILED_SERVICES[@]}"
fi
log_info "总耗时: ${DURATION}秒"
log_info "=========================================="

# 列出所有构建的镜像
echo ""
log_info "已构建的镜像:"
docker images | grep "${REGISTRY}/${PROJECT}" | grep -E "(${VERSION}|latest)"

# 如果有失败，返回非零退出码
if [ ${FAILED_COUNT} -gt 0 ]; then
    exit 1
fi

exit 0