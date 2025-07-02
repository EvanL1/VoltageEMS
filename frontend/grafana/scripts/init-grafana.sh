#!/bin/bash

echo "初始化 Grafana..."

# 等待 Grafana 启动
echo "等待 Grafana 启动..."
for i in {1..30}; do
  if curl -s http://localhost:3000/api/health >/dev/null 2>&1; then
    echo "Grafana 已启动"
    break
  fi
  echo -n "."
  sleep 1
done

# 使用默认凭据
AUTH="admin:admin"

# 1. 创建 SimpleJSON 数据源
echo "创建数据源..."
curl -X POST http://$AUTH@localhost:3000/api/datasources \
  -H "Content-Type: application/json" \
  -d '{
    "name": "SimpleJSON",
    "type": "grafana-simple-json-datasource",
    "url": "http://host.docker.internal:3001",
    "access": "proxy",
    "isDefault": true,
    "jsonData": {}
  }' 2>/dev/null

echo ""

# 2. 导入仪表板
echo "导入温度监控仪表板..."
curl -X POST http://$AUTH@localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana-dashboard.json 2>/dev/null

echo ""

echo "导入综合监控仪表板..."
curl -X POST http://$AUTH@localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana-dashboard-realtime.json 2>/dev/null

echo ""

# 3. 设置默认仪表板
echo "设置默认仪表板..."
curl -X PUT http://$AUTH@localhost:3000/api/org/preferences \
  -H "Content-Type: application/json" \
  -d '{
    "homeDashboardUID": "simple-view"
  }' 2>/dev/null

echo ""
echo "Grafana 初始化完成！"
echo "访问 http://localhost:3000 (用户名: admin, 密码: admin)"
echo "或访问 http://localhost:8080/grafana-embedded 查看嵌入式视图"