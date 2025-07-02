#!/bin/bash

echo "🔧 VoltageEMS Grafana 连接修复工具"
echo "=================================="

# 设置代理绕过
export NO_PROXY=localhost,127.0.0.1,::1,0.0.0.0
export HTTP_PROXY=
export HTTPS_PROXY=
export http_proxy=
export https_proxy=

echo "✅ 已设置代理绕过"

# 检查端口占用
echo "🔍 检查端口状态..."
echo "端口 3050 (Grafana):"
lsof -i :3050 | head -3

echo "端口 8082 (前端):"
lsof -i :8082 | head -3

# 测试连接
echo ""
echo "🌐 测试连接..."

echo -n "Grafana 直连: "
if curl -s --connect-timeout 5 --no-proxy "*" -o /dev/null http://localhost:3050/; then
    echo "✅ 成功"
else
    echo "❌ 失败"
fi

echo -n "前端服务: "
if curl -s --connect-timeout 5 --no-proxy "*" -o /dev/null http://localhost:8082/; then
    echo "✅ 成功"
else
    echo "❌ 失败"
fi

echo -n "代理路径: "
if curl -s --connect-timeout 5 --no-proxy "*" -o /dev/null "http://localhost:8082/grafana/d-solo/simple-view?orgId=1&panelId=1"; then
    echo "✅ 成功"
else
    echo "❌ 失败"
fi

echo ""
echo "💡 解决方案:"
echo "1. 如果连接失败，请在浏览器中设置代理绕过:"
echo "   - Chrome: 启动时添加 --no-proxy-server 参数"
echo "   - 系统设置: 在代理设置中添加 localhost,127.0.0.1 到绕过列表"

echo ""
echo "2. 或者使用以下命令启动 Chrome（绕过代理）:"
echo "   open -a 'Google Chrome' --args --disable-web-security --user-data-dir=/tmp/chrome_dev --no-proxy-server"

echo ""
echo "3. 访问地址:"
echo "   主前端: http://localhost:8082/"
echo "   测试页面: file://$(pwd)/embedded-simple.html"

echo ""
echo "4. 如果仍然有问题，请尝试重启服务:"
echo "   ./start-services.sh"

echo ""
echo "🎯 快速测试: 在新的终端窗口运行:"
echo "   NO_PROXY='*' curl http://localhost:8082/grafana/d-solo/simple-view?orgId=1\\&panelId=1"