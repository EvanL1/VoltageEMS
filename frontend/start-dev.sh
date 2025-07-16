#\!/bin/bash

# 启动 Vue 开发服务器，忽略系统代理设置
# 这可以避免本地开发时的 502 错误

echo "启动 Vue 开发服务器（忽略系统代理）..."
echo "访问地址: http://localhost:8083"
echo ""

# 清除代理环境变量，然后启动开发服务器
unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY

# 启动开发服务器
npm run serve
EOF < /dev/null