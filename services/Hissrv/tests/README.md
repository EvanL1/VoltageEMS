# HisSrv 测试指南

## 快速开始

### 1. 运行完整测试
```bash
cd services/hissrv
./run_tests.sh
```

这将自动：
- 启动 Redis 和 InfluxDB 容器
- 生成模拟数据到 Redis
- 启动 HisSrv 服务
- 验证数据是否正确转存到 InfluxDB
- 提供 API 查询示例

### 2. 手动生成测试数据
```bash
# 生成数据 60 秒，每秒更新一次
python3 tests/generate_redis_data.py --duration 60 --interval 1.0

# 指定通道和 Redis 连接
python3 tests/generate_redis_data.py --channels 2001 2002 --host localhost --port 6379

# 清理测试数据
python3 tests/generate_redis_data.py --clear --channels 1001 1002 1003
```

### 3. 运行选项
```bash
# 运行单元测试
./run_tests.sh --unit-tests

# 测试后清理数据
./run_tests.sh --clear

# 停止并删除容器
./run_tests.sh --stop-containers

# 组合选项
./run_tests.sh --unit-tests --clear --stop-containers
```

## 数据格式

### Redis 键格式（扁平化存储）
```
{channelID}:{type}:{pointID}
```

类型映射：
- `m` - 测量值 (telemetry/YC)
- `s` - 信号值 (signal/YX)  
- `c` - 控制值 (control/YK)
- `a` - 调节值 (adjustment/YT)

### 数据示例
```json
// 键: 1001:m:10001
{
  "value": 220.5,
  "quality": 192,
  "timestamp": 1736912345000,
  "source": "redis_generator"
}
```

## 测试验证

### 1. 检查 Redis 数据
```bash
# 查看所有测量点
docker exec redis-hissrv-test redis-cli keys "*:m:*"

# 获取单个点值
docker exec redis-hissrv-test redis-cli get "1001:m:10001"

# 监控 Redis 活动
docker exec redis-hissrv-test redis-cli monitor
```

### 2. 检查 InfluxDB 数据
```bash
# 进入 InfluxDB 控制台
docker exec -it influxdb-hissrv-test influx -database hissrv_test

# 查询数据
> SHOW MEASUREMENTS
> SELECT * FROM "1001:m:10001" LIMIT 10
> SELECT COUNT(*) FROM /.*/
```

### 3. 使用 API 查询
```bash
# 获取最新数据
curl http://localhost:8089/api/v1/data/latest?channels=1001,1002

# 查询历史数据（Linux）
curl "http://localhost:8089/api/v1/data/history?channel_id=1001&start_time=$(date -d '1 hour ago' +%s)&end_time=$(date +%s)"

# 查询历史数据（macOS）
curl "http://localhost:8089/api/v1/data/history?channel_id=1001&start_time=$(date -v-1H +%s)&end_time=$(date +%s)"

# 聚合查询
curl -X POST http://localhost:8089/api/v1/data/aggregate \
  -H "Content-Type: application/json" \
  -d '{
    "channel_id": 1001,
    "point_ids": ["10001", "10002"],
    "start_time": "'$(date -d '1 hour ago' +%s)'",
    "end_time": "'$(date +%s)'",
    "aggregation": "mean",
    "interval": "5m"
  }'
```

### 4. 访问 Web 界面
- Swagger UI: http://localhost:8089/api/v1/swagger-ui
- 健康检查: http://localhost:8089/health
- Prometheus 指标: http://localhost:9091/metrics

## 故障排查

### Redis 连接问题
```bash
# 检查 Redis 是否运行
docker ps | grep redis-hissrv-test

# 测试连接
docker exec redis-hissrv-test redis-cli ping
```

### InfluxDB 连接问题
```bash
# 检查 InfluxDB 是否运行
docker ps | grep influxdb-hissrv-test

# 检查数据库
docker exec influxdb-hissrv-test influx -execute "SHOW DATABASES"
```

### HisSrv 日志
```bash
# 查看实时日志
tail -f logs/hissrv-test.log

# 查看错误
grep ERROR logs/hissrv-test.log
```

## 性能测试

### 高负载测试
```bash
# 生成高频数据（每 0.1 秒更新）
python3 tests/generate_redis_data.py \
  --channels 1001 1002 1003 1004 1005 \
  --interval 0.1 \
  --duration 300
```

### 监控性能指标
访问 http://localhost:9091/metrics 查看：
- `hissrv_messages_processed_total` - 处理的消息总数
- `hissrv_points_written_total` - 写入的数据点总数
- `hissrv_batch_write_duration_seconds` - 批量写入耗时
- `hissrv_storage_errors_total` - 存储错误总数