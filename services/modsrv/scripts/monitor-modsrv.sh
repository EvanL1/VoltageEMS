#!/bin/bash

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# 配置
REDIS_CLI="docker-compose -f $PROJECT_DIR/docker-compose.test.yml exec -T redis redis-cli"
MODSRV_API="http://localhost:8092"
METRICS_URL="http://localhost:9092/metrics"

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

print_header() {
    clear
    echo -e "${CYAN}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║         ModSrv 监控工具 v1.0                   ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════╝${NC}"
    echo ""
}

# 显示菜单
show_menu() {
    print_header
    echo -e "${BLUE}请选择操作：${NC}"
    echo ""
    echo -e "  ${GREEN}1)${NC} 查看服务状态"
    echo -e "  ${GREEN}2)${NC} 查看Redis数据统计"
    echo -e "  ${GREEN}3)${NC} 监控实时数据流"
    echo -e "  ${GREEN}4)${NC} 查看模型计算日志"
    echo -e "  ${GREEN}5)${NC} 测试API端点"
    echo -e "  ${GREEN}6)${NC} 查看Prometheus指标"
    echo -e "  ${GREEN}7)${NC} 执行性能测试"
    echo -e "  ${GREEN}8)${NC} 清理测试数据"
    echo -e "  ${GREEN}0)${NC} 退出"
    echo ""
    echo -n "请输入选择 [0-8]: "
}

# 检查服务状态
check_service_status() {
    print_header
    echo -e "${BLUE}=== 服务状态 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    # Docker容器状态
    echo -e "${YELLOW}Docker容器状态:${NC}"
    docker-compose -f docker-compose.test.yml ps
    echo ""
    
    # Redis连接测试
    echo -e "${YELLOW}Redis连接测试:${NC}"
    if $REDIS_CLI ping > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Redis连接正常${NC}"
        echo -e "Redis信息:"
        $REDIS_CLI INFO server | grep -E "redis_version|tcp_port|uptime_in_seconds"
    else
        echo -e "${RED}✗ Redis连接失败${NC}"
    fi
    echo ""
    
    # ModSrv健康检查
    echo -e "${YELLOW}ModSrv健康检查:${NC}"
    if curl -sf $MODSRV_API/health > /dev/null; then
        echo -e "${GREEN}✓ ModSrv服务正常${NC}"
        health_response=$(curl -s $MODSRV_API/health)
        echo "健康检查响应: $health_response"
    else
        echo -e "${RED}✗ ModSrv服务异常${NC}"
    fi
}

# 查看Redis数据统计
show_redis_stats() {
    print_header
    echo -e "${BLUE}=== Redis数据统计 ===${NC}\n"
    
    # 键空间统计
    echo -e "${YELLOW}键空间统计:${NC}"
    $REDIS_CLI DBSIZE
    echo ""
    
    # 按类型统计键数量
    echo -e "${YELLOW}按模式统计键数量:${NC}"
    
    # 统计comsrv数据键
    echo -n "通道测量点(*/m/*): "
    $REDIS_CLI --scan --pattern "*:m:*" | wc -l
    
    echo -n "通道信号点(*/s/*): "
    $REDIS_CLI --scan --pattern "*:s:*" | wc -l
    
    echo -n "控制命令(modsrv:cmd:*): "
    $REDIS_CLI --scan --pattern "modsrv:cmd:*" | wc -l
    
    echo -n "模型输出(modsrv:output:*): "
    $REDIS_CLI --scan --pattern "modsrv:output:*" | wc -l
    
    echo -n "监视值(modsrv:*:measurement): "
    $REDIS_CLI --scan --pattern "modsrv:*:measurement" | wc -l
    
    echo ""
    
    # 内存使用情况
    echo -e "${YELLOW}内存使用情况:${NC}"
    $REDIS_CLI INFO memory | grep -E "used_memory_human|used_memory_peak_human"
}

# 监控实时数据流
monitor_data_flow() {
    print_header
    echo -e "${BLUE}=== 实时数据流监控 ===${NC}\n"
    echo -e "${YELLOW}监控Redis发布订阅通道...${NC}"
    echo -e "${CYAN}按 Ctrl+C 退出监控${NC}\n"
    
    # 使用psubscribe监控所有通道
    $REDIS_CLI PSUBSCRIBE "*" | while read line; do
        if [[ $line == *"message"* ]]; then
            timestamp=$(date '+%H:%M:%S')
            echo -e "${GREEN}[$timestamp]${NC} $line"
        fi
    done
}

# 查看模型计算日志
show_model_logs() {
    print_header
    echo -e "${BLUE}=== 模型计算日志 ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    echo -e "${YELLOW}最近的模型计算日志:${NC}"
    docker-compose -f docker-compose.test.yml logs --tail=50 modsrv | grep -E "model|calculation|engine" | tail -20
    
    echo ""
    echo -e "${CYAN}按任意键继续查看实时日志，或按 Ctrl+C 退出${NC}"
    read -n 1
    
    docker-compose -f docker-compose.test.yml logs -f modsrv | grep -E "model|calculation|engine"
}

# 测试API端点
test_api_endpoints() {
    print_header
    echo -e "${BLUE}=== API端点测试 ===${NC}\n"
    
    # 健康检查
    echo -e "${YELLOW}1. 健康检查端点:${NC}"
    echo "GET $MODSRV_API/health"
    curl -s $MODSRV_API/health | jq . || echo "响应: $(curl -s $MODSRV_API/health)"
    echo ""
    
    # 获取模型列表
    echo -e "${YELLOW}2. 获取模型列表:${NC}"
    echo "GET $MODSRV_API/api/v1/models"
    curl -s $MODSRV_API/api/v1/models | jq . || echo "响应: $(curl -s $MODSRV_API/api/v1/models)"
    echo ""
    
    # 获取设备模型列表
    echo -e "${YELLOW}3. 获取设备模型列表:${NC}"
    echo "GET $MODSRV_API/api/v1/device-models"
    curl -s $MODSRV_API/api/v1/device-models | jq . || echo "响应: $(curl -s $MODSRV_API/api/v1/device-models)"
    echo ""
    
    # 调试端点
    if [ -n "$($MODSRV_API/debug/models 2>/dev/null)" ]; then
        echo -e "${YELLOW}4. 调试端点:${NC}"
        echo "GET $MODSRV_API/debug/models"
        curl -s $MODSRV_API/debug/models | jq . || echo "响应: $(curl -s $MODSRV_API/debug/models)"
    fi
}

# 查看Prometheus指标
show_metrics() {
    print_header
    echo -e "${BLUE}=== Prometheus指标 ===${NC}\n"
    
    echo -e "${YELLOW}获取指标数据...${NC}"
    
    # 获取关键指标
    metrics=$(curl -s $METRICS_URL)
    
    if [ -n "$metrics" ]; then
        echo -e "\n${GREEN}关键指标:${NC}"
        echo "$metrics" | grep -E "modsrv_" | grep -v "#" | head -20
        
        echo -e "\n${GREEN}HTTP请求统计:${NC}"
        echo "$metrics" | grep -E "http_requests_total|http_request_duration" | grep -v "#"
        
        echo -e "\n${GREEN}系统资源:${NC}"
        echo "$metrics" | grep -E "process_cpu_seconds_total|process_resident_memory_bytes" | grep -v "#"
    else
        echo -e "${RED}无法获取指标数据${NC}"
    fi
}

# 执行性能测试
run_performance_test() {
    print_header
    echo -e "${BLUE}=== 性能测试 ===${NC}\n"
    
    echo -e "${YELLOW}简单性能测试 - 并发请求健康检查端点${NC}"
    echo "测试参数: 100个请求，10个并发"
    echo ""
    
    # 使用ab工具进行简单测试
    if command -v ab &> /dev/null; then
        ab -n 100 -c 10 $MODSRV_API/health
    else
        echo -e "${YELLOW}使用curl进行简单测试...${NC}"
        
        start_time=$(date +%s)
        success_count=0
        
        for i in {1..100}; do
            if curl -sf $MODSRV_API/health > /dev/null; then
                ((success_count++))
            fi
            
            # 显示进度
            if [ $((i % 10)) -eq 0 ]; then
                echo -ne "\r进度: $i/100"
            fi
        done
        
        end_time=$(date +%s)
        duration=$((end_time - start_time))
        
        echo -e "\n\n${GREEN}测试结果:${NC}"
        echo "总请求数: 100"
        echo "成功请求: $success_count"
        echo "失败请求: $((100 - success_count))"
        echo "总耗时: ${duration}秒"
        echo "平均响应时间: $((duration * 1000 / 100))ms"
    fi
}

# 清理测试数据
clean_test_data() {
    print_header
    echo -e "${BLUE}=== 清理测试数据 ===${NC}\n"
    
    echo -e "${YELLOW}警告: 这将清除Redis中的所有测试数据！${NC}"
    echo -n "确定要继续吗？(y/N) "
    read -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "\n${YELLOW}清理数据中...${NC}"
        
        # 清理特定模式的键
        patterns=("modsrv:*" "*:m:*" "*:s:*" "*:c:*" "*:a:*")
        
        for pattern in "${patterns[@]}"; do
            echo -n "清理 $pattern ... "
            count=$($REDIS_CLI --scan --pattern "$pattern" | wc -l)
            if [ $count -gt 0 ]; then
                $REDIS_CLI --scan --pattern "$pattern" | xargs -I {} $REDIS_CLI DEL {} > /dev/null 2>&1
                echo -e "${GREEN}已删除 $count 个键${NC}"
            else
                echo -e "${YELLOW}无数据${NC}"
            fi
        done
        
        echo -e "\n${GREEN}清理完成！${NC}"
    else
        echo -e "${YELLOW}操作已取消${NC}"
    fi
}

# 等待用户按键
wait_for_key() {
    echo ""
    echo -e "${CYAN}按任意键返回主菜单...${NC}"
    read -n 1
}

# 主循环
main() {
    while true; do
        show_menu
        read choice
        
        case $choice in
            1)
                check_service_status
                wait_for_key
                ;;
            2)
                show_redis_stats
                wait_for_key
                ;;
            3)
                monitor_data_flow
                ;;
            4)
                show_model_logs
                ;;
            5)
                test_api_endpoints
                wait_for_key
                ;;
            6)
                show_metrics
                wait_for_key
                ;;
            7)
                run_performance_test
                wait_for_key
                ;;
            8)
                clean_test_data
                wait_for_key
                ;;
            0)
                echo -e "\n${GREEN}退出监控工具${NC}"
                exit 0
                ;;
            *)
                echo -e "\n${RED}无效选择，请重试${NC}"
                sleep 1
                ;;
        esac
    done
}

# 检查依赖
check_dependencies() {
    if ! command -v docker &> /dev/null; then
        log_error "Docker未安装"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose未安装"
        exit 1
    fi
    
    cd "$PROJECT_DIR"
    if [ ! -f "docker-compose.test.yml" ]; then
        log_error "找不到docker-compose.test.yml文件"
        exit 1
    fi
}

# 启动
check_dependencies
main