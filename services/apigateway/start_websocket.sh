#!/bin/bash

# API网关WebSocket功能测试脚本
# 用于本地开发和测试

echo "🚀 启动API网关WebSocket功能..."

# 检查Python版本
PYTHON_VERSION=$(python3 --version 2>&1 | grep -oP '\d+\.\d+' | head -1)
if [ -z "$PYTHON_VERSION" ]; then
    echo "❌ 未找到Python3，请先安装Python 3.10+"
    exit 1
fi

echo "✅ Python版本检查通过: $PYTHON_VERSION"

# 检查Redis连接
echo "🔍 检查Redis连接..."
if command -v redis-cli &> /dev/null; then
    if redis-cli ping > /dev/null 2>&1; then
        echo "✅ Redis连接成功: localhost:6379"
    else
        echo "⚠️  Redis未启动，请先启动Redis服务"
        echo "   启动命令: sudo systemctl start redis 或 redis-server"
    fi
else
    echo "⚠️  未找到redis-cli，请确保Redis已安装并运行"
fi

# 创建虚拟环境
echo "🔧 创建虚拟环境..."
if [ ! -d "venv" ]; then
    python3 -m venv venv
fi

# 激活虚拟环境
echo "🔧 激活虚拟环境..."
source venv/bin/activate

# 安装依赖
echo "📦 安装依赖..."
pip install -r requirements.txt

# 创建日志目录
mkdir -p logs

# 复制环境配置
if [ ! -f ".env" ]; then
    cp env.development .env
    echo "📋 已创建开发环境配置文件 .env"
fi

# 运行WebSocket功能测试
echo "🧪 运行WebSocket功能测试..."
python3 test_websocket.py

# 启动API网关
echo "🌟 启动API网关WebSocket服务..."
echo "   访问地址: http://localhost:6005"
echo "   API文档: http://localhost:6005/docs"
echo "   健康检查: http://localhost:6005/health"
echo "   WebSocket: ws://localhost:6005/ws"
echo "   WebSocket状态: http://localhost:6005/websocket/status"
echo "📱 WebSocket客户端测试工具:"
echo "   打开 websocket_client_test.html 文件进行测试"
echo "按 Ctrl+C 停止服务"

python3 main.py
