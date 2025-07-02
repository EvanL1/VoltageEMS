#!/bin/bash
# Grafana 演示环境快速启动脚本

echo "=== 启动 Grafana 演示环境 ==="

# 1. 启动 Docker 服务
echo "1. 启动 Grafana 和 InfluxDB..."
docker-compose -f docker-compose.grafana.yml up -d

# 等待服务启动
echo "等待服务启动..."
sleep 10

# 2. 检查服务状态
echo "2. 检查服务状态..."
docker-compose -f docker-compose.grafana.yml ps

# 3. 初始化 InfluxDB（如果需要）
echo "3. 配置 InfluxDB..."
# 创建额外的 bucket（如果需要）
docker exec voltage-influxdb influx bucket create \
  --name voltage-data \
  --org voltageems \
  --token voltage-super-secret-auth-token \
  --retention 30d 2>/dev/null || echo "Bucket 可能已存在"

# 4. 启动模拟数据服务
echo "4. 启动模拟数据服务..."
if [ -f "mock-data-server.js" ]; then
    echo "启动 mock-data-server..."
    node mock-data-server.js &
    MOCK_PID=$!
    echo "Mock data server PID: $MOCK_PID"
else
    echo "未找到 mock-data-server.js，跳过"
fi

# 5. 显示访问信息
echo ""
echo "=== 服务已启动 ==="
echo "Grafana: http://localhost:3000"
echo "  用户名: admin"
echo "  密码: admin"
echo ""
echo "InfluxDB: http://localhost:8086"
echo "  用户名: admin"
echo "  密码: password123"
echo ""
echo "前端应用: http://localhost:8081"
echo ""
echo "停止服务: docker-compose -f docker-compose.grafana.yml down"
echo ""

# 6. 可选：打开浏览器
if command -v open &> /dev/null; then
    echo "正在打开 Grafana..."
    sleep 2
    open http://localhost:3000
fi