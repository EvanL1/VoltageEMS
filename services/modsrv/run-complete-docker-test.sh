#!/bin/bash

# ModSrv完整Docker测试环境启动脚本
# 
# 功能:
# - 启动完整内部网络的Docker测试环境
# - 严格记录所有测试日志
# - 主程序配置文件和日志映射到本地
# - 不对外暴露任何端口

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目信息
PROJECT_NAME="modsrv-complete-test"
COMPOSE_FILE="docker-compose.complete-test.yml"

# 确保当前在正确目录
cd "$(dirname "$0")"

echo -e "${BLUE}🚀 ModSrv完整Docker测试环境${NC}"
echo "=================================="

# 检查Docker和docker-compose
command -v docker >/dev/null 2>&1 || { echo -e "${RED}❌ Docker未安装${NC}"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { echo -e "${RED}❌ docker-compose未安装${NC}"; exit 1; }

# 创建必要的目录
echo -e "${YELLOW}📁 创建本地目录...${NC}"
mkdir -p logs test-reports data templates
mkdir -p test-configs/models
mkdir -p logs/{tests,monitoring}

# 设置权限
chmod 755 logs test-reports data templates
chmod 644 test-configs/config.yml 2>/dev/null || true

echo -e "${GREEN}✅ 目录创建完毕${NC}"

# 清理现有容器
echo -e "${YELLOW}🧹 清理现有容器...${NC}"
docker-compose -f $COMPOSE_FILE down --remove-orphans 2>/dev/null || true
docker system prune -f --volumes 2>/dev/null || true

# 构建和启动服务
echo -e "${YELLOW}🔨 构建并启动服务...${NC}"
echo "注意: 完全内部网络，不暴露任何端口到宿主机"

# 启动所有服务
docker-compose -f $COMPOSE_FILE up -d --build

# 等待服务启动
echo -e "${YELLOW}⏳ 等待服务启动...${NC}"
sleep 10

# 检查服务状态
echo -e "${BLUE}📊 服务状态检查:${NC}"
echo "=================================="

# 检查容器状态
containers=(
    "modsrv-complete-redis"
    "modsrv-complete-comsrv-simulator"
    "modsrv-complete-service"
    "modsrv-complete-test-executor"
    "modsrv-complete-log-monitor"
    "modsrv-complete-data-validator"
)

all_healthy=true
for container in "${containers[@]}"; do
    if docker ps --format "table {{.Names}}\t{{.Status}}" | grep -q "$container.*Up"; then
        echo -e "${GREEN}✅ $container${NC}"
    else
        echo -e "${RED}❌ $container${NC}"
        all_healthy=false
    fi
done

if [ "$all_healthy" = true ]; then
    echo -e "${GREEN}🎉 所有服务启动成功！${NC}"
else
    echo -e "${RED}⚠️  部分服务启动失败，请检查日志${NC}"
fi

echo ""
echo -e "${BLUE}📋 环境信息:${NC}"
echo "=================================="
echo "• 项目名称: $PROJECT_NAME"
echo "• 配置文件: $COMPOSE_FILE"
echo "• 网络模式: 完全内部网络 (internal: true)"
echo "• 外部端口: 无 (零端口暴露)"
echo ""
echo -e "${BLUE}📂 本地映射目录:${NC}"
echo "• 配置文件: ./test-configs → /config (主程序配置)"
echo "• 日志目录: ./logs → /logs (所有服务日志)"
echo "• 数据目录: ./data → /data (数据持久化)"
echo "• 模板目录: ./templates → /templates (设备模型模板)"
echo "• 测试报告: ./test-reports (测试结果输出)"
echo ""
echo -e "${BLUE}🔍 监控和日志:${NC}"
echo "• Redis日志: ./logs/redis.log"
echo "• ModSrv日志: ./logs/modsrv.log"
echo "• ComsRv模拟器日志: ./logs/comsrv-simulator.log"
echo "• 系统监控日志: ./logs/monitoring/system-monitor.log"
echo "• 数据验证日志: ./logs/data-validation.log"
echo "• 测试执行日志: ./logs/tests/"
echo ""
echo -e "${BLUE}🎯 主要功能:${NC}"
echo "• Redis 8 数据存储 (Hash格式v3.2规范)"
echo "• ComsRv数据模拟器 (实时数据生成)"
echo "• ModSrv设备模型引擎 (数据处理和计算)"
echo "• 完整测试套件 (10+测试场景)"
echo "• 实时系统监控 (30秒间隔)"
echo "• 数据格式验证 (1分钟间隔)"
echo ""
echo -e "${YELLOW}💡 使用指南:${NC}"
echo "1. 查看实时日志:"
echo "   docker-compose -f $COMPOSE_FILE logs -f"
echo ""
echo "2. 查看特定服务日志:"
echo "   docker-compose -f $COMPOSE_FILE logs -f modsrv"
echo "   docker-compose -f $COMPOSE_FILE logs -f test-executor"
echo ""
echo "3. 进入容器内部:"
echo "   docker exec -it modsrv-complete-service /bin/bash"
echo ""
echo "4. 查看本地日志文件:"
echo "   tail -f logs/modsrv.log"
echo "   tail -f logs/monitoring/system-monitor.log"
echo ""
echo "5. 停止环境:"
echo "   docker-compose -f $COMPOSE_FILE down"
echo ""
echo -e "${GREEN}🎉 环境启动完毕！开始自动化测试...${NC}"
echo "测试进度可通过以下方式查看:"
echo "• 实时日志: docker-compose -f $COMPOSE_FILE logs -f test-executor"
echo "• 本地日志目录: ./logs/tests/"
echo "• 测试报告目录: ./test-reports/"

# 等待用户操作
echo ""
echo -e "${YELLOW}按 Ctrl+C 停止监控，环境将继续在后台运行${NC}"
echo -e "${YELLOW}或按 Enter 继续查看实时日志...${NC}"
read -t 5 -n 1 -s || true

# 显示实时日志
echo -e "${BLUE}📊 实时日志输出:${NC}"
echo "=================================="
docker-compose -f $COMPOSE_FILE logs -f