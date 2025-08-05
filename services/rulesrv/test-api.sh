#!/bin/bash
# rulesrv API测试脚本

BASE_URL="${BASE_URL:-http://localhost:6003}"
API_URL="$BASE_URL/api/v1"

echo "🌐 rulesrv API测试"
echo "=================="
echo "Base URL: $BASE_URL"
echo

# 健康检查
echo "1️⃣ 健康检查"
echo "GET /health"
curl -s $BASE_URL/health | jq .
echo -e "\n"

# 获取示例规则
echo "2️⃣ 获取示例规则"
echo "GET /examples"
curl -s $API_URL/examples | jq .
echo -e "\n"

# 创建测试规则
echo "3️⃣ 创建电池管理规则"
echo "POST /rules"
BATTERY_RULE=$(curl -s -X POST $API_URL/rules \
  -H "Content-Type: application/json" \
  -d '{
    "rule": {
      "id": "battery_test_rule",
      "name": "Battery Test Rule",
      "description": "Test rule for battery management",
      "conditions": {
        "operator": "AND",
        "conditions": [
          {
            "source": "battery.soc",
            "operator": "<=",
            "value": 20.0,
            "description": "Battery SOC <= 20%"
          }
        ]
      },
      "actions": [
        {
          "action_type": "notify",
          "config": {
            "level": "warning",
            "message": "Battery SOC is low",
            "recipients": null
          },
          "description": "Send low battery notification"
        }
      ],
      "enabled": true,
      "priority": 1,
      "cooldown_seconds": 300
    }
  }')

echo "$BATTERY_RULE" | jq .
echo -e "\n"

# 列出所有规则
echo "4️⃣ 列出所有规则"
echo "GET /rules"
curl -s $API_URL/rules | jq .
echo -e "\n"

# 获取特定规则
echo "5️⃣ 获取特定规则"
echo "GET /rules/battery_test_rule"
curl -s $API_URL/rules/battery_test_rule | jq .
echo -e "\n"

# 准备测试数据
echo "6️⃣ 准备测试数据"
echo "设置 battery.soc = 15"
redis-cli SET battery.soc 15 > /dev/null
echo "✅ 测试数据已设置"
echo -e "\n"

# 执行规则
echo "7️⃣ 执行规则"
echo "POST /rules/battery_test_rule/execute"
EXEC_RESULT=$(curl -s -X POST $API_URL/rules/battery_test_rule/execute \
  -H "Content-Type: application/json" \
  -d '{"context": null}')

echo "$EXEC_RESULT" | jq .
echo -e "\n"

# 获取规则统计
echo "8️⃣ 获取规则统计"
echo "GET /rules/battery_test_rule/stats"
curl -s $API_URL/rules/battery_test_rule/stats | jq .
echo -e "\n"

# 测试规则（不保存）
echo "9️⃣ 测试电压监控规则"
echo "POST /rules/test"
TEST_RESULT=$(curl -s -X POST $API_URL/rules/test \
  -H "Content-Type: application/json" \
  -d '{
    "rule": {
      "id": "voltage_test",
      "name": "Voltage Test",
      "description": "Test voltage monitoring",
      "conditions": {
        "operator": "OR",
        "conditions": [
          {
            "source": "comsrv:1001:T.1",
            "operator": ">",
            "value": 240.0,
            "description": "Voltage > 240V"
          }
        ]
      },
      "actions": [
        {
          "action_type": "publish",
          "config": {
            "channel": "ems:alerts",
            "message": "High voltage detected"
          },
          "description": "Publish voltage alert"
        }
      ],
      "enabled": true,
      "priority": 2,
      "cooldown_seconds": 60
    },
    "context": null
  }')

echo "$TEST_RESULT" | jq .
echo -e "\n"

# 更新规则
echo "🔟 更新规则"
echo "PUT /rules/battery_test_rule"
UPDATE_RESULT=$(curl -s -X PUT $API_URL/rules/battery_test_rule \
  -H "Content-Type: application/json" \
  -d '{
    "rule": {
      "id": "battery_test_rule",
      "name": "Battery Test Rule (Updated)",
      "description": "Updated test rule for battery management",
      "conditions": {
        "operator": "AND",
        "conditions": [
          {
            "source": "battery.soc",
            "operator": "<=",
            "value": 25.0,
            "description": "Battery SOC <= 25%"
          }
        ]
      },
      "actions": [
        {
          "action_type": "notify",
          "config": {
            "level": "warning",
            "message": "Battery SOC is low (Updated threshold)",
            "recipients": null
          },
          "description": "Send low battery notification"
        }
      ],
      "enabled": true,
      "priority": 1,
      "cooldown_seconds": 300
    }
  }')

echo "$UPDATE_RESULT" | jq .
echo -e "\n"

# 获取执行历史
echo "1️⃣1️⃣ 获取执行历史"
echo "GET /rules/battery_test_rule/history"
curl -s $API_URL/rules/battery_test_rule/history | jq .
echo -e "\n"

# 删除规则
echo "1️⃣2️⃣ 删除规则"
echo "DELETE /rules/battery_test_rule"
curl -s -X DELETE $API_URL/rules/battery_test_rule | jq .
echo -e "\n"

# 清理测试数据
echo "🧹 清理测试数据"
redis-cli DEL battery.soc > /dev/null
echo "✅ 测试数据已清理"

echo
echo "✅ API测试完成！"