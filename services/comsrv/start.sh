#!/bin/bash

# Comsrv 启动脚本

# 设置环境变量
export LOG_DIR="./logs"
export RUST_LOG=info

# 创建日志目录
mkdir -p "$LOG_DIR"

# 默认配置文件
CONFIG_FILE="./config/comsrv.yaml"

# 检查环境变量
if [ -n "$CONFIG_PATH" ]; then
    # 如果CONFIG_PATH是目录，则拼接默认配置文件名
    if [ -d "$CONFIG_PATH" ]; then
        CONFIG_FILE="$CONFIG_PATH/comsrv.yaml"
    else
        CONFIG_FILE="$CONFIG_PATH"
    fi
fi

# 检查命令行参数
if [ "$#" -ge 1 ]; then
    CONFIG_FILE="$1"
fi

# 检查配置文件是否存在
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Configuration file not found: $CONFIG_FILE"
    exit 1
fi

# 启动服务
echo "Starting Comsrv with configuration: $CONFIG_FILE"
./comsrv "$CONFIG_FILE" 