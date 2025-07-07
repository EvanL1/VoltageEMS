#!/bin/bash
# 设置本地Docker Registry

set -e

echo "设置本地Docker Registry..."

# 检查是否已存在
if docker ps | grep -q registry; then
    echo "Registry已在运行"
    exit 0
fi

# 创建Registry数据目录
sudo mkdir -p /opt/docker-registry/data

# 启动Registry
docker run -d \
    -p 5000:5000 \
    --restart=always \
    --name registry \
    -v /opt/docker-registry/data:/var/lib/registry \
    registry:2

# 等待Registry启动
sleep 5

# 测试Registry
curl -s http://localhost:5000/v2/_catalog

echo "本地Docker Registry设置完成！"
echo "Registry地址: localhost:5000"