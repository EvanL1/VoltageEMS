#!/bin/bash
# rulesrv规则执行测试脚本

echo "🎯 rulesrv规则执行测试"
echo "====================="
echo

# 启动服务前准备
echo "📦 准备测试环境..."

# 检查Redis
if ! redis-cli ping > /dev/null 2>&1; then
    echo "❌ Redis未运行，请先启动Redis"
    exit 1
fi

# 清理旧数据
redis-cli --scan --pattern "rulesrv:*" | xargs -L 100 redis-cli DEL 2>/dev/null || true
redis-cli --scan --pattern "battery.*" | xargs -L 100 redis-cli DEL 2>/dev/null || true
redis-cli --scan --pattern "comsrv:*" | xargs -L 100 redis-cli DEL 2>/dev/null || true

# 创建测试数据
echo "📊 创建测试数据..."
# 电池数据
redis-cli SET battery.soc 85 > /dev/null
redis-cli SET battery.voltage 48.5 > /dev/null
redis-cli SET battery.current 10.2 > /dev/null
redis-cli SET battery.temperature 25.5 > /dev/null

# 电压数据（模拟comsrv格式）
redis-cli HSET comsrv:1001:T 1 "230.5" > /dev/null
redis-cli HSET comsrv:1001:T 2 "231.2" > /dev/null
redis-cli HSET comsrv:1001:T 3 "229.8" > /dev/null

# 发电机状态
redis-cli SET generator.status "stopped" > /dev/null
redis-cli SET generator.fuel 80 > /dev/null

echo "✅ 测试数据已创建"
echo

# 测试规则1：电池低电量启动发电机
echo "🔋 测试规则1：电池低电量启动发电机"
echo "================================="
cat > /tmp/battery_low_rule.json << 'EOF'
{
  "id": "battery_low_start_gen",
  "name": "低电量启动发电机",
  "description": "当电池电量低于20%时启动发电机",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "battery.soc",
        "operator": "<=",
        "value": 20.0,
        "description": "电池SOC <= 20%"
      },
      {
        "source": "generator.status",
        "operator": "==",
        "value": "stopped",
        "description": "发电机处于停止状态"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "generator_001",
        "channel": "control",
        "point": "start",
        "value": true
      },
      "description": "启动发电机"
    },
    {
      "action_type": "set_value",
      "config": {
        "key": "generator.status",
        "value": "starting",
        "ttl": null
      },
      "description": "更新发电机状态"
    },
    {
      "action_type": "notify",
      "config": {
        "level": "warning",
        "message": "电池电量低，已启动发电机",
        "recipients": null
      },
      "description": "发送通知"
    }
  ],
  "enabled": true,
  "priority": 1,
  "cooldown_seconds": 300
}
EOF

# 测试条件不满足的情况
echo "📝 测试条件不满足（电池电量85%）"
./rulesrv test battery_low_start_gen 2>/dev/null || echo "规则尚未加载"

# 修改电池电量
echo ""
echo "🔄 修改电池电量为15%"
redis-cli SET battery.soc 15 > /dev/null

# 测试条件满足的情况
echo ""
echo "📝 测试条件满足（电池电量15%）"
./rulesrv test battery_low_start_gen 2>/dev/null || echo "规则尚未加载"

echo ""
echo "⚡ 测试规则2：电压监控"
echo "===================="
cat > /tmp/voltage_monitor_rule.json << 'EOF'
{
  "id": "voltage_monitor",
  "name": "电压监控",
  "description": "监控电压异常",
  "conditions": {
    "operator": "OR",
    "conditions": [
      {
        "source": "comsrv:1001:T.1",
        "operator": "<",
        "value": 220.0,
        "description": "电压低于220V"
      },
      {
        "source": "comsrv:1001:T.1",
        "operator": ">",
        "value": 240.0,
        "description": "电压高于240V"
      }
    ]
  },
  "actions": [
    {
      "action_type": "publish",
      "config": {
        "channel": "ems:voltage:alert",
        "message": "电压异常检测"
      },
      "description": "发布电压告警"
    },
    {
      "action_type": "set_value",
      "config": {
        "key": "voltage.alert.last",
        "value": "timestamp",
        "ttl": null
      },
      "description": "记录告警时间"
    }
  ],
  "enabled": true,
  "priority": 2,
  "cooldown_seconds": 60
}
EOF

# 测试正常电压
echo "📝 测试正常电压（230.5V）"
./rulesrv test voltage_monitor 2>/dev/null || echo "规则尚未加载"

# 修改电压值
echo ""
echo "🔄 修改电压为245V（超高）"
redis-cli HSET comsrv:1001:T 1 "245.0" > /dev/null

echo ""
echo "📝 测试高电压告警"
./rulesrv test voltage_monitor 2>/dev/null || echo "规则尚未加载"

# 测试规则3：复合条件
echo ""
echo "🔧 测试规则3：复合条件规则"
echo "======================="
cat > /tmp/complex_rule.json << 'EOF'
{
  "id": "complex_condition",
  "name": "复合条件测试",
  "description": "测试复杂的条件组合",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "battery.soc",
        "operator": "<",
        "value": 50.0,
        "description": "电池电量小于50%"
      },
      {
        "source": "battery.temperature",
        "operator": ">",
        "value": 40.0,
        "description": "电池温度高于40℃"
      },
      {
        "source": "generator.fuel",
        "operator": ">",
        "value": 20.0,
        "description": "发电机燃料充足"
      }
    ]
  },
  "actions": [
    {
      "action_type": "notify",
      "config": {
        "level": "critical",
        "message": "电池状态异常，需要立即处理",
        "recipients": ["admin@example.com"]
      },
      "description": "发送紧急通知"
    }
  ],
  "enabled": true,
  "priority": 0,
  "cooldown_seconds": 180
}
EOF

echo "📝 测试复合条件（部分满足）"
echo "  - 电池SOC: 15% ✓"
echo "  - 电池温度: 25.5℃ ✗"  
echo "  - 发电机燃料: 80% ✓"
./rulesrv test complex_condition 2>/dev/null || echo "规则尚未加载"

# 修改温度
echo ""
echo "🔄 修改电池温度为45℃"
redis-cli SET battery.temperature 45 > /dev/null

echo ""
echo "📝 测试复合条件（全部满足）"
echo "  - 电池SOC: 15% ✓"
echo "  - 电池温度: 45℃ ✓"  
echo "  - 发电机燃料: 80% ✓"
./rulesrv test complex_condition 2>/dev/null || echo "规则尚未加载"

# 测试CLI命令
echo ""
echo "🖥️  测试CLI命令"
echo "=============="

# 检查二进制文件
if [ -f "./rulesrv" ]; then
    echo "✅ rulesrv二进制文件存在"
    
    # 列出规则
    echo ""
    echo "📋 列出所有规则:"
    ./rulesrv list || echo "暂无规则"
    
    # 测试特定规则
    echo ""
    echo "🧪 测试特定规则:"
    ./rulesrv test battery_low_start_gen 2>/dev/null || echo "规则不存在"
    
    # 执行规则
    echo ""
    echo "▶️  执行规则:"
    ./rulesrv execute battery_low_start_gen 2>/dev/null || echo "规则不存在"
else
    echo "⚠️  rulesrv二进制文件不存在，请先编译：cargo build --release"
fi

# 清理测试数据
echo ""
echo "🧹 清理测试数据..."
redis-cli --scan --pattern "battery.*" | xargs -L 100 redis-cli DEL 2>/dev/null || true
redis-cli --scan --pattern "generator.*" | xargs -L 100 redis-cli DEL 2>/dev/null || true
redis-cli --scan --pattern "voltage.*" | xargs -L 100 redis-cli DEL 2>/dev/null || true
redis-cli DEL comsrv:1001:T > /dev/null
rm -f /tmp/battery_low_rule.json /tmp/voltage_monitor_rule.json /tmp/complex_rule.json

echo ""
echo "✅ 规则执行测试完成！"