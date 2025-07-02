#!/bin/bash

# VoltageEMS 服务启动脚本
# 自动设置代理绕过并启动所有服务

echo "🚀 Starting VoltageEMS Services..."

# 设置代理绕过
export NO_PROXY=localhost,127.0.0.1,::1
export HTTP_PROXY=
export HTTPS_PROXY=

echo "✅ Proxy bypass configured"

# 检查 Docker 服务
echo "📊 Checking Docker services..."
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" | grep voltage

# 启动 Grafana 和 InfluxDB (如果没有运行)
if ! docker ps | grep -q voltage-grafana; then
    echo "🔧 Starting Grafana and InfluxDB..."
    docker-compose -f frontend/grafana/docker-compose.grafana.yml up -d
    echo "⏳ Waiting for Grafana to start..."
    sleep 15
fi

# 检查数据写入器
if ! pgrep -f "influxdb-writer.js" > /dev/null; then
    echo "📝 Starting data writer..."
    nohup node frontend/scripts/influxdb-writer.js > influxdb-writer.log 2>&1 &
    echo "✅ Data writer started"
fi

# 检查前端服务
if ! pgrep -f "vue-cli-service serve" > /dev/null; then
    echo "🌐 Starting frontend service..."
    cd frontend
    NO_PROXY=localhost,127.0.0.1 nohup npm run serve > ../frontend.log 2>&1 &
    cd ..
    echo "⏳ Waiting for frontend to compile..."
    sleep 20
fi

echo ""
echo "🎉 All services started!"
echo ""
echo "📍 Access URLs:"
echo "  Frontend:     http://localhost:8082/"
echo "  Grafana:      http://localhost:3050/"
echo "  Test Page:    frontend/public/test-pages/embedded-test-proxy.html"
echo ""
echo "💡 If you see connection refused:"
echo "  1. Open browser with --disable-web-security flag"
echo "  2. Or use the proxy version: embedded-test-proxy.html"
echo "  3. Or access through the main frontend"
echo ""

# 测试连接
echo "🔍 Testing connections..."
NO_PROXY=localhost,127.0.0.1 curl -s -o /dev/null -w "Frontend (8082): %{http_code}\n" http://localhost:8082/
NO_PROXY=localhost,127.0.0.1 curl -s -o /dev/null -w "Grafana (3050): %{http_code}\n" http://localhost:3050/
NO_PROXY=localhost,127.0.0.1 curl -s -o /dev/null -w "Proxy Path: %{http_code}\n" "http://localhost:8082/grafana/d-solo/simple-view?orgId=1&panelId=1"

echo ""
echo "✨ Setup complete! Open http://localhost:8082/ in your browser"