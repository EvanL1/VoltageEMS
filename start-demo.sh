#!/bin/bash
# VoltageEMS 演示环境快速启动脚本
# 包含 Grafana 监控和模拟数据

set -e

echo "=== VoltageEMS 演示环境启动脚本 ==="
echo ""

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 检查依赖
check_dependencies() {
    echo -e "${YELLOW}检查依赖项...${NC}"
    
    # 检查 Docker
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}错误: 未找到 Docker，请先安装 Docker${NC}"
        exit 1
    fi
    
    # 检查 Node.js
    if ! command -v node &> /dev/null; then
        echo -e "${RED}错误: 未找到 Node.js，请先安装 Node.js${NC}"
        exit 1
    fi
    
    # 检查 Redis
    if ! command -v redis-cli &> /dev/null; then
        echo -e "${YELLOW}警告: 未找到 Redis，将尝试通过 Docker 启动${NC}"
        USE_DOCKER_REDIS=true
    fi
    
    echo -e "${GREEN}✓ 依赖检查完成${NC}"
}

# 启动 Docker 服务
start_docker_services() {
    echo ""
    echo -e "${YELLOW}启动 Docker 服务...${NC}"
    
    # 启动 Grafana 和 InfluxDB
    docker-compose -f frontend/grafana/docker-compose.grafana.yml up -d
    
    # 如果需要，启动 Redis
    if [ "$USE_DOCKER_REDIS" = true ]; then
        docker run -d --name voltage-redis -p 6379:6379 redis:7-alpine 2>/dev/null || true
    fi
    
    # 等待服务启动
    echo "等待服务启动..."
    sleep 10
    
    # 检查服务状态
    docker-compose -f frontend/grafana/docker-compose.grafana.yml ps
    
    echo -e "${GREEN}✓ Docker 服务已启动${NC}"
}

# 安装前端依赖
install_frontend_deps() {
    echo ""
    echo -e "${YELLOW}检查前端依赖...${NC}"
    
    if [ ! -f "frontend/scripts/mock-data-generator.js" ]; then
        echo -e "${RED}错误: 未找到 frontend/scripts/mock-data-generator.js${NC}"
        exit 1
    fi
    
    # 检查是否需要安装 redis 包
    if ! npm list redis &> /dev/null; then
        echo "安装 redis npm 包..."
        npm install redis
    fi
    
    echo -e "${GREEN}✓ 前端依赖就绪${NC}"
}

# 启动模拟数据生成器
start_mock_data() {
    echo ""
    echo -e "${YELLOW}启动模拟数据生成器...${NC}"
    
    # 杀死之前的进程（如果存在）
    pkill -f "node frontend/scripts/mock-data-generator.js" 2>/dev/null || true
    
    # 启动新的数据生成器
    node frontend/scripts/mock-data-generator.js &
    MOCK_PID=$!
    echo "模拟数据生成器 PID: $MOCK_PID"
    
    # 保存 PID 到文件
    echo $MOCK_PID > .mock-data.pid
    
    echo -e "${GREEN}✓ 模拟数据生成器已启动${NC}"
}

# 创建 InfluxDB bucket（如果需要）
setup_influxdb() {
    echo ""
    echo -e "${YELLOW}配置 InfluxDB...${NC}"
    
    # 尝试创建额外的 bucket
    docker exec voltage-influxdb influx bucket create \
        --name voltage-data \
        --org voltageems \
        --token voltage-super-secret-auth-token \
        --retention 30d 2>/dev/null || echo "Bucket 可能已存在"
    
    echo -e "${GREEN}✓ InfluxDB 配置完成${NC}"
}

# 显示访问信息
show_info() {
    echo ""
    echo "============================================"
    echo -e "${GREEN}🚀 VoltageEMS 演示环境已启动！${NC}"
    echo "============================================"
    echo ""
    echo "📊 Grafana 监控面板:"
    echo "   URL: http://localhost:3000"
    echo "   用户名: admin"
    echo "   密码: admin"
    echo "   "
    echo "   预配置的仪表板:"
    echo "   - 温度监控面板 (simple-view)"
    echo "   - VoltageEMS 实时监控 (voltage-realtime)"
    echo ""
    echo "💾 InfluxDB 时序数据库:"
    echo "   URL: http://localhost:8086"
    echo "   用户名: admin"
    echo "   密码: password123"
    echo ""
    echo "🔄 模拟数据生成器:"
    echo "   状态: 运行中 (PID: $(cat .mock-data.pid 2>/dev/null || echo 'N/A'))"
    echo "   数据类型: 温度、电压、功率"
    echo "   发送间隔: 1秒"
    echo ""
    echo "🎯 前端应用:"
    echo "   启动命令: cd frontend && npm run serve"
    echo "   访问地址: http://localhost:8081"
    echo ""
    echo "============================================"
    echo ""
    echo "📝 常用命令:"
    echo "   查看日志: docker-compose -f frontend/grafana/docker-compose.grafana.yml logs -f"
    echo "   停止服务: ./stop-demo.sh"
    echo "   重启服务: ./restart-demo.sh"
    echo ""
}

# 主函数
main() {
    echo "开始时间: $(date)"
    
    # 执行各步骤
    check_dependencies
    start_docker_services
    install_frontend_deps
    setup_influxdb
    start_mock_data
    show_info
    
    # 可选：打开浏览器
    if command -v open &> /dev/null; then
        echo -e "${YELLOW}是否打开 Grafana？(y/n)${NC}"
        read -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            open http://localhost:3000
        fi
    fi
    
    echo ""
    echo -e "${GREEN}✨ 启动完成！${NC}"
    echo "结束时间: $(date)"
}

# 执行主函数
main