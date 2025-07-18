# Redis Pub/Sub 功能使用指南

## 概述

comsrv 现在支持在设置点位值时自动发布到 Redis Pub/Sub 通道。这允许其他服务实时订阅数据变化。

## 功能特性

- **批量发布优化**: 使用 pipeline 批量发布消息，提高性能
- **可配置缓冲**: 支持按数量（batch_size）或时间（batch_timeout_ms）触发发布
- **向后兼容**: 不影响现有功能，可通过配置开关控制
- **统一消息格式**: JSON 格式的标准化消息

## 配置

在服务配置文件中添加 pubsub 配置：

```yaml
service:
  redis:
    url: "redis://127.0.0.1:6379"
    pubsub:
      enabled: true              # 是否启用发布功能
      batch_size: 100           # 批量大小
      batch_timeout_ms: 50      # 批量超时（毫秒）
      publish_on_set: true      # 在 set 操作时发布
      message_version: "1.0"    # 消息版本
```

## 消息格式

发布的消息采用 JSON 格式：

```json
{
  "channel_id": 1001,
  "point_type": "m",
  "point_id": 10001,
  "value": 25.6,
  "timestamp": 1736764800000,
  "version": "1.0"
}
```

## 通道命名

使用与存储键相同的扁平化格式作为通道名：

- 测量点: `{channel_id}:m:{point_id}`
- 信号点: `{channel_id}:s:{point_id}`
- 控制点: `{channel_id}:c:{point_id}`
- 调节点: `{channel_id}:a:{point_id}`

例如：`1001:m:10001` 表示通道 1001 的测量点 10001

## 使用示例

### 1. 创建带发布功能的存储实例

```rust
use comsrv::core::redis::storage::RedisStorage;
use comsrv::core::config::types::redis::PubSubConfig;

let pubsub_config = PubSubConfig {
    enabled: true,
    batch_size: 100,
    batch_timeout_ms: 50,
    publish_on_set: true,
    message_version: "1.0".to_string(),
};

let storage = RedisStorage::with_publisher(redis_url, &pubsub_config).await?;
```

### 2. 设置点位值（自动发布）

```rust
// 单点更新
storage.set_point(1001, "m", 10001, 25.6).await?;

// 批量更新
let updates = vec![
    PointUpdate {
        channel_id: 1001,
        point_type: "m",
        point_id: 10002,
        value: 30.5,
    },
    // ...
];
storage.set_points(&updates).await?;
```

### 3. 订阅消息

运行订阅者示例：

```bash
cargo run --example pubsub_subscriber
```

或在其他服务中订阅：

```rust
use redis::AsyncCommands;

let mut pubsub = conn.into_pubsub();
pubsub.psubscribe("*:m:*").await?;  // 订阅所有测量点

loop {
    let msg = pubsub.get_message().await?;
    let payload: String = msg.get_payload()?;
    let json: serde_json::Value = serde_json::from_str(&payload)?;
    // 处理消息...
}
```

## 性能优化

1. **批量大小**: 根据数据频率调整 `batch_size`
   - 高频数据: 100-500
   - 低频数据: 10-50

2. **批量超时**: 根据实时性要求调整 `batch_timeout_ms`
   - 高实时性: 10-50ms
   - 普通场景: 50-200ms

3. **监控**: 查看日志中的批量发布统计信息

## 测试

运行集成测试：

```bash
# 终端 1: 启动订阅者
cargo run --example pubsub_subscriber

# 终端 2: 运行测试
cargo run --example pubsub_test
```

## 注意事项

1. 发布是异步的，不会阻塞存储操作
2. 发布失败不会影响数据存储
3. 关闭存储时会等待所有待发布消息完成
4. 大批量更新时，消息会自动分批发布