#!/bin/bash
# VoltageEMS 演示环境重启脚本

echo "=== 重启 VoltageEMS 演示环境 ==="

# 先停止
./stop-demo.sh

echo ""
echo "等待 5 秒..."
sleep 5

# 再启动
./start-demo.sh