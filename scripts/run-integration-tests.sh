#!/bin/bash
# 集成测试脚本

set -e

echo "运行VoltageEMS集成测试..."

# 等待服务就绪
wait_for_service() {
    local service=$1
    local port=$2
    local max_attempts=30
    local attempt=0
    
    echo -n "等待 $service (端口 $port) 就绪..."
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -f -s http://localhost:$port/health >/dev/null 2>&1; then
            echo " ✓"
            return 0
        fi
        echo -n "."
        sleep 1
        ((attempt++))
    done
    
    echo " ✗"
    return 1
}

# 检查所有服务
wait_for_service "API Gateway" 8080
wait_for_service "Frontend" 80
wait_for_service "Redis" 6379

# 运行API测试
echo ""
echo "运行API测试..."
curl -X GET http://localhost:8080/api/v1/health || exit 1
curl -X GET http://localhost:8080/api/v1/channels || exit 1

# 运行前端测试
echo ""
echo "运行前端测试..."
curl -s http://localhost/ | grep -q "VoltageEMS" || exit 1

echo ""
echo "所有集成测试通过！"