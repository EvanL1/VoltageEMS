#!/bin/bash
# Docker测试运行脚本

set -e

echo "🐳 rulesrv Docker测试"
echo "===================="
echo

# 清理旧容器
echo "🧹 清理旧容器..."
docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true

# 构建镜像
echo "🔨 构建测试镜像..."
docker-compose -f docker-compose.test.yml build

# 启动服务
echo "🚀 启动测试服务..."
docker-compose -f docker-compose.test.yml up -d redis rulesrv

# 等待服务启动
echo "⏳ 等待服务启动..."
sleep 5

# 检查服务健康状态
echo "🏥 检查服务健康状态..."
if curl -s http://localhost:6003/health | jq .; then
    echo "✅ rulesrv服务正常运行"
else
    echo "❌ rulesrv服务未正常启动"
    docker-compose -f docker-compose.test.yml logs rulesrv
    exit 1
fi

# 加载示例规则
echo ""
echo "📥 加载示例规则..."
for file in examples/*.json; do
    echo "Loading: $file"
    rule_count=$(jq '. | length' "$file")
    echo "  规则数量: $rule_count"
    
    jq -c '.[]' "$file" | while read -r rule; do
        rule_id=$(echo "$rule" | jq -r '.id')
        echo -n "  - $rule_id ... "
        
        response=$(curl -s -X POST http://localhost:6003/rules \
            -H "Content-Type: application/json" \
            -d "{\"rule\": $rule}")
        
        if echo "$response" | jq -e '.data' > /dev/null 2>&1; then
            echo "✓"
        else
            echo "✗"
            echo "$response" | jq .
        fi
    done
done

# 运行测试
echo ""
echo "🧪 运行测试..."
docker-compose -f docker-compose.test.yml run --rm test-runner

# 显示服务日志
echo ""
echo "📋 服务日志："
docker-compose -f docker-compose.test.yml logs --tail=50 rulesrv

# 测试API端点
echo ""
echo "🌐 测试API端点..."

# 列出所有规则
echo "GET /rules"
curl -s http://localhost:6003/rules | jq '.data | length' | xargs -I {} echo "已加载规则数: {}"

# 测试规则执行
echo ""
echo "测试规则执行..."

# 设置测试数据
echo "设置测试数据: battery.soc = 15"
docker exec rulesrv-redis-test redis-cli SET battery.soc 15 > /dev/null

# 执行电池管理规则
echo "执行规则: battery_low_start_generator"
curl -s -X POST http://localhost:6003/rules/battery_low_start_generator/execute \
    -H "Content-Type: application/json" \
    -d '{"context": null}' | jq .

# 获取规则统计
echo ""
echo "获取规则统计..."
curl -s http://localhost:6003/rules/battery_low_start_generator/stats | jq .

# 清理
read -p "是否清理测试环境？(y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🧹 清理测试环境..."
    docker-compose -f docker-compose.test.yml down -v
    echo "✅ 清理完成"
else
    echo "⚠️  测试环境保留，使用以下命令手动清理："
    echo "    docker-compose -f docker-compose.test.yml down -v"
fi

echo ""
echo "✅ Docker测试完成！"