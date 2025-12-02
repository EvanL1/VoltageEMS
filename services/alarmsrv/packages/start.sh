#!/bin/bash

# 告警服务启动脚本 - 简化版
# 适用于aarch64架构的工控机

echo "🚀 启动告警服务..."

# 检查Docker是否安装
if ! command -v docker &> /dev/null; then
    echo "❌ Docker未安装，请先安装Docker"
    exit 1
fi

# 检查Redis连接
echo "🔍 检查Redis连接..."
if command -v redis-cli &> /dev/null; then
    if redis-cli ping > /dev/null 2>&1; then
        echo "✅ 本地Redis连接正常"
    else
        echo "⚠️  本地Redis未启动，请确保Redis服务运行"
    fi
else
    echo "⚠️  未找到redis-cli，请确保Redis已安装并运行"
fi

# 智能选择可用的镜像版本
echo "🔍 查找可用的镜像版本..."

# 查找所有voltageems-alarmsrv镜像
AVAILABLE_IMAGES=$(docker images --format "table {{.Repository}}:{{.Tag}}" | grep "voltageems-alarmsrv" | grep -v "REPOSITORY" | head -10)

if [ -z "$AVAILABLE_IMAGES" ]; then
    echo "❌ 未找到voltageems-alarmsrv镜像"
    echo "💡 请先运行 ./load_image.sh 加载镜像"
    exit 1
fi

echo "📋 可用的镜像版本:"
echo "$AVAILABLE_IMAGES"

# 智能选择镜像优先级：latest > 最新版本号 > 第一个可用的
IMAGE_NAME=""
if echo "$AVAILABLE_IMAGES" | grep -q "voltageems-alarmsrv:latest"; then
    IMAGE_NAME="voltageems-alarmsrv:latest"
    echo "✅ 使用latest版本"
else
    # 尝试找到版本号最高的镜像
    VERSIONED_IMAGES=$(echo "$AVAILABLE_IMAGES" | grep -E "voltageems-alarmsrv:[0-9]+\.[0-9]+\.[0-9]+")
    if [ -n "$VERSIONED_IMAGES" ]; then
        # 按版本号排序，选择最新的
        IMAGE_NAME=$(echo "$VERSIONED_IMAGES" | sort -V -r | head -1)
        echo "✅ 使用最新版本: $IMAGE_NAME"
    else
        # 选择第一个可用的镜像
        IMAGE_NAME=$(echo "$AVAILABLE_IMAGES" | head -1)
        echo "✅ 使用可用镜像: $IMAGE_NAME"
    fi
fi

# 停止现有容器
echo "🛑 停止现有容器..."
docker stop voltageems-alarmsrv 2>/dev/null || true
docker rm voltageems-alarmsrv 2>/dev/null || true

# 创建配置目录
echo "📁 创建配置目录..."
mkdir -p /extp/config
mkdir -p /extp/logs

# 启动服务（使用host网络模式）
echo "🚀 启动告警服务..."
echo "🏷️  使用镜像: $IMAGE_NAME"
docker run -d \
    --name voltageems-alarmsrv \
    --network=host \
    --restart=unless-stopped \
    -v /extp/logs:/app/logs \
    -v /extp/config:/app/config \
    -e REDIS_HOST=localhost \
    -e REDIS_PORT=6379 \
    -e REDIS_DB=0 \
    -e REDIS_PREFIX=alarmsrv: \
    -e DEBUG=false \
    -e LOG_LEVEL=INFO \
    "$IMAGE_NAME"

# 等待服务启动
echo "⏳ 等待服务启动..."
sleep 10

# 检查服务状态（重试机制）
echo "🔍 检查服务状态..."
for i in {1..6}; do
    if curl -f -s http://localhost:6007/health > /dev/null 2>&1; then
        echo "✅ 告警服务启动成功！"
        echo "📱 服务地址: http://localhost:6007"
        echo "📊 健康检查: http://localhost:6007/health"
        echo "📖 API文档: http://localhost:6007/docs"
        break
    else
        if [ $i -eq 6 ]; then
            echo "❌ 服务启动失败，请检查日志"
            echo "💡 提示：服务可能仍在启动中，请稍后手动验证"
            docker logs --tail 20 voltageems-alarmsrv
            exit 1
        else
            echo "⏳ 等待服务响应... ($i/5)"
            sleep 5
        fi
    fi
done

echo "🎉 启动完成！"
echo "🔧 管理命令:"
echo "   查看日志: docker logs voltageems-alarmsrv"
echo "   停止服务: docker stop voltageems-alarmsrv"
echo "   重启服务: docker restart voltageems-alarmsrv"
echo ""
echo "📊 服务数据:"
echo "   数据库路径: /extp/config/voltageems-alarm.db"
echo "   日志路径: /extp/logs/"
echo "   配置挂载: /extp/config/ -> /app/config/"
