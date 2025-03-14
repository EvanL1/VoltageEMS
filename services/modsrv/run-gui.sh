#!/bin/bash

# 确保X11允许连接
xhost +local:docker

# 构建并运行GUI容器
docker-compose build modsrv-gui
docker-compose run --rm modsrv-gui

# 恢复X11安全设置
xhost -local:docker 