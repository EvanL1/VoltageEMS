#!/bin/bash
"""
comsrv 通道监控快速启动脚本
"""

# 检查 Python 是否可用
if command -v python3 &> /dev/null; then
    PYTHON_CMD="python3"
elif command -v python &> /dev/null; then
    PYTHON_CMD="python"
else
    echo "❌ 错误: 没有找到 Python 解释器"
    echo "请安装 Python 3.x"
    exit 1
fi

# 检查 comsrv 是否运行
echo "🔍 检查 comsrv 服务状态..."
if curl -s http://localhost:3001/api/v1/status > /dev/null 2>&1; then
    echo "✅ comsrv 服务正在运行"
else
    echo "❌ comsrv 服务未运行或无法连接"
    echo "请先启动 comsrv 服务："
    echo "  cd services/comsrv && CONFIG_FILE=config/comsrv_test.yaml cargo run --bin comsrv"
    exit 1
fi

# 启动交互式监控
echo "🚀 启动交互式监控界面..."
echo ""
$PYTHON_CMD tools/channel_monitor.py --interactive
