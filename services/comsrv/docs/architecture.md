# comsrv 架构设计

## 概述

comsrv 采用插件化架构设计，将协议逻辑与传输层分离，实现了高度的可扩展性和灵活性。服务通过 Redis Hash 结构存储实时数据，并使用 Pub/Sub 机制推送数据变化。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      comsrv                             │
├─────────────────────────────────────────────────────────┤
│                    API Server                           │
│                 (Health/Metrics)                        │
├─────────────────────────────────────────────────────────┤
│                 Protocol Manager                        │
│          ┌──────────┬──────────┬──────────┐             │
│          │ Modbus   │  IEC104  │   CAN    │             │
│          │ Plugin   │  Plugin  │  Plugin  │             │
│          └──────────┴──────────┴──────────┘             │
├─────────────────────────────────────────────────────────┤
│                 Transport Layer                         │
│     ┌─────────┬─────────┬─────────┬─────────┐           │
│     │   TCP   │ Serial  │   CAN   │  Mock   │           │
│     └─────────┴─────────┴─────────┴─────────┘           │
├─────────────────────────────────────────────────────────┤
│                  Data Pipeline                          │
│     ┌──────────┬──────────┬──────────┬──────────┐       │
│     │  Parser  │ Validator│  Cache   │Publisher │       │
│     └──────────┴──────────┴──────────┴──────────┘       │
├─────────────────────────────────────────────────────────┤
│                  Redis Interface                        │
│          ┌──────────────┬──────────────┐                │
│          │ Hash Storage │   Pub/Sub    │                │
│          └──────────────┴──────────────┘                │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Protocol Manager

负责管理所有协议插件的生命周期：

```rust
pub struct ProtocolManager {
    plugins: HashMap<String, Box<dyn ProtocolPlugin>>,
    channels: Vec<ChannelConfig>,
}
```

主要职责：
- 插件注册和初始化
- 通道配置管理
- 任务调度和监控

### 2. Protocol Plugins

每个协议实现 `ProtocolPlugin` trait：

```rust
#[async_trait]
pub trait ProtocolPlugin: Send + Sync {
    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;
    async fn collect_data(&self) -> Result<Vec<PointData>>;
    async fn send_command(&self, command: Command) -> Result<()>;
    fn get_info(&self) -> PluginInfo;
}
```

### 3. Transport Layer

统一的传输层抽象：

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send(&mut self, data: &[u8]) -> Result<()>;
    async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize>;
}
```

### 4. Data Pipeline

数据处理流水线：

1. **Parser**: 解析原始数据为标准格式
2. **Validator**: 验证数据范围和有效性
3. **Cache**: 本地缓存减少重复写入
4. **Publisher**: 发布变化数据到 Redis

### 5. Redis Interface

#### Hash 存储

```rust
// 批量更新 Hash
pub async fn batch_update_hash(
    &mut self,
    channel_id: u16,
    point_type: &str,
    updates: Vec<(u32, StandardFloat)>,
) -> Result<()> {
    let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
    let mut pipe = redis::pipe();

    for (point_id, value) in updates {
        pipe.hset(&hash_key, point_id.to_string(), value.to_redis());
    }

    pipe.query_async(&mut self.conn).await?;
    Ok(())
}
```

#### 发布订阅

```rust
// 发布数据变化
pub async fn publish_updates(
    &mut self,
    channel_id: u16,
    point_type: &str,
    updates: Vec<(u32, StandardFloat)>,
) -> Result<()> {
    let channel = format!("comsrv:{}:{}", channel_id, point_type);

    for (point_id, value) in updates {
        let message = format!("{}:{}", point_id, value.to_redis());
        self.conn.publish(&channel, message).await?;
    }

    Ok(())
}
```

## 数据流

### 采集流程

1. **协议插件**从设备读取数据
2. **解析器**转换为标准 `PointData` 格式
3. **验证器**检查数据有效性
4. **缓存**比较并过滤未变化数据
5. **存储**批量写入 Redis Hash
6. **发布**推送变化到订阅者

### 控制流程

1. **订阅**监听控制命令通道
2. **解析**提取控制参数
3. **验证**检查权限和范围
4. **执行**调用协议插件发送
5. **确认**更新执行状态

## 性能优化

### 1. 批量操作

```rust
// 批量读取设备数据
let batch_size = 100;
for chunk in points.chunks(batch_size) {
    let values = protocol.read_multiple(chunk).await?;
    // 处理数据...
}
```

### 2. 连接池

```rust
// Redis 连接池配置
let pool = RedisPool::builder()
    .max_size(32)
    .min_idle(8)
    .build(redis_url)?;
```

### 3. 异步并发

```rust
// 并发处理多个通道
let tasks: Vec<_> = channels.iter()
    .map(|channel| {
        tokio::spawn(process_channel(channel.clone()))
    })
    .collect();

futures::future::join_all(tasks).await;
```

### 4. 内存优化

- 使用 Hash 结构减少键数量
- 点位 ID 使用 u32 而非字符串
- 复用缓冲区避免频繁分配

## 错误处理

### 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ComsrvError {
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Configuration error: {0}")]
    Config(String),
}
```

### 重试机制

```rust
pub struct RetryPolicy {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    exponential_base: f64,
}
```

## 监控指标

### Prometheus 指标

- `comsrv_points_collected_total` - 采集点位总数
- `comsrv_points_updated_total` - 更新点位总数
- `comsrv_protocol_errors_total` - 协议错误计数
- `comsrv_redis_operations_total` - Redis 操作计数
- `comsrv_channel_status` - 通道连接状态

### 健康检查

```rust
// GET /health
{
    "status": "healthy",
    "channels": {
        "1001": "connected",
        "1002": "disconnected"
    },
    "redis": "connected",
    "uptime": 3600
}
```

## 扩展性设计

### 添加新协议

1. 实现 `ProtocolPlugin` trait
2. 实现对应的解析逻辑
3. 注册到 `ProtocolFactory`
4. 配置协议参数

### 添加新传输方式

1. 实现 `Transport` trait
2. 处理连接管理
3. 注册到 `TransportFactory`
4. 更新配置模板

## 安全考虑

1. **访问控制**: Redis ACL 限制读写权限
2. **数据验证**: 严格的输入验证
3. **错误隔离**: 单个通道故障不影响其他
4. **审计日志**: 记录所有控制操作
