#!/bin/bash

# 告警服务 API 测试脚本

export NO_PROXY=localhost
API_BASE="http://localhost:8087"

echo "=== 告警服务 API 测试 ==="

# 1. 健康检查
echo -e "\n1. 健康检查"
curl -s "$API_BASE/health"
echo

# 2. 服务状态
echo -e "\n2. 服务状态"
curl -s "$API_BASE/status" | jq .

# 3. 创建测试告警
echo -e "\n3. 创建测试告警"
curl -s -X POST "$API_BASE/alarms" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "测试告警 - 高温预警",
    "description": "设备温度超过阈值，当前温度85°C",
    "level": "Major"
  }' | jq .

# 4. 获取告警统计
echo -e "\n4. 获取告警统计"
curl -s "$API_BASE/stats" | jq .

# 5. 获取告警列表（分页）
echo -e "\n5. 获取告警列表"
curl -s "$API_BASE/alarms?limit=10&offset=0" | jq .

# 6. 测试筛选功能
echo -e "\n6. 测试筛选功能 - 按级别"
curl -s "$API_BASE/alarms?level=Major&limit=5" | jq .

# 7. 测试关键词搜索
echo -e "\n7. 测试关键词搜索"
curl -s "$API_BASE/alarms?keyword=温度&limit=5" | jq .

echo -e "\n=== 测试完成 ==="