#!/bin/bash

# Docker镜像构建脚本
# 生成aarch64格式的Docker镜像文件

echo "🔨 构建Docker镜像..."

# 检查Docker是否安装
if ! command -v docker &> /dev/null; then
    echo "❌ Docker未安装，请先安装Docker"
    exit 1
fi

# 检查是否在packages目录中
if [ ! -f "../app/core/config.py" ]; then
    echo "❌ 请在packages目录中运行此脚本"
    exit 1
fi

# 从config.py读取版本号
VERSION=$(grep 'APP_VERSION.*=' ../app/core/config.py | grep -o '"[^"]*"' | head -1 | tr -d '"')
if [ -z "$VERSION" ]; then
    VERSION="1.0.0"
    echo "⚠️  无法从config.py读取版本号，使用默认版本: $VERSION"
else
    echo "✅ 从config.py读取到版本号: $VERSION"
fi

IMAGE_NAME="voltageems-alarmsrv:${VERSION}"
FILE_NAME="voltageems-alarmsrv-${VERSION}.tar.gz"

echo "📋 构建信息:"
echo "   镜像名称: ${IMAGE_NAME}"
echo "   输出文件: ${FILE_NAME}"
echo "   目标架构: aarch64"
echo "   Dockerfile路径: ../Dockerfile"

# 构建镜像（指定上级目录的Dockerfile）
echo "🔨 开始构建镜像..."
docker build --platform linux/arm64 -f ../Dockerfile -t ${IMAGE_NAME} ..

if [ $? -eq 0 ]; then
    echo "✅ 镜像构建成功！"
else
    echo "❌ 镜像构建失败"
    exit 1
fi

# 保存镜像
echo "💾 保存镜像到文件..."
docker save ${IMAGE_NAME} | gzip > ${FILE_NAME}

if [ $? -eq 0 ]; then
    echo "✅ 镜像保存成功！"
    echo "📁 文件位置: ${FILE_NAME}"
    echo "📊 文件大小: $(du -h ${FILE_NAME} | cut -f1)"
else
    echo "❌ 镜像保存失败"
    exit 1
fi

echo "🎉 构建完成！"
echo "📦 现在可以将 ${FILE_NAME} 传输到工控机，使用 ./load_image.sh 加载"
