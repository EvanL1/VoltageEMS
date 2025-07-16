#!/bin/bash

echo "=========================================="
echo "HisSrv Docker 本地测试"
echo "=========================================="
echo ""

# 切换到 hissrv 目录
cd "$(dirname "$0")"

# 1. 清理旧容器
echo "1. 清理旧容器..."
docker compose -f docker-compose.test.yml down -v

# 2. 创建内部网络
echo "2. 创建内部网络..."
docker network create hissrv-test-network --internal || true

# 3. 启动 Redis
echo "3. 启动 Redis..."
docker run -d \
    --name hissrv-redis \
    --network hissrv-test-network \
    redis:7-alpine \
    redis-server --appendonly yes --notify-keyspace-events KEA

# 4. 启动 InfluxDB
echo "4. 启动 InfluxDB..."
docker run -d \
    --name hissrv-influxdb \
    --network hissrv-test-network \
    influxdb:3.2-core \
    serve \
    --object-store file \
    --data-dir /var/lib/influxdb3 \
    --node-id test-node \
    --without-auth

# 5. 等待服务就绪
echo "5. 等待服务就绪..."
sleep 10

# 6. 启动 HisSrv
echo "6. 启动 HisSrv..."
docker run -d \
    --name hissrv \
    --network hissrv-test-network \
    -e RUST_LOG=debug \
    -e HISSRV_CONFIG=/app/config/docker.yaml \
    -e NO_PROXY=localhost,127.0.0.1,redis,influxdb \
    -v ./config:/app/config:ro \
    -v ./logs:/app/logs \
    hissrv:test

# 7. 等待 HisSrv 启动
echo "7. 等待 HisSrv 启动..."
sleep 5

# 8. 查看容器状态
echo "8. 容器状态："
docker ps -a | grep hissrv

# 9. 查看 HisSrv 日志
echo ""
echo "9. HisSrv 日志："
docker logs hissrv --tail 20

# 10. 运行测试
echo ""
echo "10. 运行集成测试..."
docker run --rm \
    --name hissrv-test-runner \
    --network hissrv-test-network \
    -v ./tests:/tests:ro \
    -v ./test-results:/results \
    -e REDIS_HOST=hissrv-redis \
    -e INFLUX_HOST=hissrv-influxdb \
    -e HISSRV_HOST=hissrv \
    alpine:3.19 \
    sh -c "
        apk add --no-cache python3 py3-pip py3-redis py3-requests &&
        python3 /tests/docker-integration-test.py &&
        echo '测试完成！'
    "

# 11. 清理
echo ""
echo "11. 清理测试环境..."
docker stop hissrv-redis hissrv-influxdb hissrv hissrv-test-runner 2>/dev/null || true
docker rm hissrv-redis hissrv-influxdb hissrv hissrv-test-runner 2>/dev/null || true
docker network rm hissrv-test-network 2>/dev/null || true

echo ""
echo "测试完成！查看 test-results 目录获取详细结果。"