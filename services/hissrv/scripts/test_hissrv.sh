#!/bin/bash
# 测试 hissrv 功能的脚本

REDIS_CLI=${REDIS_CLI:-redis-cli}

echo "=== hissrv 功能测试 ==="
echo ""

# 1. 测试 comsrv 数据
echo "1. 创建测试 comsrv 数据..."
$REDIS_CLI HSET comsrv:1001:m "1" "220.123456"
$REDIS_CLI HSET comsrv:1001:m "2" "15.234567"
$REDIS_CLI HSET comsrv:1001:m "3" "3456.789012"
$REDIS_CLI HSET comsrv:1001:m "timestamp" "$(date +%s)"

# 2. 测试 modsrv 数据
echo ""
echo "2. 创建测试 modsrv 数据..."
$REDIS_CLI HSET modsrv:power_meter_1:measurement "voltage_a" "220.5"
$REDIS_CLI HSET modsrv:power_meter_1:measurement "current_a" "15.3"
$REDIS_CLI HSET modsrv:power_meter_1:measurement "power" "3366.5"
$REDIS_CLI HSET modsrv:power_meter_1:measurement "timestamp" "$(date +%s)"

echo ""
echo "3. 查看创建的数据..."
echo "comsrv 数据:"
$REDIS_CLI HGETALL comsrv:1001:m
echo ""
echo "modsrv 数据:"
$REDIS_CLI HGETALL modsrv:power_meter_1:measurement

echo ""
echo "测试数据已准备就绪。"
echo "现在可以运行 hissrv 来处理这些数据。"
echo ""
echo "运行命令："
echo "  export INFLUXDB_TOKEN=your_token_here"
echo "  export RUST_LOG=hissrv=debug"
echo "  cargo run"