@echo off
REM 在Windows环境中运行modsrv GUI

REM 设置DISPLAY环境变量
set DISPLAY=host.docker.internal:0

REM 构建并运行GUI容器
docker-compose build modsrv-gui
docker-compose run --rm modsrv-gui

echo GUI应用已关闭 