#!/bin/bash
# 内部网络监控脚本

set -e

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 脚本目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${CYAN}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║     ModSrv 内部网络监控工具 v1.0              ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════╝${NC}"
    echo ""
}

# 显示菜单
show_menu() {
    print_header
    echo -e "${BLUE}请选择操作：${NC}"
    echo ""
    echo -e "  ${GREEN}1)${NC} 检查服务健康状态"
    echo -e "  ${GREEN}2)${NC} 查看Redis数据"
    echo -e "  ${GREEN}3)${NC} 测试API端点"
    echo -e "  ${GREEN}4)${NC} 查看容器日志"
    echo -e "  ${GREEN}5)${NC} 进入测试容器Shell"
    echo -e "  ${GREEN}6)${NC} 运行完整测试套件"
    echo -e "  ${GREEN}7)${NC} 查看测试报告"
    echo -e "  ${GREEN}8)${NC} 性能监控"
    echo -e "  ${GREEN}0)${NC} 退出"
    echo ""
    echo -n "请输入选择 [0-8]: "
}

# 检查健康状态
check_health() {
    print_header
    echo -e "${BLUE}=== 服务健康状态 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    # 容器状态
    echo -e "${YELLOW}容器状态:${NC}"
    docker-compose -f docker-compose.test.yml ps
    echo ""
    
    # 通过test-runner检查服务
    echo -e "${YELLOW}服务健康检查:${NC}"
    
    # Redis健康检查
    echo -n "Redis: "
    if docker-compose -f docker-compose.test.yml exec -T test-runner redis-cli -h redis ping > /dev/null 2>&1; then
        echo -e "${GREEN}✓ 正常${NC}"
    else
        echo -e "${RED}✗ 异常${NC}"
    fi
    
    # ModSrv健康检查
    echo -n "ModSrv API: "
    if docker-compose -f docker-compose.test.yml exec -T test-runner curl -sf http://modsrv:8092/health > /dev/null 2>&1; then
        echo -e "${GREEN}✓ 正常${NC}"
        echo "健康检查响应:"
        docker-compose -f docker-compose.test.yml exec -T test-runner curl -s http://modsrv:8092/health | jq . 2>/dev/null || \
        docker-compose -f docker-compose.test.yml exec -T test-runner curl -s http://modsrv:8092/health
    else
        echo -e "${RED}✗ 异常${NC}"
    fi
}

# 查看Redis数据
view_redis_data() {
    print_header
    echo -e "${BLUE}=== Redis数据查看 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    echo -e "${YELLOW}数据统计:${NC}"
    docker-compose -f docker-compose.test.yml exec -T test-runner sh -c '
        REDIS_CLI="redis-cli -h redis"
        echo -n "总键数: "
        $REDIS_CLI DBSIZE | cut -d" " -f2
        echo -n "测量点数量: "
        $REDIS_CLI --scan --pattern "*:m:*" | wc -l
        echo -n "信号点数量: "
        $REDIS_CLI --scan --pattern "*:s:*" | wc -l
        echo -n "模型数量: "
        $REDIS_CLI --scan --pattern "modsrv:model:*" | wc -l
        echo -n "实例数量: "
        $REDIS_CLI --scan --pattern "modsrv:instance:*" | wc -l
    '
    
    echo ""
    echo -e "${YELLOW}示例数据:${NC}"
    docker-compose -f docker-compose.test.yml exec -T test-runner sh -c '
        REDIS_CLI="redis-cli -h redis"
        echo "测量点示例:"
        $REDIS_CLI HGET "1003:m:10001" value
        echo "模型示例:"
        $REDIS_CLI GET "modsrv:model:power_meter_v1" | head -n 5
    '
}

# 测试API端点
test_api() {
    print_header
    echo -e "${BLUE}=== API端点测试 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    docker-compose -f docker-compose.test.yml exec -T test-runner sh -c '
        API_URL="http://modsrv:8092"
        
        echo "1. 健康检查:"
        curl -s $API_URL/health | jq . || curl -s $API_URL/health
        echo ""
        
        echo "2. API文档:"
        curl -s $API_URL/api/v1/docs | head -n 20
        echo ""
        
        echo "3. 模板列表:"
        curl -s $API_URL/api/v1/templates | jq . || echo "无模板数据"
    '
}

# 查看日志
view_logs() {
    print_header
    echo -e "${BLUE}=== 容器日志 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    echo -e "${YELLOW}选择要查看的日志:${NC}"
    echo "  1) ModSrv日志"
    echo "  2) Redis日志"
    echo "  3) 所有日志"
    echo -n "选择 [1-3]: "
    read log_choice
    
    case $log_choice in
        1) docker-compose -f docker-compose.test.yml logs --tail=50 -f modsrv ;;
        2) docker-compose -f docker-compose.test.yml logs --tail=50 -f redis ;;
        3) docker-compose -f docker-compose.test.yml logs --tail=50 -f ;;
        *) echo "无效选择" ;;
    esac
}

# 进入测试容器
enter_shell() {
    print_header
    echo -e "${BLUE}=== 进入测试容器Shell ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    log_info "进入test-runner容器..."
    echo -e "${YELLOW}提示: 使用 'exit' 退出容器${NC}"
    echo ""
    
    docker-compose -f docker-compose.test.yml exec test-runner bash
}

# 运行测试套件
run_tests() {
    print_header
    echo -e "${BLUE}=== 运行测试套件 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    log_info "初始化测试数据..."
    docker-compose -f docker-compose.test.yml run --rm data-generator
    
    log_info "运行测试..."
    docker-compose -f docker-compose.test.yml exec test-runner /scripts/run-internal-tests.sh
}

# 查看测试报告
view_reports() {
    print_header
    echo -e "${BLUE}=== 测试报告 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    if [ -f "test-reports/test_report.json" ]; then
        echo -e "${YELLOW}最新测试报告:${NC}"
        cat test-reports/test_report.json | jq . 2>/dev/null || cat test-reports/test_report.json
        
        echo ""
        echo -e "${YELLOW}测试摘要:${NC}"
        if [ -f "test-reports/summary.csv" ]; then
            cat test-reports/summary.csv
        fi
    else
        echo -e "${YELLOW}暂无测试报告${NC}"
    fi
}

# 性能监控
performance_monitor() {
    print_header
    echo -e "${BLUE}=== 性能监控 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    # 容器资源使用
    echo -e "${YELLOW}容器资源使用:${NC}"
    docker stats --no-stream modsrv-test modsrv-test-redis
    
    echo ""
    echo -e "${YELLOW}ModSrv指标:${NC}"
    docker-compose -f docker-compose.test.yml exec -T test-runner curl -s http://modsrv:9092/metrics | grep -E "process_|http_" | head -20
}

# 等待按键
wait_key() {
    echo ""
    echo -e "${CYAN}按任意键继续...${NC}"
    read -n 1
}

# 主循环
main() {
    while true; do
        show_menu
        read choice
        
        case $choice in
            1) check_health; wait_key ;;
            2) view_redis_data; wait_key ;;
            3) test_api; wait_key ;;
            4) view_logs ;;
            5) enter_shell ;;
            6) run_tests; wait_key ;;
            7) view_reports; wait_key ;;
            8) performance_monitor; wait_key ;;
            0) echo -e "\n${GREEN}退出监控工具${NC}"; exit 0 ;;
            *) echo -e "\n${RED}无效选择${NC}"; sleep 1 ;;
        esac
    done
}

# 检查依赖
if ! command -v docker &> /dev/null; then
    log_error "Docker未安装"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    log_error "Docker Compose未安装"
    exit 1
fi

# 执行主函数
main