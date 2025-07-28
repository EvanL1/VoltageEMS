#!/bin/bash
#
# 启动Comsrv Docker测试环境
#

set -e

echo "=== 启动Comsrv Docker测试环境 ==="
echo "时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo ""

# 创建必要的目录
echo "创建日志和结果目录..."
mkdir -p logs/{comsrv,redis-monitor,validator}
mkdir -p logs/{CH1001,CH1002,CH1003,CH1004,CH1005,CH1006,CH2001,CH2002}
mkdir -p logs/{modbus-1001,modbus-1002,modbus-1003,modbus-1004,modbus-1005,modbus-1006}
mkdir -p logs/{modbus-2001,modbus-2002}
mkdir -p test-results

# 清理旧容器和卷
echo ""
echo "清理旧容器..."
docker-compose down -v || true

# 构建镜像
echo ""
echo "构建Docker镜像..."
docker-compose build

# 启动服务
echo ""
echo "启动所有服务..."
docker-compose up -d

# 等待服务启动
echo ""
echo "等待服务启动..."
sleep 10

# 检查服务状态
echo ""
echo "=== 服务状态 ==="
docker-compose ps

# 检查健康状态
echo ""
echo "=== 健康检查 ==="
for service in redis modbus-sim-1001 modbus-sim-1002 modbus-sim-1003 modbus-sim-1004 modbus-sim-1005 modbus-sim-1006 modbus-rtu-2001 modbus-rtu-2002 comsrv; do
    echo -n "检查 $service ... "
    if docker-compose ps | grep -q "$service.*healthy"; then
        echo "✓ 健康"
    else
        echo "✗ 不健康或未就绪"
    fi
done

echo ""
echo "=== 测试环境已启动 ==="
echo ""
echo "查看日志:"
echo "  - comsrv日志: tail -f logs/comsrv/comsrv.log"
echo "  - 通道日志: tail -f logs/CH*/channel.log"
echo "  - Redis监控: tail -f logs/redis-monitor/*.log"
echo "  - 验证结果: cat test-results/validation_result.json"
echo ""
echo "查看实时日志:"
echo "  docker-compose logs -f comsrv"
echo ""
echo "停止环境:"
echo "  ./stop-docker-test.sh"
echo ""