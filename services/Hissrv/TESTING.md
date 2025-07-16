# HisSrv 测试指南

## 概述

本文档描述了如何测试重构后的 HisSrv 服务（v0.2.0）。HisSrv 是一个简化的 Redis 到 InfluxDB 3.2 数据传输服务。

## 架构

```
Redis (键值存储) -> HisSrv (数据处理) -> InfluxDB 3.2 (历史数据存储)
                              |
                              v
                        REST API (查询/状态)
```

## 快速开始

### 运行完整测试套件
```bash
cd services/hissrv
./run-integration-test.sh
```

这个脚本会自动：
1. 检查依赖 (Redis, InfluxDB, Python)
2. 编译并启动 HisSrv 服务
3. 生成测试数据写入 Redis
4. 验证数据转存和查询功能

### 手动测试步骤

#### 1. 启动依赖服务
```bash
# 启动 Redis
docker run -d --name redis-test -p 6379:6379 redis:7-alpine

# 启动 InfluxDB 3.2
docker run -d --name influxdb-test -p 8086:8086 influxdb:2.7-alpine
```

#### 2. 生成测试数据
```bash
cd services/hissrv
python3 ./test-redis-writer.py
```

#### 3. 运行 HisSrv
```bash
export HISSRV_CONFIG=config/test.yaml
export RUST_LOG=debug
cargo run --release
```

#### 4. 验证数据
```bash
# 查看 Redis 数据
redis-cli keys "*:m:*"

# 检查 HisSrv 健康状态
curl http://localhost:8081/health

# 查看处理统计
curl http://localhost:8081/stats

# 查询数据
curl "http://localhost:8081/query/simple?measurement=telemetry&limit=10"
```

## 单元测试

### 运行所有测试
```bash
cargo test -p hissrv
```

### 运行特定测试
```bash
# 批量写入器测试
cargo test -p hissrv batch_writer_test

# Redis 订阅器测试
cargo test -p hissrv redis_subscriber_test

# 集成测试
cargo test -p hissrv integration_test
```

## 数据格式

### Redis 键格式
```
{channelID}:{type}:{pointID}
```

- `channelID`: 通道ID (u32)
- `type`: 数据类型
  - `m`: 测量数据 (telemetry)
  - `s`: 信号数据 (signal)  
  - `c`: 控制数据 (control)
  - `a`: 调节数据 (adjustment)
- `pointID`: 点ID (u32)

### 点数据格式
```json
{
  "point_id": 10001,
  "value": 123.45,
  "quality": "Good",
  "timestamp": "2025-07-15T10:30:00Z",
  "metadata": null
}
```

## API 端点

### 健康检查
```bash
curl http://localhost:8081/health
```

### 统计信息
```bash
curl http://localhost:8081/stats
```

### 数据查询
```bash
# 简单查询
curl "http://localhost:8081/query/simple?measurement=telemetry&limit=10"

# SQL 查询
curl -X POST http://localhost:8081/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM telemetry ORDER BY time DESC LIMIT 10"}'
```

### 强制刷新
```bash
curl -X POST http://localhost:8081/flush
```

## 故障排查

### 常见问题

1. **Redis 连接失败**
   - 检查 Redis 是否运行：`docker ps | grep redis`
   - 测试连接：`redis-cli ping`

2. **InfluxDB 写入失败**
   - 检查数据库是否存在：`influx -execute "SHOW DATABASES"`
   - 查看 HisSrv 日志：`tail -f logs/hissrv.log`

3. **数据未转存**
   - 检查订阅通道配置是否正确
   - 验证数据格式是否符合要求
   - 查看批量写入统计信息

### 性能测试

高负载测试：
```bash
# 生成高频数据
python3 tests/generate_redis_data.py \
  --channels 1001 1002 1003 1004 1005 \
  --interval 0.1 \
  --duration 300
```

监控性能指标：
- 访问 http://localhost:9090/metrics
- 关注关键指标：
  - `hissrv_messages_processed_total`
  - `hissrv_batch_write_duration_seconds`
  - `hissrv_storage_errors_total`