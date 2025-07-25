#!/bin/bash
# 启动完整的Docker测试环境

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_section() {
    echo -e "\n${BLUE}========== $1 ==========${NC}"
}

# 清理函数
cleanup() {
    log_section "清理环境"
    cd "$PROJECT_ROOT"
    
    if [ "$1" != "keep" ]; then
        log_info "停止并删除容器..."
        docker-compose -f docker-compose.test.yml down -v
    else
        log_info "保持容器运行（用于调试）"
    fi
}

# 错误处理
error_handler() {
    log_error "测试执行失败！"
    log_info "查看日志目录获取详细信息: $PROJECT_ROOT/logs/"
    cleanup
    exit 1
}

# 设置错误处理
trap error_handler ERR

# 检查依赖
check_dependencies() {
    log_section "检查依赖"
    
    # 检查Docker
    if ! command -v docker &> /dev/null; then
        log_error "未找到Docker，请先安装Docker"
        exit 1
    fi
    
    # 检查Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        log_error "未找到Docker Compose，请先安装Docker Compose"
        exit 1
    fi
    
    # 检查Docker是否运行
    if ! docker info &> /dev/null; then
        log_error "Docker未运行，请启动Docker服务"
        exit 1
    fi
    
    log_info "依赖检查通过"
}

# 准备测试环境
prepare_environment() {
    log_section "准备测试环境"
    
    cd "$PROJECT_ROOT"
    
    # 创建必要的目录
    log_info "创建测试目录..."
    mkdir -p logs/{redis,comsrv-simulator,modsrv,test-executor,collector}
    mkdir -p test-results/{api-messages,performance,logs}
    mkdir -p test-results/api-messages/{health_check,model_list,model_detail,control_commands,performance}
    
    # 清理旧的测试结果
    log_info "清理旧的测试结果..."
    find test-results -name "*.json" -type f -delete 2>/dev/null || true
    find test-results -name "*.log" -type f -delete 2>/dev/null || true
    find test-results -name "*.result" -type f -delete 2>/dev/null || true
    
    # 清理旧的日志
    log_info "清理旧的日志..."
    find logs -name "*.log" -type f -delete 2>/dev/null || true
    
    # 确保脚本可执行
    log_info "设置脚本权限..."
    chmod +x tests/*.sh 2>/dev/null || true
    chmod +x docker/test-executor/*.sh 2>/dev/null || true
    chmod +x docker/test-executor/*.py 2>/dev/null || true
    chmod +x docker/entrypoint.sh 2>/dev/null || true
    
    log_info "环境准备完成"
}

# 构建镜像
build_images() {
    log_section "构建Docker镜像"
    
    cd "$PROJECT_ROOT"
    
    # 检查是否需要重新构建
    if [ "$FORCE_BUILD" == "true" ] || [ "$1" == "force" ]; then
        log_info "强制重新构建所有镜像..."
        docker-compose -f docker-compose.test.yml build --no-cache
    else
        log_info "构建镜像（使用缓存）..."
        docker-compose -f docker-compose.test.yml build
    fi
    
    log_info "镜像构建完成"
}

# 启动服务
start_services() {
    log_section "启动测试服务"
    
    cd "$PROJECT_ROOT"
    
    # 启动服务
    log_info "启动Docker Compose服务..."
    docker-compose -f docker-compose.test.yml up -d
    
    # 等待服务健康
    log_info "等待服务就绪..."
    local max_wait=60
    local wait_time=0
    
    while [ $wait_time -lt $max_wait ]; do
        # 检查所有服务健康状态
        if docker-compose -f docker-compose.test.yml ps | grep -E "(unhealthy|starting)" > /dev/null; then
            log_info "等待服务启动... ($wait_time/$max_wait 秒)"
            sleep 5
            wait_time=$((wait_time + 5))
        else
            log_info "所有服务已就绪"
            break
        fi
    done
    
    if [ $wait_time -ge $max_wait ]; then
        log_error "服务启动超时"
        docker-compose -f docker-compose.test.yml ps
        exit 1
    fi
    
    # 显示服务状态
    log_info "服务状态:"
    docker-compose -f docker-compose.test.yml ps
}

# 监控测试执行
monitor_tests() {
    log_section "监控测试执行"
    
    cd "$PROJECT_ROOT"
    
    # 实时监控测试执行器日志
    log_info "监控测试执行（按Ctrl+C停止监控）..."
    
    # 使用timeout避免无限等待
    timeout 300 docker-compose -f docker-compose.test.yml logs -f test-executor || true
    
    # 检查测试是否完成
    if docker-compose -f docker-compose.test.yml ps test-executor | grep "Exit 0" > /dev/null; then
        log_info "测试执行成功完成"
    else
        log_warn "测试执行可能未成功完成"
    fi
}

# 收集测试结果
collect_results() {
    log_section "收集测试结果"
    
    cd "$PROJECT_ROOT"
    
    # 统计测试结果
    log_info "测试结果统计:"
    
    # 统计.result文件
    if [ -d "test-results" ]; then
        local passed=$(grep -l "PASS" test-results/*.result 2>/dev/null | wc -l || echo "0")
        local failed=$(grep -l "FAIL" test-results/*.result 2>/dev/null | wc -l || echo "0")
        local total=$((passed + failed))
        
        log_info "总测试数: $total"
        log_info "通过: $passed"
        log_info "失败: $failed"
        
        # 显示失败的测试
        if [ $failed -gt 0 ]; then
            log_warn "失败的测试:"
            grep -l "FAIL" test-results/*.result 2>/dev/null | xargs -I {} basename {} .result | sed 's/^/  - /'
        fi
    fi
    
    # 显示API消息统计
    if [ -d "test-results/api-messages" ]; then
        log_info "\nAPI消息统计:"
        for dir in test-results/api-messages/*/; do
            if [ -d "$dir" ]; then
                local count=$(find "$dir" -name "*.json" | wc -l)
                local dirname=$(basename "$dir")
                log_info "  $dirname: $count 条消息"
            fi
        done
    fi
    
    # 生成测试报告
    if command -v python3 &> /dev/null; then
        log_info "\n生成测试报告..."
        python3 scripts/generate-test-summary.py || log_warn "无法生成测试报告"
    fi
}

# 显示使用帮助
show_help() {
    echo "使用方法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  -h, --help      显示帮助信息"
    echo "  -f, --force     强制重新构建镜像"
    echo "  -k, --keep      测试后保持容器运行"
    echo "  -m, --monitor   仅监控正在运行的测试"
    echo "  -c, --clean     仅清理环境"
    echo ""
    echo "示例:"
    echo "  $0              # 运行完整测试"
    echo "  $0 -f           # 强制重建并运行测试"
    echo "  $0 -k           # 运行测试并保持容器"
    echo "  $0 -m           # 监控已运行的测试"
}

# 主函数
main() {
    local force_build=false
    local keep_containers=false
    local monitor_only=false
    local clean_only=false
    
    # 解析参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -f|--force)
                force_build=true
                shift
                ;;
            -k|--keep)
                keep_containers=true
                shift
                ;;
            -m|--monitor)
                monitor_only=true
                shift
                ;;
            -c|--clean)
                clean_only=true
                shift
                ;;
            *)
                log_error "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    # 仅清理
    if [ "$clean_only" == "true" ]; then
        cleanup
        exit 0
    fi
    
    # 仅监控
    if [ "$monitor_only" == "true" ]; then
        cd "$PROJECT_ROOT"
        docker-compose -f docker-compose.test.yml logs -f
        exit 0
    fi
    
    log_section "ModSrv Docker测试环境"
    log_info "项目目录: $PROJECT_ROOT"
    log_info "开始时间: $(date)"
    
    # 执行测试流程
    check_dependencies
    prepare_environment
    
    if [ "$force_build" == "true" ]; then
        FORCE_BUILD=true build_images
    else
        build_images
    fi
    
    start_services
    monitor_tests
    collect_results
    
    # 清理或保持
    if [ "$keep_containers" == "true" ]; then
        log_section "测试完成"
        log_info "容器保持运行状态"
        log_info "查看日志: docker-compose -f docker-compose.test.yml logs -f [service_name]"
        log_info "停止服务: docker-compose -f docker-compose.test.yml down"
    else
        cleanup
    fi
    
    log_info "结束时间: $(date)"
    log_section "测试执行完成"
}

# 执行主函数
main "$@"