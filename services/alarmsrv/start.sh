#!/bin/bash

# 检查 Redis 是否运行
if ! redis-cli ping > /dev/null 2>&1; then
    echo "错误: Redis 未运行，请先启动 Redis"
    exit 1
fi

# 设置环境变量
export RUST_LOG=info
export ALARM_CONFIG_FILE=alarmsrv.yaml

# 启动服务
echo "启动告警服务..."
cargo run

# 如果退出代码为 1 (端口占用)，尝试查找占用进程
if [ $? -eq 1 ]; then
    echo "端口可能被占用，检查端口 8087..."
    lsof -i :8087
fi