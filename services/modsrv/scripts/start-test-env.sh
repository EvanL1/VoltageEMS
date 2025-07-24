#!/bin/bash

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

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

log_section() {
    echo -e "\n${BLUE}=== $1 ===${NC}\n"
}

# 检查Docker和Docker Compose
check_requirements() {
    log_section "检查环境要求"
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker未安装，请先安装Docker"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose未安装，请先安装Docker Compose"
        exit 1
    fi
    
    log_info "Docker版本: $(docker --version)"
    log_info "Docker Compose版本: $(docker-compose --version)"
}

# 创建必要的目录
prepare_directories() {
    log_section "准备目录结构"
    
    cd "$PROJECT_DIR"
    
    # 创建日志目录
    if [ ! -d "logs" ]; then
        mkdir -p logs
        log_info "创建日志目录: logs/"
    fi
    
    # 创建数据目录
    if [ ! -d "data" ]; then
        mkdir -p data
        log_info "创建数据目录: data/"
    fi
    
    # 创建模型目录
    if [ ! -d "test-configs/models" ]; then
        mkdir -p test-configs/models
        log_info "创建模型目录: test-configs/models/"
    fi
}

# 构建镜像
build_images() {
    log_section "构建Docker镜像"
    
    cd "$PROJECT_DIR"
    
    log_info "开始构建modsrv镜像..."
    
    # 使用docker-compose构建
    if docker-compose -f docker-compose.test.yml build --no-cache; then
        log_info "镜像构建成功"
    else
        log_error "镜像构建失败"
        exit 1
    fi
}

# 启动服务
start_services() {
    log_section "启动测试环境"
    
    cd "$PROJECT_DIR"
    
    log_info "启动所有服务..."
    
    # 启动服务
    if docker-compose -f docker-compose.test.yml up -d; then
        log_info "服务启动命令执行成功"
    else
        log_error "服务启动失败"
        exit 1
    fi
    
    # 等待服务就绪
    log_info "等待服务启动..."
    sleep 5
    
    # 检查服务状态
    check_services_health
}

# 检查服务健康状态
check_services_health() {
    log_section "检查服务状态"
    
    cd "$PROJECT_DIR"
    
    # 检查Redis
    log_info "检查Redis服务..."
    if docker-compose -f docker-compose.test.yml exec -T redis redis-cli ping > /dev/null 2>&1; then
        log_info "✓ Redis服务正常"
    else
        log_error "✗ Redis服务异常"
    fi
    
    # 通过test-runner容器检查ModSrv健康状态
    log_info "检查ModSrv服务..."
    max_attempts=30
    attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if docker-compose -f docker-compose.test.yml exec -T test-runner curl -sf http://modsrv:8092/health > /dev/null 2>&1; then
            log_info "✓ ModSrv服务正常"
            break
        else
            attempt=$((attempt + 1))
            if [ $attempt -ge $max_attempts ]; then
                log_error "✗ ModSrv服务启动超时"
                docker-compose -f docker-compose.test.yml logs modsrv
                exit 1
            fi
            log_warn "ModSrv尚未就绪，等待中... ($attempt/$max_attempts)"
            sleep 2
        fi
    done
    
    # 显示所有服务状态
    log_info "所有服务状态:"
    docker-compose -f docker-compose.test.yml ps
}

# 显示访问信息
show_access_info() {
    log_section "访问信息"
    
    echo -e "${GREEN}服务已在内部网络启动！${NC}"
    echo -e "${YELLOW}注意：所有服务仅在Docker内部网络可访问，无端口暴露到宿主机${NC}"
    echo ""
    echo -e "${GREEN}测试命令：${NC}"
    echo -e "  • 运行测试: ${YELLOW}docker-compose -f docker-compose.test.yml exec test-runner /scripts/run-internal-tests.sh${NC}"
    echo -e "  • 进入测试环境: ${YELLOW}docker-compose -f docker-compose.test.yml exec test-runner bash${NC}"
    echo -e "  • 查看日志: ${YELLOW}docker-compose -f docker-compose.test.yml logs -f modsrv${NC}"
    echo -e "  • 停止服务: ${YELLOW}docker-compose -f docker-compose.test.yml down${NC}"
    echo ""
    echo -e "${GREEN}内部调试：${NC}"
    echo -e "  • 检查健康状态: ${YELLOW}docker-compose exec test-runner curl http://modsrv:8092/health${NC}"
    echo -e "  • Redis操作: ${YELLOW}docker-compose exec test-runner redis-cli -h redis${NC}"
    echo -e "  • 监控网络: ${YELLOW}docker-compose --profile debug up -d monitor${NC}"
    echo ""
}

# 主函数
main() {
    log_section "ModSrv Docker测试环境启动脚本"
    
    check_requirements
    prepare_directories
    
    # 询问是否重新构建镜像
    read -p "是否重新构建镜像？(y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        build_images
    fi
    
    start_services
    show_access_info
    
    log_info "启动完成！"
}

# 执行主函数
main