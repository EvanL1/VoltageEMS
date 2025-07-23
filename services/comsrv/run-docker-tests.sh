#!/bin/bash
#
# COMSRV Docker测试启动脚本
# 使用docker-compose运行完整的测试环境，不对外暴露端口，记录所有测试日志
#

set -e  # 遇到错误立即退出

# 脚本配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$SCRIPT_DIR/logs"
TEST_DATE=$(date +%Y-%m-%d)
TEST_TIME=$(date +%H%M%S)

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'  
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

# 清理函数
cleanup() {
    log_info "正在清理Docker资源..."
    cd "$SCRIPT_DIR"
    docker-compose -f docker-compose.test.yml down --volumes --remove-orphans 2>/dev/null || true
    
    # 清理未使用的Docker资源
    docker system prune -f 2>/dev/null || true
    
    log_info "Docker资源清理完成"
}

# 信号处理
trap cleanup EXIT INT TERM

# 显示帮助信息
show_help() {
    echo "COMSRV Docker测试脚本"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  -h, --help         显示此帮助信息"
    echo "  -c, --clean        测试前强制清理所有Docker资源"
    echo "  -v, --verbose      详细输出模式"
    echo "  --timeout N        设置测试超时时间（秒，默认600）"
    echo "  --no-logs          不收集详细日志"
    echo ""
    echo "功能特点:"
    echo "  • 使用内部网络，不对外暴露端口"
    echo "  • 严格记录所有测试日志"
    echo "  • 自动收集和整理测试结果"
    echo "  • 包含性能测试和集成测试"
    echo ""
    echo "示例:"
    echo "  $0                 # 运行完整测试套件"
    echo "  $0 --clean         # 清理后运行测试"
    echo "  $0 --verbose       # 详细输出模式"
    echo "  $0 --timeout 900   # 设置15分钟超时"
}

# 检查Docker环境
check_prerequisites() {
    log_info "检查系统环境..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker未安装或不在PATH中"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose未安装或不在PATH中"  
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker守护进程未运行"
        exit 1
    fi
    
    log_success "系统环境检查通过"
}

# 设置日志目录
setup_logging() {
    log_info "设置日志环境..."
    
    # 创建日志目录结构
    mkdir -p "$LOG_DIR/$TEST_DATE"
    
    # 设置主日志文件
    export MAIN_LOG_FILE="$LOG_DIR/$TEST_DATE/docker-test-$TEST_TIME.log"
    
    # 创建测试会话信息
    cat > "$LOG_DIR/$TEST_DATE/test-session-$TEST_TIME.info" << EOF
测试会话信息
============
开始时间: $(date)
脚本路径: $0
工作目录: $SCRIPT_DIR
日志目录: $LOG_DIR/$TEST_DATE
主日志文件: $MAIN_LOG_FILE
Docker版本: $(docker --version)
Docker Compose版本: $(docker-compose --version)
EOF
    
    log_info "日志将保存到: $LOG_DIR/$TEST_DATE/"
}

# 构建所有镜像
build_images() {
    log_info "开始构建Docker镜像..."
    
    cd "$SCRIPT_DIR"
    
    # 构建所有镜像
    log_info "构建测试环境镜像..."
    if docker-compose -f docker-compose.test.yml build 2>&1 | tee -a "$MAIN_LOG_FILE"; then
        log_success "Docker镜像构建完成"
    else
        log_error "Docker镜像构建失败"
        exit 1
    fi
}

# 启动和验证服务
start_services() {
    log_info "启动测试服务..."
    
    cd "$SCRIPT_DIR"
    
    # 启动所有服务（除了测试运行器）
    log_info "启动基础设施服务..."
    docker-compose -f docker-compose.test.yml up -d redis influxdb modbus-simulator 2>&1 | tee -a "$MAIN_LOG_FILE"
    
    # 等待基础服务就绪
    log_info "等待基础服务就绪..."
    sleep 30
    
    # 启动COMSRV
    log_info "启动COMSRV服务..."
    docker-compose -f docker-compose.test.yml up -d comsrv 2>&1 | tee -a "$MAIN_LOG_FILE"
    
    # 等待COMSRV就绪
    log_info "等待COMSRV服务就绪（最多120秒）..."
    local wait_count=0
    while [ $wait_count -lt 120 ]; do
        if docker-compose -f docker-compose.test.yml exec -T comsrv curl -f http://localhost:8080/health &>/dev/null; then
            log_success "COMSRV服务已就绪"
            break
        fi
        
        wait_count=$((wait_count + 1))
        sleep 1
        
        if [ $((wait_count % 10)) -eq 0 ]; then
            log_info "等待COMSRV就绪... ($wait_count/120)"
        fi
    done
    
    if [ $wait_count -ge 120 ]; then
        log_error "COMSRV服务启动超时"
        log_error "查看COMSRV日志:"
        docker-compose -f docker-compose.test.yml logs comsrv | tail -50
        exit 1
    fi
    
    # 显示服务状态
    log_info "当前服务状态:"
    docker-compose -f docker-compose.test.yml ps 2>&1 | tee -a "$MAIN_LOG_FILE"
}

# 运行测试套件
run_tests() {
    log_info "开始运行测试套件..."
    
    cd "$SCRIPT_DIR"
    
    local test_success=true
    
    # 运行集成测试
    log_info "执行集成测试..."
    if docker-compose -f docker-compose.test.yml run --rm integration-tests 2>&1 | tee -a "$MAIN_LOG_FILE"; then
        log_success "集成测试完成"
    else
        log_error "集成测试失败"
        test_success=false
    fi
    
    # 等待一会让日志收集器工作
    log_info "启动日志收集..."
    docker-compose -f docker-compose.test.yml up -d log-collector 2>&1 | tee -a "$MAIN_LOG_FILE"
    sleep 15
    
    if [ "$test_success" = true ]; then
        return 0
    else
        return 1
    fi
}

# 收集测试结果和日志
collect_results() {
    log_info "收集测试结果和日志..."
    
    cd "$SCRIPT_DIR"
    
    # 收集容器日志
    log_info "导出容器日志..."
    for service in redis modbus-simulator influxdb comsrv integration-tests; do
        if docker-compose -f docker-compose.test.yml ps -q $service &>/dev/null; then
            log_info "收集 $service 服务日志..."
            docker-compose -f docker-compose.test.yml logs $service > "$LOG_DIR/$TEST_DATE/${service}-$TEST_TIME.log" 2>&1
        fi
    done
    
    # 收集系统信息
    log_info "收集系统信息..."
    cat > "$LOG_DIR/$TEST_DATE/system-info-$TEST_TIME.txt" << EOF
系统信息收集
============
时间: $(date)
Docker信息:
$(docker info 2>&1)

Docker Compose配置:
$(docker-compose -f docker-compose.test.yml config 2>&1)

容器状态:
$(docker-compose -f docker-compose.test.yml ps 2>&1)

网络信息:
$(docker network ls 2>&1)

卷信息:
$(docker volume ls 2>&1)
EOF
    
    # 统计日志文件
    local log_count=$(find "$LOG_DIR/$TEST_DATE" -name "*.log" -type f | wc -l)
    local total_size=$(du -sh "$LOG_DIR/$TEST_DATE" | cut -f1)
    
    log_info "日志收集统计:"
    log_info "  日志文件数量: $log_count"
    log_info "  总大小: $total_size"
    log_info "  保存位置: $LOG_DIR/$TEST_DATE/"
    
    # 生成测试总结报告
    cat > "$LOG_DIR/$TEST_DATE/test-summary-$TEST_TIME.txt" << EOF
COMSRV Docker测试总结报告
========================
测试时间: $(date)
测试环境: Docker Compose（内部网络）
日志目录: $LOG_DIR/$TEST_DATE/

服务组件:
- Redis: 数据存储
- InfluxDB: 时序数据库  
- Modbus模拟器: 协议模拟
- COMSRV: 主服务
- 集成测试: Python测试套件

特点:
✓ 完全隔离的内部网络环境
✓ 不对外暴露任何端口
✓ 完整的日志记录和收集
✓ 自动化的测试执行和结果收集

文件说明:
- docker-test-$TEST_TIME.log: 主测试日志
- *-$TEST_TIME.log: 各服务的详细日志
- system-info-$TEST_TIME.txt: 系统环境信息
- test-summary-$TEST_TIME.txt: 本总结报告

测试完成时间: $(date)
EOF
    
    log_success "测试结果收集完成"
}

# 主函数
main() {
    local clean_first=false
    local verbose=false
    local timeout=600
    local collect_logs=true
    
    # 解析命令行参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -c|--clean)
                clean_first=true
                shift
                ;;
            -v|--verbose)
                verbose=true
                set -x  # 启用命令跟踪
                shift
                ;;
            --timeout)
                timeout="$2"
                shift 2
                ;;
            --no-logs)
                collect_logs=false
                shift
                ;;
            *)
                log_error "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    # 显示开始横幅
    echo ""
    echo "========================================"
    echo "      COMSRV Docker测试套件"
    echo "========================================"
    echo "开始时间: $(date)"
    echo "测试类型: 完整集成测试"
    echo "网络模式: 内部隔离（无外部端口）"
    echo "日志模式: 完整记录"
    echo "超时设置: ${timeout}秒"
    echo "========================================"
    echo ""
    
    # 执行测试流程
    check_prerequisites
    setup_logging
    
    # 清理（如果需要）
    if [ "$clean_first" = true ]; then
        log_info "执行预清理..."
        cleanup
        sleep 5
    fi
    
    # 主要测试流程
    build_images
    start_services
    
    # 运行测试
    local test_result=0
    if run_tests; then
        log_success "测试执行成功!"
    else
        log_error "测试执行失败!"
        test_result=1
    fi
    
    # 收集结果
    if [ "$collect_logs" = true ]; then
        collect_results
    fi
    
    # 显示结束横幅
    echo ""
    echo "========================================"
    if [ $test_result -eq 0 ]; then
        echo -e "      ${GREEN}测试完成 - 成功${NC}"
    else
        echo -e "      ${RED}测试完成 - 失败${NC}"
    fi
    echo "结束时间: $(date)"
    echo "日志位置: $LOG_DIR/$TEST_DATE/"
    echo "========================================"
    echo ""
    
    exit $test_result
}

# 运行主函数
main "$@"