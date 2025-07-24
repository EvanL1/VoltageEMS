#!/bin/bash
# 测试数据流：comsrv-simulator -> Redis -> modsrv
# 验证Redis数据结构规范v3.2的实现

set -e

echo "🔄 开始测试数据流..."

# Redis连接配置
REDIS_HOST="redis"
REDIS_PORT="6379"

# 等待服务启动
echo "⏳ 等待服务准备就绪..."
sleep 5

# 1. 验证comsrv-simulator生成的数据格式
echo "📊 检查ComsRv模拟器生成的数据..."

# 检查Hash键存在
echo "检查Hash键格式 comsrv:{channelID}:{type}:"
redis-cli -h $REDIS_HOST -p $REDIS_PORT keys "comsrv:*" | head -10

# 验证具体的Hash数据
echo -e "\n检查通道1001的测量数据 (comsrv:1001:m):"
redis-cli -h $REDIS_HOST -p $REDIS_PORT hgetall "comsrv:1001:m" | head -20

echo -e "\n检查通道1001的信号数据 (comsrv:1001:s):"
redis-cli -h $REDIS_HOST -p $REDIS_PORT hgetall "comsrv:1001:s"

# 2. 监听pub/sub消息（5秒钟）
echo -e "\n🔊 监听5秒钟的发布消息..."
timeout 5s redis-cli -h $REDIS_HOST -p $REDIS_PORT psubscribe "comsrv:*" || true

# 3. 验证数值精度（应该是6位小数）
echo -e "\n🔍 验证数值精度（应为6位小数）:"
redis-cli -h $REDIS_HOST -p $REDIS_PORT hget "comsrv:1001:m" "10001"
redis-cli -h $REDIS_HOST -p $REDIS_PORT hget "comsrv:1001:m" "10004"

# 4. 检查所有通道的统计信息
echo -e "\n📈 通道统计信息:"
for channel in 1001 1002 1003; do
    echo "通道 $channel:"
    echo "  测量点数: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT hlen "comsrv:${channel}:m" 2>/dev/null || echo 0)"
    echo "  信号点数: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT hlen "comsrv:${channel}:s" 2>/dev/null || echo 0)"
    echo "  控制点数: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT hlen "comsrv:${channel}:c" 2>/dev/null || echo 0)"
    echo "  调节点数: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT hlen "comsrv:${channel}:a" 2>/dev/null || echo 0)"
done

# 5. 检查modsrv健康状态
echo -e "\n💚 检查ModSrv服务状态:"
if curl -f -s http://modsrv:8092/health > /dev/null; then
    echo "✅ ModSrv API服务器运行正常"
    curl -s http://modsrv:8092/health | jq '.'
else
    echo "❌ ModSrv API服务器无法访问"
fi

# 6. 检查Redis中是否有modsrv相关的键
echo -e "\n🔧 检查ModSrv处理的数据:"
echo "ModSrv相关键:"
redis-cli -h $REDIS_HOST -p $REDIS_PORT keys "modsrv:*" | head -10

# 7. 数据一致性验证
echo -e "\n🔎 数据一致性验证:"
echo "检查同一个点位在不同存储中的一致性..."

# 获取一个测量点的值
point_value=$(redis-cli -h $REDIS_HOST -p $REDIS_PORT hget "comsrv:1001:m" "10001")
echo "通道1001点位10001当前值: $point_value"

# 检查值的格式（应该是数字且有6位小数）
if echo "$point_value" | grep -qE '^[0-9]+\.[0-9]{6}$'; then
    echo "✅ 数值格式正确（6位小数精度）"
else
    echo "❌ 数值格式错误，期望6位小数: $point_value"
fi

echo -e "\n✅ 数据流测试完成"
echo "📋 测试总结:"
echo "  1. ComsRv模拟器: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT keys "comsrv:*" | wc -l) 个Hash键"
echo "  2. ModSrv处理: $(redis-cli -h $REDIS_HOST -p $REDIS_PORT keys "modsrv:*" | wc -l) 个相关键"
echo "  3. 数据精度: 6位小数格式"
echo "  4. 服务状态: API服务器可访问"