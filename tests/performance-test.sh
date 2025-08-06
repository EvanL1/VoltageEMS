#!/bin/bash

echo "=========================================="
echo "VoltageEMS 性能测试"
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 1. Redis基础操作性能
echo -e "${BLUE}1. Redis基础操作性能测试${NC}"
echo "----------------------------------------"

echo "测试HSET性能 (1000次操作)..."
start_time=$(date +%s%N)
for i in {1..1000}; do
    docker exec redis-test redis-cli HSET "perf:hash" "field$i" "value$i" > /dev/null 2>&1
done
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 1000 * 1000 / $elapsed_ms ))
echo -e "  HSET操作: ${GREEN}$ops_per_sec ops/sec${NC} (耗时: ${elapsed_ms}ms)"

# 2. Lua Functions性能
echo -e "\n${BLUE}2. Lua Functions性能测试${NC}"
echo "----------------------------------------"

echo "测试model_upsert性能 (100次操作)..."
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli FCALL model_upsert 1 "perf_model_$i" "{\"name\":\"Model $i\"}" > /dev/null 2>&1
done
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))
echo -e "  Model操作: ${GREEN}$ops_per_sec ops/sec${NC} (耗时: ${elapsed_ms}ms)"

echo "测试store_alarm性能 (100次操作)..."
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli FCALL store_alarm 1 "perf_alarm_$i" "{\"title\":\"Alarm $i\",\"level\":\"Info\"}" > /dev/null 2>&1
done
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))
echo -e "  Alarm操作: ${GREEN}$ops_per_sec ops/sec${NC} (耗时: ${elapsed_ms}ms)"

# 3. 批量数据读取性能
echo -e "\n${BLUE}3. 批量数据读取性能测试${NC}"
echo "----------------------------------------"

# 准备测试数据
for i in {1..100}; do
    docker exec redis-test redis-cli HSET "perf:batch" "$i" "$i" > /dev/null 2>&1
done

echo "测试HGETALL性能 (100个字段)..."
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli HGETALL "perf:batch" > /dev/null 2>&1
done
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))
echo -e "  批量读取: ${GREEN}$ops_per_sec ops/sec${NC} (耗时: ${elapsed_ms}ms)"

# 4. 并发性能（模拟）
echo -e "\n${BLUE}4. 模拟并发操作${NC}"
echo "----------------------------------------"

echo "并发写入测试 (5个并发进程，每个100次操作)..."
start_time=$(date +%s%N)
for p in {1..5}; do
    (
        for i in {1..100}; do
            docker exec redis-test redis-cli HSET "concurrent:$p" "$i" "$i" > /dev/null 2>&1
        done
    ) &
done
wait
end_time=$(date +%s%N)
elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
total_ops=500
ops_per_sec=$(( $total_ops * 1000 / $elapsed_ms ))
echo -e "  并发写入: ${GREEN}$ops_per_sec ops/sec${NC} (耗时: ${elapsed_ms}ms)"

# 5. 清理测试数据
echo -e "\n清理测试数据..."
docker exec redis-test redis-cli DEL "perf:hash" > /dev/null 2>&1
docker exec redis-test redis-cli DEL "perf:batch" > /dev/null 2>&1
for p in {1..5}; do
    docker exec redis-test redis-cli DEL "concurrent:$p" > /dev/null 2>&1
done
# 清理模型和告警
for i in {1..100}; do
    docker exec redis-test redis-cli FCALL model_delete 1 "perf_model_$i" > /dev/null 2>&1
    docker exec redis-test redis-cli DEL "alarmsrv:perf_alarm_$i" > /dev/null 2>&1
done

# 总结
echo ""
echo "=========================================="
echo "性能测试总结"
echo "=========================================="
echo -e "${GREEN}✅ 所有性能测试完成${NC}"
echo ""
echo "建议性能基准:"
echo "  - Redis HSET: > 100 ops/sec"
echo "  - Lua Functions: > 50 ops/sec"
echo "  - 批量读取: > 50 ops/sec"
echo "  - 并发操作: > 100 ops/sec"
echo ""
echo "注意: 实际性能受Docker exec开销影响，生产环境性能会更高"