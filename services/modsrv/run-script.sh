#!/bin/bash

# 设置运行环境
export RUST_LOG=debug

# 检查是否有输入参数
if [ $# -eq 0 ]; then
  # 没有参数，默认以服务模式运行
  echo "Running ModSrv in service mode with local config..."
  cargo run -- --config config/local-config.yaml service
else
  # 有参数，执行相应的命令
  echo "Running ModSrv with command: $@"
  cargo run -- --config config/local-config.yaml "$@"
fi 