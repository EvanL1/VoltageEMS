#!/bin/bash
#
# COMSRV Docker测试启动脚本
# 使用docker-compose运行完整的测试环境
#

set -e  # 遇到错误立即退出

# 脚本配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOG_DIR="$PROJECT_DIR/logs"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'  
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 脚本目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"
WORKSPACE_ROOT="$( cd "$PROJECT_DIR/../.." && pwd )"

# 默认参数
TEST_TYPE="all"
KEEP_RUNNING=false
ENABLE_MONITORING=false
TIMEOUT=300

# 打印帮助信息
print_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -t, --test-type TYPE     测试类型: all, integration, performance (默认: all)"
    echo "  -k, --keep-running       测试后保持容器运行"
    echo "  -m, --monitoring         启用Prometheus和Grafana监控"
    echo "  --timeout SECONDS        测试超时时间 (默认: 300秒)"
    echo "  -h, --help              显示帮助信息"
    echo ""
    echo "Examples:"
    echo "  $0                      # 运行所有测试"
    echo "  $0 -t integration       # 只运行集成测试"
    echo "  $0 -t performance -m    # 运行性能测试并启用监控"
    echo "  $0 -k                   # 测试后保持容器运行"
}

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--test-type)
            TEST_TYPE="$2"
            shift 2
            ;;
        -k|--keep-running)
            KEEP_RUNNING=true
            shift
            ;;
        -m|--monitoring)
            ENABLE_MONITORING=true
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_help
            exit 1
            ;;
    esac
done

# 打印配置
echo -e "${GREEN}=== comsrv Docker测试环境 ===${NC}"
echo "项目目录: $PROJECT_DIR"
echo "测试类型: $TEST_TYPE"
echo "保持运行: $KEEP_RUNNING"
echo "启用监控: $ENABLE_MONITORING"
echo "超时时间: ${TIMEOUT}秒"
echo ""

# 进入项目目录
cd "$PROJECT_DIR"

# 清理函数
cleanup() {
    echo -e "\n${YELLOW}清理测试环境...${NC}"
    if [ "$KEEP_RUNNING" = false ]; then
        docker-compose -f docker-compose.test.yml down -v
    else
        echo -e "${YELLOW}保持容器运行，使用以下命令停止:${NC}"
        echo "cd $PROJECT_DIR && docker-compose -f docker-compose.test.yml down -v"
    fi
}

# 设置清理钩子
trap cleanup EXIT INT TERM

# 步骤1: 清理旧环境
echo -e "${GREEN}步骤1: 清理旧环境${NC}"
docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true

# 步骤2: 构建镜像
echo -e "\n${GREEN}步骤2: 构建Docker镜像${NC}"
# 从workspace根目录构建
cd "$WORKSPACE_ROOT"
docker-compose -f services/comsrv/docker-compose.test.yml build --no-cache comsrv
cd "$PROJECT_DIR"

# 构建测试运行器镜像
docker-compose -f docker-compose.test.yml build test-runner

# 步骤3: 启动基础服务
echo -e "\n${GREEN}步骤3: 启动基础服务${NC}"
if [ "$ENABLE_MONITORING" = true ]; then
    docker-compose -f docker-compose.test.yml --profile monitoring up -d redis modbus-simulator prometheus grafana
else
    docker-compose -f docker-compose.test.yml up -d redis modbus-simulator
fi

# 等待Redis就绪
echo -e "${YELLOW}等待Redis就绪...${NC}"
for i in {1..30}; do
    if docker-compose -f docker-compose.test.yml exec -T redis redis-cli ping >/dev/null 2>&1; then
        echo -e "${GREEN}Redis已就绪${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}Redis启动超时${NC}"
        exit 1
    fi
    sleep 1
done

# 步骤4: 启动comsrv
echo -e "\n${GREEN}步骤4: 启动comsrv服务${NC}"
docker-compose -f docker-compose.test.yml up -d comsrv

# 等待comsrv就绪
echo -e "${YELLOW}等待comsrv就绪...${NC}"
for i in {1..60}; do
    # 使用docker-compose exec检查容器内部健康状态
    if docker-compose -f docker-compose.test.yml exec -T comsrv curl -f http://localhost:3000/api/health >/dev/null 2>&1; then
        echo -e "${GREEN}comsrv已就绪${NC}"
        break
    fi
    if [ $i -eq 60 ]; then
        echo -e "${RED}comsrv启动超时${NC}"
        echo -e "${YELLOW}查看日志:${NC}"
        docker-compose -f docker-compose.test.yml logs comsrv
        exit 1
    fi
    sleep 1
done

# 显示服务状态
echo -e "\n${GREEN}服务状态:${NC}"
docker-compose -f docker-compose.test.yml ps

# 步骤5: 运行测试
echo -e "\n${GREEN}步骤5: 运行测试${NC}"

# 创建测试结果目录
mkdir -p test-results

case $TEST_TYPE in
    integration)
        echo -e "${YELLOW}运行集成测试...${NC}"
        docker-compose -f docker-compose.test.yml run --rm \
            -e TEST_TIMEOUT=$TIMEOUT \
            test-runner \
            python -m pytest -v /app/tests/docker/integration_test.py \
            --junit-xml=/test-results/integration-junit.xml
        ;;
    performance)
        echo -e "${YELLOW}运行性能测试...${NC}"
        docker-compose -f docker-compose.test.yml run --rm \
            -e TEST_TIMEOUT=$TIMEOUT \
            test-runner \
            python /app/tests/docker/performance_test.py
        ;;
    all)
        echo -e "${YELLOW}运行所有测试...${NC}"
        # 集成测试
        docker-compose -f docker-compose.test.yml run --rm \
            -e TEST_TIMEOUT=$TIMEOUT \
            test-runner \
            python -m pytest -v /app/tests/docker/integration_test.py \
            --junit-xml=/test-results/integration-junit.xml
        
        # 性能测试
        docker-compose -f docker-compose.test.yml run --rm \
            -e TEST_TIMEOUT=$TIMEOUT \
            test-runner \
            python /app/tests/docker/performance_test.py
        ;;
    *)
        echo -e "${RED}未知的测试类型: $TEST_TYPE${NC}"
        exit 1
        ;;
esac

# 检查测试结果
if [ $? -eq 0 ]; then
    echo -e "\n${GREEN}测试成功!${NC}"
else
    echo -e "\n${RED}测试失败!${NC}"
    echo -e "${YELLOW}查看日志:${NC}"
    docker-compose -f docker-compose.test.yml logs comsrv
    exit 1
fi

# 显示监控信息（如果启用）
if [ "$ENABLE_MONITORING" = true ]; then
    echo -e "\n${GREEN}监控服务已启动:${NC}"
    echo "Prometheus: http://localhost:9090"
    echo "Grafana: http://localhost:3001 (用户名: admin, 密码: admin)"
fi

# 保持运行提示
if [ "$KEEP_RUNNING" = true ]; then
    echo -e "\n${GREEN}服务保持运行中...${NC}"
    echo "查看comsrv日志: docker-compose -f docker-compose.test.yml logs -f comsrv"
    echo "Redis CLI: docker-compose -f docker-compose.test.yml exec redis redis-cli -a testpass123"
    echo "容器内部API: docker-compose -f docker-compose.test.yml exec comsrv curl http://localhost:3000/api/health"
    echo ""
    echo -e "${YELLOW}按Ctrl+C停止服务${NC}"
    
    # 等待用户中断
    while true; do
        sleep 1
    done
fi