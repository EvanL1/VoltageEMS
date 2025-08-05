#!/bin/bash
# 加载测试规则脚本

BASE_URL="${BASE_URL:-http://localhost:6003}"
API_URL="$BASE_URL/api/v1"
EXAMPLES_DIR="${EXAMPLES_DIR:-./examples}"

echo "📥 加载测试规则到rulesrv"
echo "======================="
echo "API URL: $API_URL"
echo "Examples: $EXAMPLES_DIR"
echo

# 检查服务是否运行
echo "🏥 检查服务状态..."
if ! curl -s "$BASE_URL/health" > /dev/null; then
    echo "❌ rulesrv服务未运行"
    echo "   请先启动服务: ./rulesrv service"
    exit 1
fi
echo "✅ 服务正常运行"
echo

# 统计信息
total_rules=0
loaded_rules=0
failed_rules=0

# 加载规则文件
for rule_file in "$EXAMPLES_DIR"/*.json; do
    if [ ! -f "$rule_file" ]; then
        continue
    fi
    
    filename=$(basename "$rule_file")
    echo "📄 处理文件: $filename"
    
    # 计算规则数量
    rule_count=$(jq '. | length' "$rule_file" 2>/dev/null || echo 0)
    total_rules=$((total_rules + rule_count))
    
    echo "  规则数量: $rule_count"
    
    # 逐个加载规则
    jq -c '.[]' "$rule_file" 2>/dev/null | while read -r rule; do
        rule_id=$(echo "$rule" | jq -r '.id' 2>/dev/null || echo "unknown")
        rule_name=$(echo "$rule" | jq -r '.name' 2>/dev/null || echo "unknown")
        
        echo -n "  - [$rule_id] $rule_name ... "
        
        # 发送创建规则请求
        response=$(curl -s -X POST "$API_URL/rules" \
            -H "Content-Type: application/json" \
            -d "{\"rule\": $rule}" 2>/dev/null)
        
        # 检查响应
        if echo "$response" | jq -e '.data' > /dev/null 2>&1; then
            echo "✅"
            loaded_rules=$((loaded_rules + 1))
        else
            echo "❌"
            error_msg=$(echo "$response" | jq -r '.error.message' 2>/dev/null || echo "Unknown error")
            echo "    错误: $error_msg"
            failed_rules=$((failed_rules + 1))
        fi
    done
    
    echo
done

# 获取当前规则列表
echo "📋 获取当前规则列表..."
current_rules=$(curl -s "$API_URL/rules" | jq -r '.data[]' 2>/dev/null)
current_count=$(echo "$current_rules" | jq -s 'length' 2>/dev/null || echo 0)

echo "当前系统中的规则数: $current_count"
echo

# 显示加载的规则
if [ "$current_count" -gt 0 ]; then
    echo "已加载的规则:"
    echo "$current_rules" | jq -r '. | "  - [\(.id)] \(.name) (优先级: \(.priority), 状态: \(if .enabled then "启用" else "禁用" end))"' 2>/dev/null
fi

# 汇总统计
echo ""
echo "📊 加载统计"
echo "=========="
echo "总规则数: $total_rules"
echo "成功加载: $loaded_rules"
echo "加载失败: $failed_rules"
echo "系统规则数: $current_count"

# 测试规则执行
echo ""
echo "🧪 测试规则执行..."
echo "=================="

# 找一个可执行的规则进行测试
test_rule_id=$(echo "$current_rules" | jq -r 'select(.enabled == true) | .id' 2>/dev/null | head -1)

if [ -n "$test_rule_id" ]; then
    echo "测试规则: $test_rule_id"
    
    # 执行规则
    echo -n "执行规则... "
    exec_response=$(curl -s -X POST "$API_URL/rules/$test_rule_id/execute" \
        -H "Content-Type: application/json" \
        -d '{"context": null}' 2>/dev/null)
    
    if echo "$exec_response" | jq -e '.data' > /dev/null 2>&1; then
        echo "✅"
        
        # 显示执行结果
        echo "$exec_response" | jq '{
            rule_id: .data.rule_id,
            conditions_met: .data.conditions_met,
            success: .data.success,
            actions_executed: .data.actions_executed | length,
            duration_ms: .data.duration_ms
        }' 2>/dev/null
        
        # 获取规则统计
        echo ""
        echo "规则统计:"
        stats_response=$(curl -s "$API_URL/rules/$test_rule_id/stats" 2>/dev/null)
        echo "$stats_response" | jq '.data' 2>/dev/null
    else
        echo "❌"
        echo "$exec_response" | jq '.error' 2>/dev/null
    fi
else
    echo "⚠️  没有可测试的启用规则"
fi

echo ""
echo "✅ 规则加载完成！"