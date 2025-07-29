# HisSrv - 历史数据服务

HisSrv是VoltageEMS系统的历史数据服务，通过Lua脚本聚合Redis实时数据，并将聚合结果持久化到InfluxDB。

## 特性

- **极简设计** - 精简的代码结构，易于维护
- **Lua脚本聚合** - 高性能的数据预聚合处理
- **多级时间窗口** - 支持1分钟、5分钟等多级聚合
- **轮询架构** - 简单可靠的数据采集机制
- **批量写入优化** - 减少InfluxDB写入压力
- **配置管理** - 支持运行时配置修改和热重载

## 架构

```
Redis实时数据 → Lua脚本聚合 → 聚合数据 → HisSrv轮询 → InfluxDB
                    ↓
              Cron定时触发
```

## 快速开始

### 环境要求

- Rust 1.88+
- Redis 7.0+
- InfluxDB 2.x

### 初始化Lua脚本

```bash
# 加载聚合脚本到Redis
cd services/hissrv/scripts
./init_scripts.sh
```

### 定时任务配置

#### 选项1: 容器内置定时器（推荐）

HisSrv可以内置定时器，自动触发Lua脚本：

```yaml
# services/hissrv/config/default.yml
aggregation:
  enabled: true
  intervals:
    - name: "1m"
      interval: 60s
      script: "aggregate_1m"
    - name: "5m"
      interval: 300s
      script: "aggregate_5m"
```

#### 选项2: Docker Compose with Cron

```yaml
version: '3.8'
services:
  hissrv:
    image: voltageems/hissrv
    environment:
      - INFLUXDB_TOKEN=${INFLUXDB_TOKEN}
    depends_on:
      - redis
      - influxdb
    
  hissrv-cron:
    image: voltageems/hissrv
    command: crond -f
    volumes:
      - ./scripts:/scripts
      - type: bind
        source: ./crontab
        target: /etc/crontabs/root
```

crontab文件：
```
* * * * * /scripts/hissrv_cron.sh 1m
*/5 * * * * /scripts/hissrv_cron.sh 5m
```

#### 选项3: Kubernetes CronJob

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: hissrv-1m-aggregation
spec:
  schedule: "* * * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: aggregator
            image: redis:7-alpine
            command:
            - redis-cli
            - EVALSHA
            - $(AGGREGATE_SCRIPT_SHA)
            - "0"
            - "aggregate_1m"
```

### 运行服务

```bash
# 设置环境变量
export INFLUXDB_TOKEN=your_influxdb_token
export RUST_LOG=hissrv=info

# 开发模式
cargo run -p hissrv

# 生产模式
cargo run --release -p hissrv
```

### 配置文件

```yaml
# services/hissrv/config/default.yml
service:
  name: "hissrv"
  polling_interval: 10s    # 轮询间隔
  batch_size: 1000         # 批量写入大小

redis:
  url: "redis://localhost:6379"
  data_patterns:           # 数据源模式
    - "archive:1m:*"      # 1分钟聚合数据
    - "archive:5m:*"      # 5分钟聚合数据
    - "archive:pending"   # 待处理队列

influxdb:
  url: "http://localhost:8086"
  org: "voltage"
  bucket: "ems"
  token: "${INFLUXDB_TOKEN}"
  
api:
  host: "0.0.0.0"
  port: 8082
```

### 数据映射配置

```yaml
# 数据映射规则
mappings:
  - source_pattern: "comsrv:(\\d+):m"    # Redis key模式
    measurement: "telemetry"              # InfluxDB measurement
    tags:
      - name: "channel"
        source: "capture"                 # 从正则捕获组获取
        index: 1
    field_mappings:
      "1": "voltage"                      # 点位ID到字段名映射
      "2": "current"
      "3": "power"
      
  - source_pattern: "modsrv:([^:]+):measurement"
    measurement: "model_data"
    tags:
      - name: "model"
        source: "capture"
        index: 1
    field_mappings:
      "*": "direct"                       # 直接使用原字段名
```

## API接口

### 健康检查

```bash
curl http://localhost:8082/health
```

### 获取服务状态

```bash
curl http://localhost:8082/status
```

响应示例：
```json
{
  "status": "running",
  "processed": 150000,
  "failed": 12,
  "queue_size": 0,
  "last_sync": "2024-01-29T10:30:00Z"
}
```

### 触发手动同步

```bash
curl -X POST http://localhost:8082/sync
```

### 查询历史数据

```bash
# 查询最近1小时的数据
curl "http://localhost:8082/query?measurement=telemetry&channel=1001&range=1h"
```

## 数据流程

1. **原始数据采集**: comsrv写入实时数据到 `comsrv:{channelID}:m`
2. **Lua脚本聚合**: Cron定时触发Lua脚本进行数据聚合
   - 1分钟聚合：计算avg/min/max，存储到 `archive:1m:*`
   - 5分钟聚合：从1分钟数据二次聚合到 `archive:5m:*`
3. **HisSrv轮询**: 定时扫描 `archive:*` 模式的聚合数据
4. **批量写入**: 累积数据批量写入InfluxDB
5. **清理过期数据**: Redis聚合数据自动过期（2小时）

## 性能优化

### 批量写入策略

```yaml
performance:
  batch_size: 1000          # 每批数据量
  batch_timeout: 5s         # 最大等待时间
  write_buffer_size: 10000  # 写入缓冲区大小
  max_retries: 3            # 最大重试次数
```

### Redis扫描优化

- 使用SCAN命令避免阻塞
- 并行处理多个数据源
- 智能跳过无变化数据

### InfluxDB优化

- 合理设置retention policy
- 使用tag indexed fields
- 避免高基数tags

## 监控和调试

### 查看处理日志

```bash
# 详细日志
RUST_LOG=hissrv=debug cargo run

# 查看实时日志
tail -f logs/hissrv.log
```

### Redis监控

```bash
# 查看聚合数据
redis-cli keys "archive:1m:*"
redis-cli hgetall "archive:1m:1704067200:1001"

# 监控Lua脚本执行
redis-cli monitor | grep EVALSHA

# 查看待处理队列
redis-cli llen "archive:pending"
```

### InfluxDB查询

```bash
# 使用influx CLI
influx query 'from(bucket:"ems") 
  |> range(start: -1h) 
  |> filter(fn: (r) => r._measurement == "telemetry")'
```

## 故障排查

### 常见问题

1. **数据未写入InfluxDB**
   - 检查INFLUXDB_TOKEN环境变量
   - 验证InfluxDB连接和权限
   - 查看错误日志

2. **数据延迟**
   - 调整polling_interval
   - 增加batch_size
   - 检查Redis和InfluxDB性能

3. **内存占用高**
   - 减小write_buffer_size
   - 优化数据映射规则
   - 启用数据压缩

## 高级配置

### 数据聚合

```yaml
aggregations:
  - name: "1m_avg"
    interval: 1m
    function: "mean"
    sources: ["telemetry"]
    
  - name: "5m_max"
    interval: 5m
    function: "max"
    sources: ["telemetry"]
```

### 数据过滤

```yaml
filters:
  - source: "comsrv:*:m"
    conditions:
      - field: "quality"
        operator: "eq"
        value: "good"
```

### 告警集成

```yaml
alerts:
  - name: "sync_failure"
    condition: "failed_count > 100"
    action: "webhook"
    url: "http://alert-service/webhook"
```

## 部署建议

### Docker部署

```dockerfile
FROM rust:1.88 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p hissrv

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/hissrv /usr/local/bin/
COPY services/hissrv/config /etc/hissrv
CMD ["hissrv"]
```

### 资源需求

- CPU: 1-2核心
- 内存: 512MB-1GB
- 存储: 取决于日志保留策略

### 高可用部署

- 多实例部署，使用Redis分布式锁
- 负载均衡不同的数据源模式
- 定期备份InfluxDB数据

## 开发指南

### 添加新的数据源

1. 在配置中添加新的pattern
2. 实现相应的映射规则
3. 测试数据流转

### 自定义聚合函数

```rust
// 实现新的聚合函数
impl AggregationFunction {
    pub fn custom_percentile(&self, values: &[f64], p: f64) -> f64 {
        // 实现百分位数计算
    }
}
```

## 测试

```bash
# 单元测试
cargo test -p hissrv

# 集成测试
cargo test -p hissrv --test integration

# 性能测试
cargo bench -p hissrv
```

## 许可证

MIT License