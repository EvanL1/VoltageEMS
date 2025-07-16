# HisSrv - 历史数据服务

HisSrv 是 VoltageEMS 系统中的历史数据服务，负责将 Redis 中的实时数据转发到 InfluxDB 3.2 进行长期存储。

## 功能特性

- ✅ **Redis Pub/Sub 集成** - 实时监听 Redis 键空间通知
- ✅ **批量写入优化** - 自动批量收集数据点，提高写入效率
- ✅ **灵活的数据模式** - 支持测量(m)、信号(s)、控制(c)、调节(a)等多种数据类型
- ✅ **REST API** - 提供健康检查、统计信息和查询接口
- ✅ **自动重连** - 支持 Redis 和 InfluxDB 连接中断后的自动恢复
- ✅ **配置管理** - 使用 Figment 支持 YAML 配置和环境变量覆盖

## 架构设计

```
Redis (键空间通知) → HisSrv → InfluxDB 3.2
     ↓                  ↓           ↓
  实时数据          数据处理     历史存储
```

### 核心组件

1. **RedisSubscriber** - 监听 Redis 键空间通知
2. **DataProcessor** - 处理和批量收集数据点
3. **InfluxDBClient** - 写入 InfluxDB 3.2
4. **API Server** - 提供 REST API 接口

## 配置

### 配置文件示例 (config/default.yaml)

```yaml
service:
  name: "hissrv"
  version: "0.2.0"
  host: "0.0.0.0"
  port: 8081

redis:
  connection:
    host: "localhost"
    port: 6379
    database: 0
    timeout_seconds: 5
  subscription:
    patterns:
      - "*:m:*"  # 测量数据
      - "*:s:*"  # 信号数据

influxdb:
  enabled: true
  url: "http://localhost:8181"
  database: "voltage_ems"
  batch_size: 1000
  flush_interval_seconds: 10

logging:
  level: "info"
  format: "text"
  file: "./logs/hissrv.log"
```

### 环境变量

支持通过环境变量覆盖配置：

```bash
HISSRV_SERVICE__PORT=8082
HISSRV_REDIS__CONNECTION__HOST=redis.example.com
HISSRV_INFLUXDB__URL=http://influxdb:8086
```

## Redis 配置要求

HisSrv 使用 Redis 键空间通知功能。需要确保 Redis 配置了：

```
notify-keyspace-events KEA
```

或者在 redis.conf 中设置，或者运行时配置：

```bash
redis-cli CONFIG SET notify-keyspace-events KEA
```

HisSrv 启动时会尝试自动配置此选项。

## 数据格式

### Redis 键格式

```
{channelID}:{type}:{pointID}
```

例如：
- `1001:m:10001` - 通道 1001 的测量点 10001
- `2001:s:20001` - 通道 2001 的信号点 20001

### Redis 值格式 (JSON)

```json
{
  "point_id": 10001,
  "value": 23.45,
  "timestamp": "2025-07-16T00:00:00Z",
  "metadata": null
}
```

### InfluxDB 数据点格式

```
telemetry,channel_id=1001,point_id=10001,point_type=m value=23.45 1752627600000000000
```

## API 端点

### 健康检查
```
GET /health
```

响应示例：
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "service": "hissrv",
    "version": "0.2.0",
    "components": {
      "influxdb": {
        "status": "healthy"
      },
      "processor": {
        "status": "healthy"
      }
    }
  }
}
```

### 统计信息
```
GET /stats
```

响应示例：
```json
{
  "success": true,
  "data": {
    "processing": {
      "messages_received": 100,
      "messages_processed": 98,
      "messages_failed": 2,
      "points_written": 98
    },
    "influxdb": {
      "connected": true,
      "database": "voltage_ems",
      "url": "http://localhost:8181"
    },
    "uptime_seconds": 3600
  }
}
```

### 简单查询
```
GET /query/simple?measurement=telemetry&limit=10
```

### SQL 查询
```
POST /query
Content-Type: application/json

{
  "query": "SELECT * FROM telemetry WHERE time > now() - 1h LIMIT 100"
}
```

### 强制刷新
```
POST /flush
```

## 运行

### 本地开发

```bash
# 编译
cargo build -p hissrv

# 运行（开发模式）
RUST_LOG=debug cargo run -p hissrv

# 运行（指定配置文件）
HISSRV_CONFIG=config/custom.yaml cargo run -p hissrv
```

### Docker 运行

```bash
# 构建镜像
docker build -f services/hissrv/Dockerfile -t hissrv:latest .

# 运行容器
docker run -d \
  --name hissrv \
  -p 8081:8081 \
  -e HISSRV_REDIS__CONNECTION__HOST=redis \
  -e HISSRV_INFLUXDB__URL=http://influxdb:8086 \
  -v ./config:/app/config:ro \
  hissrv:latest
```

### Docker Compose

```yaml
services:
  hissrv:
    image: hissrv:latest
    environment:
      - RUST_LOG=info
      - HISSRV_CONFIG=/app/config/production.yaml
    volumes:
      - ./config:/app/config:ro
      - ./logs:/app/logs
    depends_on:
      - redis
      - influxdb
    ports:
      - "8081:8081"
```

## 测试

### 单元测试

```bash
cargo test -p hissrv
```

### 集成测试

运行提供的测试脚本：

```bash
cd services/hissrv
./test-pubsub.sh
```

### 手动测试

1. 写入测试数据到 Redis：
```bash
redis-cli SET "1001:m:10001" '{"point_id":10001,"value":23.45,"timestamp":"2025-07-16T00:00:00Z"}'
```

2. 检查处理统计：
```bash
curl http://localhost:8081/stats
```

3. 查询数据：
```bash
curl "http://localhost:8081/query/simple?measurement=telemetry&limit=10"
```

## 故障排除

### Redis 连接问题

1. 检查 Redis 是否运行：
```bash
redis-cli ping
```

2. 检查键空间通知配置：
```bash
redis-cli CONFIG GET notify-keyspace-events
```

### InfluxDB 写入失败

1. 检查 InfluxDB 是否运行：
```bash
curl http://localhost:8181/health
```

2. 检查日志中的错误信息：
```bash
tail -f logs/hissrv.log
```

### 没有接收到数据

1. 确认 Redis 键格式正确
2. 检查订阅模式是否匹配
3. 查看 debug 日志了解详细信息

## 性能优化

1. **批量大小** - 调整 `batch_size` 以优化写入性能
2. **刷新间隔** - 根据数据量调整 `flush_interval_seconds`
3. **连接池** - Redis 使用连接池以提高性能
4. **日志级别** - 生产环境使用 `info` 或 `warn` 级别

## 已知限制

1. InfluxDB 3.2 需要配置正确的 bucket/database
2. 不支持 Redis Cluster（可以通过配置多个实例解决）
3. 查询功能目前比较基础

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License