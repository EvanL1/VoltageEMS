#!/bin/bash

# 创建必要的目录
mkdir -p config/points logs

# 如果配置文件不存在，复制默认配置
if [ ! -f config/comsrv/devices.json ]; then
    cp /ems/config/comsrv/devices.json config/comsrv/
fi

# 设置串口设备权限
for port in /dev/ttyUSB*; do
    if [ -e "$port" ]; then
        echo "Setting permissions for $port"
        sudo chmod 666 "$port"
    fi
done

# 构建并启动容器
docker-compose up -d --build

# 显示容器状态
docker-compose ps

# 显示日志
docker-compose logs -f 