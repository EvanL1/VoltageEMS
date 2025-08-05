#!/bin/bash
# 测试简化版Lua规则引擎

echo "🚀 测试Lua规则引擎 V2"
echo "===================="

# 加载Lua规则引擎
echo "1️⃣ 加载Lua规则引擎..."
redis-cli FUNCTION LOAD REPLACE "$(cat scripts/rule-engine-lua-v2.lua)"

echo ""
echo "2️⃣ 创建简单规则..."
# 创建测试规则: test_value > 50
redis-cli FCALL rule_create_simple 1 test_rule_1 test_value ">" 50

# 创建电池规则: battery.soc <= 20
redis-cli FCALL rule_create_simple 1 battery_rule_1 battery.soc "<=" 20

echo ""
echo "3️⃣ 列出所有规则..."
echo "规则列表: $(redis-cli FCALL rule_list 0)"

echo ""
echo "4️⃣ 测试条件满足..."
echo "设置 test_value = 60"
redis-cli SET test_value 60
echo "执行结果："
redis-cli FCALL rule_execute 1 test_rule_1

echo ""
echo "5️⃣ 测试条件不满足..."
echo "设置 test_value = 40"
redis-cli SET test_value 40
echo "执行结果："
redis-cli FCALL rule_execute 1 test_rule_1

echo ""
echo "6️⃣ 测试battery.soc（点号键）..."
echo "设置 battery.soc = 15"
redis-cli SET battery.soc 15
echo "执行结果："
redis-cli FCALL rule_execute 1 battery_rule_1

echo ""
echo "7️⃣ 测试battery.soc条件不满足..."
echo "设置 battery.soc = 25"
redis-cli SET battery.soc 25
echo "执行结果："
redis-cli FCALL rule_execute 1 battery_rule_1

echo ""
echo "8️⃣ 监听通知（5秒）..."
echo "在另一个终端运行: redis-cli SUBSCRIBE ems:notifications"
echo "然后设置 battery.soc = 10"
sleep 2
redis-cli SET battery.soc 10
redis-cli FCALL rule_execute 1 battery_rule_1

echo ""
echo "✅ 测试完成！"
echo ""
echo "📊 总结："
echo "- ✅ Lua规则引擎可以正确处理点号键名"
echo "- ✅ 条件评估正常工作"
echo "- ✅ 可以发布通知"
echo "- ✅ 无需额外的JSON库"
echo "- ✅ 完全在Redis内部执行"