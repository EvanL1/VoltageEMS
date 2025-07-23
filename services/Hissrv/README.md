# hissrv - 历史数据服务

## 概述

hissrv 是 VoltageEMS 的历史数据存储服务，负责将 Redis 中的实时数据批量转储到 InfluxDB 进行长期存储。服务采用高效的批处理机制，支持新的 Redis Hash 数据结构，并提供灵活的数据过滤和存储策略。

## 主要特性

- **Hash 结构订阅**: 监听 Redis Hash 数据变化，支持通道级订阅
- **批量写入优化**: 自动批量收集数据点，提高 InfluxDB 写入效率
- **智能数据过滤**: 支持值范围、时间间隔等多种过滤规则
- **标准化精度**: 所有浮点数值保持 6 位小数精度
- **配置热重载**: 支持动态更新点位配置而无需重启服务
- **自动重连机制**: Redis 和 InfluxDB 连接中断后自动恢复

## 快速开始

### 运行服务

```bash
cd services/hissrv
cargo run
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "hissrv"
  host: "0.0.0.0"
  port: 8083
  
redis:
  url: "redis://localhost:6379"
  subscription:
    patterns:
      - "comsrv:*:m"  # 订阅所有测量数据
      - "comsrv:*:s"  # 订阅所有信号数据
      - "modsrv:*:measurement"  # 订阅计算结果
      
influxdb:
  url: "http://localhost:8086"
  database: "voltageems"
  batch_size: 1000
  flush_interval: 10  # 秒
  
logging:
  level: "info"
  file: "logs/hissrv.log"
```

## 数据流架构

```
Redis Hash 结构         hissrv              InfluxDB
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
comsrv:1001:m  ───┐                    ┌─── telemetry
  10001: "25.1" ──┼──► 订阅处理 ───────┼──► channel_id=1001
  10002: "26.2" ──┘    批量收集         └─── point_id=10001
                       时间戳添加             value=25.100000
```

## Redis 订阅机制

### Hash 键监听

hissrv 订阅 Redis Hash 结构的变化：

```rust
// 订阅模式
PSUBSCRIBE __keyspace@0__:comsrv:*:m
PSUBSCRIBE __keyspace@0__:comsrv:*:s
PSUBSCRIBE __keyspace@0__:modsrv:*:measurement
```

### 数据读取流程

1. 接收键空间通知
2. 解析 Hash 键获取通道和类型信息
3. 批量读取 Hash 中的所有字段
4. 添加时间戳并转换为 InfluxDB 格式
5. 批量写入 InfluxDB

## 数据格式转换

### Redis Hash 格式

```
键: comsrv:1001:m
字段值对:
  10001 → "25.123456"
  10002 → "26.789012"
```

### InfluxDB 数据点格式

```
测量名: telemetry
标签:
  - channel_id: "1001"
  - point_id: "10001"
  - point_type: "m"
字段:
  - value: 25.123456
时间戳: 1642592400000000000
```

## 批处理优化

### 批量收集器

```rust
pub struct BatchCollector {
    buffer: Vec<DataPoint>,
    max_size: usize,
    flush_interval: Duration,
}

// 自动批量处理
- 达到批量大小限制时刷新
- 超过时间间隔时刷新
- 手动触发刷新
```

### 性能参数

- `batch_size`: 批量大小（默认 1000）
- `flush_interval`: 刷新间隔（默认 10 秒）
- `buffer_size`: 内部缓冲区大小

## 点位过滤配置

### 配置文件 (`config/points.yml`)

```yaml
enabled: true
default_policy: "allow_all"  # 或 "deny_all"

rules:
  # 通道级规则
  channels:
    - channel_id: 1001
      enabled: true
      point_types: ["m", "s"]
      description: "主站数据"
      
    - channel_id: 2001
      enabled: false
      description: "测试通道，不保存"
      
  # 点位级规则
  points:
    - channel_id: 1001
      point_id: 10099
      enabled: false
      description: "故障点位"

# 数据过滤器
filters:
  - type: "value_range"
    point_types: ["m"]
    min_value: -10000.0
    max_value: 10000.0
    
  - type: "time_interval"
    min_interval_seconds: 5
    description: "防止过于频繁的更新"
```

## API 接口

### 健康检查

```bash
GET /health
```

响应：
```json
{
  "status": "healthy",
  "components": {
    "redis": "connected",
    "influxdb": "connected",
    "processor": "running"
  }
}
```

### 统计信息

```bash
GET /stats
```

响应：
```json
{
  "messages_received": 10000,
  "points_written": 9950,
  "current_batch_size": 150,
  "write_errors": 0,
  "uptime_seconds": 3600
}
```

### 配置管理

```bash
# 获取当前配置
GET /config/points

# 更新配置
PUT /config/points
Content-Type: application/json

# 重载配置
POST /config/reload
```

### 手动刷新

```bash
POST /flush
```

## 监控和调试

### 日志配置

```yaml
logging:
  level: "debug"  # debug, info, warn, error
  format: "json"  # json 或 text
  file: "logs/hissrv.log"
  rotate: true
  max_size: "100MB"
```

### 监控指标

通过 `/metrics` 端点暴露 Prometheus 指标：

- `hissrv_messages_received_total` - 接收消息总数
- `hissrv_points_written_total` - 写入点位总数
- `hissrv_batch_size` - 当前批次大小
- `hissrv_write_duration_seconds` - 写入耗时

## 故障排查

### Redis 连接问题

1. 检查键空间通知配置：
```bash
redis-cli CONFIG GET notify-keyspace-events
# 应该包含 'K' 和 'h'
```

2. 手动设置（如需要）：
```bash
redis-cli CONFIG SET notify-keyspace-events Kh
```

### InfluxDB 写入失败

1. 检查连接：
```bash
curl http://localhost:8086/health
```

2. 查看错误日志：
```bash
tail -f logs/hissrv.log | grep ERROR
```

### 数据未保存

1. 检查点位配置是否允许该数据
2. 验证数据格式是否正确
3. 查看过滤器是否阻止了数据

## 环境变量

- `RUST_LOG` - 日志级别
- `HISSRV_CONFIG` - 配置文件路径
- `HISSRV_REDIS_URL` - Redis 连接地址
- `HISSRV_INFLUXDB_URL` - InfluxDB 连接地址

## 相关文档

- [架构设计](docs/architecture.md)
- [InfluxDB 桥接](docs/influxdb-bridge.md)