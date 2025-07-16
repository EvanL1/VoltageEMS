#!/bin/bash

# HisSrv Docker 测试运行脚本
# 在完全隔离的 Docker 网络环境中运行集成测试

set -e

echo "========================================"
echo "HisSrv Docker 集成测试"
echo "========================================"
echo "测试环境: 完全隔离的 Docker 内部网络"
echo "不暴露任何端口到宿主机"
echo ""

# 创建必要的目录
mkdir -p logs test-results

# 清理旧容器
echo "1. 清理旧容器..."
docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true

# 构建镜像
echo "2. 构建 Docker 镜像..."
docker-compose -f docker-compose.test.yml build

# 启动服务
echo "3. 启动测试环境..."
docker-compose -f docker-compose.test.yml up -d redis influxdb

# 等待基础服务就绪
echo "4. 等待基础服务就绪..."
sleep 10

# 启动 HisSrv
echo "5. 启动 HisSrv 服务..."
docker-compose -f docker-compose.test.yml up -d hissrv

# 等待 HisSrv 就绪
echo "6. 等待 HisSrv 就绪..."
sleep 10

# 运行测试
echo "7. 运行集成测试..."
docker-compose -f docker-compose.test.yml run --rm test-runner

# 获取测试结果
echo "8. 获取测试结果..."
if [ -f test-results/test_report.txt ]; then
    echo ""
    echo "测试报告:"
    echo "========================================="
    cat test-results/test_report.txt
fi

# 收集日志
echo "9. 收集服务日志..."
docker-compose -f docker-compose.test.yml logs hissrv > logs/hissrv-docker.log 2>&1

# 清理
echo "10. 清理测试环境..."
read -p "是否清理测试容器？[y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker-compose -f docker-compose.test.yml down -v
    echo "✓ 测试环境已清理"
else
    echo "容器保留，可使用以下命令查看："
    echo "  docker-compose -f docker-compose.test.yml ps"
    echo "  docker-compose -f docker-compose.test.yml logs"
fi

echo ""
echo "========================================"
echo "测试完成！"
echo "========================================"
echo "日志文件: logs/hissrv-docker.log"
echo "测试结果: test-results/test_report.txt"