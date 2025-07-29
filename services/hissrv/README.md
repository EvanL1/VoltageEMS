# hissrv - 极简历史数据归档服务

专为边端设备设计的轻量级 Redis 到 InfluxDB 数据桥接服务。

## 特性

- **极简设计**：只有 5 个源文件，代码量减少 70%
- **轮询模式**：简单可靠，无复杂的订阅机制
- **配置驱动**：灵活的数据映射规则
- **批量写入**：优化 InfluxDB 写入性能
- **配置管理**：支持运行时配置修改和热重载

## 架构

```
Redis → hissrv (轮询) → InfluxDB
```

## 快速开始

### 1. 配置文件

编辑 `config/hissrv.yaml`：

```yaml
service:
  name: "hissrv"
  polling_interval: 10s

redis:
  url: "redis://localhost:6379"
  data_keys:
    - pattern: "archive:pending"
      type: "list"
    - pattern: "archive:1m:*"
      type: "hash"

influxdb:
  url: "http://localhost:8086"
  org: "voltage"
  bucket: "ems"
  token: "${INFLUXDB_TOKEN}"
```

### 2. 运行服务

```bash
# 设置环境变量
export INFLUXDB_TOKEN=your_token_here
export RUST_LOG=hissrv=info

# 运行
cargo run --release
```

## 数据流

1. **原始数据**：ComsRv 写入 `comsrv:1001:m`
2. **Lua 聚合**：定时聚合到 `archive:1m:*`
3. **hissrv 轮询**：读取聚合数据
4. **批量写入**：写入 InfluxDB

## 配置说明

### Redis 数据源

- `list` 类型：从列表中获取 JSON 数据
- `hash` 类型：从 Hash 中获取键值对

### 数据映射

```yaml
mappings:
  - source: "archive:1m:*"
    measurement: "metrics_1m"
    tags:
      - type: "extract"
        field: "channel"
      - type: "static"
        value: "interval=1m"
    fields:
      - name: "voltage_avg"
        field_type: "float"
```

## 监控

```bash
# 查看 Redis 数据
redis-cli KEYS "comsrv:*"
redis-cli HGETALL "comsrv:1001:m"

# 查看服务日志
RUST_LOG=hissrv=debug cargo run
```

## Docker 部署

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/hissrv /usr/local/bin/
COPY config/hissrv.yaml /etc/hissrv.yaml
CMD ["hissrv"]
```

## 性能优化

- 调整 `polling_interval` 平衡延迟和资源使用
- 增加 `batch_size` 提高写入吞吐量
- 使用 Lua 脚本减少数据传输量
- Redis `SCAN` 避免阻塞操作