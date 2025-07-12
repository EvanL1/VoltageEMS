#!/bin/bash
# run_plugin_storage_tests.sh - 运行协议插件存储集成测试

set -e

echo "=== 协议插件扁平化存储集成测试 ==="
echo "开始时间: $(date)"
echo ""

# 检查Redis是否运行
if ! redis-cli ping >/dev/null 2>&1; then
    echo "启动Redis容器..."
    docker run -d --name redis-plugin-test -p 6379:6379 redis:7-alpine
    sleep 2
    CLEANUP_REDIS=true
else
    echo "Redis已运行"
    CLEANUP_REDIS=false
fi

# 进入comsrv目录
cd services/comsrv

# 运行测试
echo "运行集成测试..."
cargo test --test plugin_storage_integration_test -- --nocapture

# 检查测试结果
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ 所有测试通过！"
    
    # 显示一些Redis数据样例
    echo ""
    echo "=== Redis数据样例 ==="
    echo "扁平化键格式示例："
    redis-cli --scan --pattern "*:*:*" | head -10
    
    echo ""
    echo "数据值示例："
    redis-cli get "1001:m:10001" || true
    redis-cli get "2001:s:1001" || true
else
    echo ""
    echo "❌ 测试失败"
fi

# 清理
if [ "$CLEANUP_REDIS" = true ]; then
    echo ""
    echo "清理Redis容器..."
    docker stop redis-plugin-test
    docker rm redis-plugin-test
fi

echo ""
echo "结束时间: $(date)"