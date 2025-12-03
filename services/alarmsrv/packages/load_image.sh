#!/bin/bash

# Docker镜像加载脚本
# 用于在工控机上加载预构建的aarch64镜像

echo "📦 加载Docker镜像..."

# 检查镜像文件是否存在
IMAGE_FILES=$(ls voltage-alarmsrv-*.tar.gz 2>/dev/null | wc -l)
if [ "$IMAGE_FILES" -eq 0 ]; then
    echo "❌ 镜像文件 voltage-alarmsrv-*.tar.gz 不存在"
    echo "请确保镜像文件在当前目录中"
    exit 1
fi

# 检查Docker是否安装
if ! command -v docker &> /dev/null; then
    echo "❌ Docker未安装，请先安装Docker"
    exit 1
fi

# 停止并删除现有容器
echo "🛑 停止并删除现有容器..."
docker stop $(docker ps -q --filter "name=voltage-alarmsrv") 2>/dev/null || true
docker rm $(docker ps -aq --filter "name=voltage-alarmsrv") 2>/dev/null || true

# 删除现有镜像
echo "🗑️  删除现有镜像..."
docker rmi $(docker images -q "voltage-alarmsrv*") 2>/dev/null || true

# 查找镜像文件
IMAGE_FILE=$(ls voltage-alarmsrv-*.tar.gz | head -1)
if [ -z "$IMAGE_FILE" ]; then
    echo "❌ 未找到voltage-alarmsrv镜像文件"
    exit 1
fi

echo "📁 找到镜像文件: $IMAGE_FILE"

# 加载镜像
echo "🔄 正在加载镜像..."
docker load < "$IMAGE_FILE"

# 检查镜像是否加载成功
if docker images | grep -q "voltage-alarmsrv"; then
    echo "✅ 镜像加载成功！"
    echo "📋 可用镜像:"
    docker images | grep voltage-alarmsrv
    
    # 自动为最新加载的镜像创建latest标签
    echo "🏷️  创建latest标签..."
    LATEST_IMAGE=$(docker images --format "{{.Repository}}:{{.Tag}}" | grep "voltage-alarmsrv" | grep -v latest | head -1)
    if [ -n "$LATEST_IMAGE" ]; then
        docker tag "$LATEST_IMAGE" "voltage-alarmsrv:latest"
        echo "✅ 已创建latest标签: $LATEST_IMAGE -> voltage-alarmsrv:latest"
		
        # 删除原版本标签，只保留latest
        echo "🗑️  删除原版本标签: $LATEST_IMAGE"
        docker rmi "$LATEST_IMAGE" 2>/dev/null || true
        echo "✅ 已删除原版本标签，只保留latest镜像"
    fi
else
    echo "❌ 镜像加载失败"
    exit 1
fi

echo "🧹 清理悬空镜像..."
docker rmi $(docker images -f "dangling=true" -q) 2>/dev/null || true

echo "🎉 镜像加载完成！"
echo "🚀 现在可以使用 ./start.sh 启动服务了"
