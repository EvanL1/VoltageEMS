#!/bin/bash
# 测试 hissrv 功能的脚本

REDIS_CLI=${REDIS_CLI:-redis-cli}

echo "=== hissrv 功能测试 ==="
echo ""

# 1. 测试列表数据（JSON格式）
echo "1. 推送 JSON 测试数据到列表..."
$REDIS_CLI LPUSH archive:pending '{
  "timestamp": 1234567890,
  "measurement": "test_metrics",
  "tags": {
    "source": "test",
    "type": "demo"
  },
  "fields": {
    "value": 123.456,
    "count": 10
  }
}'

# 2. 测试 Hash 数据
echo ""
echo "2. 创建测试 Hash 数据..."
TIMESTAMP=$(date +%s)
TEST_KEY="archive:1m:$TIMESTAMP:1001"

$REDIS_CLI HSET $TEST_KEY voltage_avg "220.123456"
$REDIS_CLI HSET $TEST_KEY voltage_max "230.000000"
$REDIS_CLI HSET $TEST_KEY voltage_min "210.000000"
$REDIS_CLI HSET $TEST_KEY current_avg "15.234567"
$REDIS_CLI HSET $TEST_KEY power_avg "3456.789012"
$REDIS_CLI HSET $TEST_KEY timestamp "$TIMESTAMP"

echo ""
echo "3. 查看创建的数据..."
echo "列表长度: $($REDIS_CLI LLEN archive:pending)"
echo "Hash 内容:"
$REDIS_CLI HGETALL $TEST_KEY

echo ""
echo "测试数据已准备就绪。"
echo "现在可以运行 hissrv 来处理这些数据。"
echo ""
echo "运行命令："
echo "  export INFLUXDB_TOKEN=your_token_here"
echo "  export RUST_LOG=hissrv=debug"
echo "  cargo run --release"