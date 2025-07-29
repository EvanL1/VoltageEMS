#!/bin/bash
# hissrv 配置管理 API 使用示例

# API 基础 URL
API_URL="http://localhost:8082"

echo "=== hissrv 配置管理 API 示例 ==="
echo ""

# 1. 获取当前配置
echo "1. 获取当前完整配置:"
curl -s ${API_URL}/config | jq '.'
echo ""

# 2. 列出所有映射规则
echo "2. 列出所有映射规则:"
curl -s ${API_URL}/mappings | jq '.'
echo ""

# 3. 查找特定映射
echo "3. 查找特定映射 (archive:1m:*):"
curl -s ${API_URL}/mappings/archive:1m:* | jq '.'
echo ""

# 4. 添加新的映射规则
echo "4. 添加新的映射规则 (15分钟聚合):"
curl -X POST ${API_URL}/mappings \
  -H "Content-Type: application/json" \
  -d '{
    "source": "archive:15m:*",
    "measurement": "metrics_15m",
    "tags": [
      {"type": "extract", "field": "channel"},
      {"type": "static", "value": "interval=15m"}
    ],
    "fields": [
      {"name": "voltage_avg", "field_type": "float"},
      {"name": "current_avg", "field_type": "float"},
      {"name": "power_avg", "field_type": "float"},
      {"name": "energy_total", "field_type": "float"}
    ]
  }' | jq '.'
echo ""

# 5. 更新映射规则
echo "5. 更新映射规则 (修改 1m 映射):"
curl -X PUT ${API_URL}/mappings/archive:1m:* \
  -H "Content-Type: application/json" \
  -d '{
    "source": "archive:1m:*",
    "measurement": "metrics_1m_v2",
    "tags": [
      {"type": "extract", "field": "channel"},
      {"type": "static", "value": "interval=1m"},
      {"type": "static", "value": "version=2"}
    ],
    "fields": [
      {"name": "voltage_avg", "field_type": "float"},
      {"name": "voltage_max", "field_type": "float"},
      {"name": "voltage_min", "field_type": "float"},
      {"name": "current_avg", "field_type": "float"},
      {"name": "power_avg", "field_type": "float"},
      {"name": "power_factor", "field_type": "float"}
    ]
  }' | jq '.'
echo ""

# 6. 验证配置
echo "6. 验证当前配置:"
curl -X POST ${API_URL}/validate | jq '.'
echo ""

# 7. 重新加载配置（从文件）
echo "7. 重新加载配置:"
curl -X POST ${API_URL}/reload | jq '.'
echo ""

# 8. 删除映射规则
echo "8. 删除映射规则 (删除 15m 映射):"
curl -X DELETE ${API_URL}/mappings/archive:15m:* | jq '.'
echo ""

echo "=== 示例完成 ==="
echo ""
echo "提示："
echo "1. 使用 SIGHUP 信号也可以触发配置重载: kill -HUP <hissrv_pid>"
echo "2. 配置文件位置: config/hissrv.yaml"
echo "3. 所有修改会自动保存到配置文件"
echo "4. 配置修改后立即生效，无需重启服务"