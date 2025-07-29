#!/bin/bash

# 编译 protobuf 文件
echo "Compiling protobuf files..."

# 创建输出目录
mkdir -p src/proto

# 复制 proto 文件
cp ../../services/comsrv/proto/protocol_plugin.proto ./proto/

# 编译
python -m grpc_tools.protoc \
    -I./proto \
    --python_out=./src/proto \
    --grpc_python_out=./src/proto \
    ./proto/protocol_plugin.proto

# 创建 __init__.py
touch ./src/proto/__init__.py

echo "Protobuf compilation completed!"