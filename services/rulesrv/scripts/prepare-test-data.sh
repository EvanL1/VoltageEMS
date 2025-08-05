#!/bin/bash
# 准备测试数据脚本

REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"
REDIS_CLI="redis-cli -h $REDIS_HOST -p $REDIS_PORT"

echo "📊 准备rulesrv测试数据"
echo "====================="
echo "Redis: $REDIS_HOST:$REDIS_PORT"
echo

# 清理旧数据
echo "🧹 清理旧数据..."
$REDIS_CLI --scan --pattern "battery.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "generator.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "system.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "voltage.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "transformer.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "comsrv:*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "device.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "alarm.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "grid.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true
$REDIS_CLI --scan --pattern "ups.*" | xargs -L 100 $REDIS_CLI DEL 2>/dev/null || true

echo "✅ 旧数据已清理"
echo

# 创建电池数据
echo "🔋 创建电池数据..."
$REDIS_CLI SET battery.soc 75 > /dev/null
$REDIS_CLI SET battery.voltage 48.5 > /dev/null
$REDIS_CLI SET battery.current 15.2 > /dev/null
$REDIS_CLI SET battery.temperature 28.5 > /dev/null
$REDIS_CLI SET battery.cycles 450 > /dev/null
$REDIS_CLI SET battery.days_in_service 180 > /dev/null
echo "  - SOC: 75%"
echo "  - 电压: 48.5V"
echo "  - 电流: 15.2A"
echo "  - 温度: 28.5°C"
echo "  - 循环次数: 450"
echo "  - 使用天数: 180"

# 创建发电机数据
echo ""
echo "⚡ 创建发电机数据..."
$REDIS_CLI SET generator.status "stopped" > /dev/null
$REDIS_CLI SET generator.fuel 85 > /dev/null
$REDIS_CLI SET generator.temperature 35 > /dev/null
$REDIS_CLI SET generator.running_hours 1250 > /dev/null
echo "  - 状态: stopped"
echo "  - 燃料: 85%"
echo "  - 温度: 35°C"
echo "  - 运行时间: 1250小时"

# 创建电压数据（模拟comsrv格式）
echo ""
echo "⚡ 创建电压数据..."
# 三相电压
$REDIS_CLI HSET comsrv:1001:T 1 "230.5" > /dev/null  # A相电压
$REDIS_CLI HSET comsrv:1001:T 2 "231.2" > /dev/null  # B相电压
$REDIS_CLI HSET comsrv:1001:T 3 "229.8" > /dev/null  # C相电压
$REDIS_CLI HSET comsrv:1001:T 4 "15.5" > /dev/null   # A相电流
$REDIS_CLI HSET comsrv:1001:T 5 "16.2" > /dev/null   # B相电流
$REDIS_CLI HSET comsrv:1001:T 6 "15.8" > /dev/null   # C相电流
echo "  - A相电压: 230.5V"
echo "  - B相电压: 231.2V"
echo "  - C相电压: 229.8V"
echo "  - A相电流: 15.5A"
echo "  - B相电流: 16.2A"
echo "  - C相电流: 15.8A"

# 创建系统数据
echo ""
echo "🖥️  创建系统数据..."
$REDIS_CLI SET system.load_rate 65 > /dev/null
$REDIS_CLI SET system.grid_connected "true" > /dev/null
$REDIS_CLI SET system.temperature 42 > /dev/null
$REDIS_CLI SET system.pressure 125 > /dev/null
$REDIS_CLI SET system.flow_rate 48 > /dev/null
$REDIS_CLI SET system.status "running" > /dev/null
echo "  - 负载率: 65%"
echo "  - 并网状态: true"
echo "  - 系统温度: 42°C"
echo "  - 系统压力: 125"
echo "  - 流量: 48"
echo "  - 状态: running"

# 创建变压器数据
echo ""
echo "🔌 创建变压器数据..."
$REDIS_CLI SET transformer.temperature 55 > /dev/null
$REDIS_CLI SET transformer.load_rate 72 > /dev/null
$REDIS_CLI SET transformer.oil_level 95 > /dev/null
echo "  - 温度: 55°C"
echo "  - 负载率: 72%"
echo "  - 油位: 95%"

# 创建传感器数据
echo ""
echo "📡 创建传感器数据..."
$REDIS_CLI HSET sensor:readings temp_sensor_1 "78.5" > /dev/null
$REDIS_CLI HSET sensor:readings temp_sensor_2 "65.2" > /dev/null
$REDIS_CLI HSET sensor:readings humidity_sensor_1 "45.8" > /dev/null
echo "  - 温度传感器1: 78.5°C"
echo "  - 温度传感器2: 65.2°C"
echo "  - 湿度传感器1: 45.8%"

# 创建电压质量数据
echo ""
echo "📊 创建电压质量数据..."
$REDIS_CLI SET voltage.imbalance_rate 1.5 > /dev/null
$REDIS_CLI SET voltage.fluctuation_rate 2.3 > /dev/null
$REDIS_CLI SET voltage.fluctuation_frequency 3 > /dev/null
$REDIS_CLI SET voltage.alert.active "false" > /dev/null
echo "  - 不平衡率: 1.5%"
echo "  - 波动率: 2.3%"
echo "  - 波动频率: 3次/10分钟"
echo "  - 告警激活: false"

# 创建设备通信数据
echo ""
echo "🌐 创建设备通信数据..."
$REDIS_CLI SET device.communication.status "online" > /dev/null
$REDIS_CLI SET device.communication.timeout_count 0 > /dev/null
$REDIS_CLI SET device.days_since_maintenance 45 > /dev/null
$REDIS_CLI SET device.running_hours 1800 > /dev/null
echo "  - 通信状态: online"
echo "  - 超时计数: 0"
echo "  - 距上次维护: 45天"
echo "  - 运行时间: 1800小时"

# 创建告警统计数据
echo ""
echo "🚨 创建告警统计数据..."
$REDIS_CLI SET alarm.count.last_minute 2 > /dev/null
$REDIS_CLI SET alarm.unique_types 2 > /dev/null
echo "  - 最近一分钟告警数: 2"
echo "  - 告警类型数: 2"

# 创建电网和UPS数据
echo ""
echo "🔌 创建电网和UPS数据..."
$REDIS_CLI SET grid.power.status "normal" > /dev/null
$REDIS_CLI SET ups.battery.soc 95 > /dev/null
echo "  - 电网状态: normal"
echo "  - UPS电池: 95%"

# 创建逆变器数据
echo ""
echo "🔄 创建逆变器数据..."
$REDIS_CLI SET inverter.temperature 58 > /dev/null
$REDIS_CLI SET inverter.efficiency 96.5 > /dev/null
echo "  - 逆变器温度: 58°C"
echo "  - 逆变器效率: 96.5%"

# 显示数据汇总
echo ""
echo "📊 数据准备完成！"
echo "================"
echo "总计创建了以下类型的数据："
echo "  - 电池数据: 6项"
echo "  - 发电机数据: 4项"
echo "  - 电压数据: 6项"
echo "  - 系统数据: 6项"
echo "  - 变压器数据: 3项"
echo "  - 传感器数据: 3项"
echo "  - 电压质量数据: 4项"
echo "  - 设备通信数据: 4项"
echo "  - 告警统计数据: 2项"
echo "  - 电网/UPS数据: 2项"
echo "  - 逆变器数据: 2项"
echo ""
echo "✅ 测试数据准备完成！"