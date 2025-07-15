# 数据流架构

## 概述

VoltageEMS 的数据流设计遵循事件驱动架构，通过 Redis 作为中央消息总线，实现数据的实时流转。系统支持从设备采集、实时计算、历史存储到前端展示的完整数据链路。

## 数据流类型

### 1. 实时遥测数据流

从设备采集到前端展示的主要数据流路径。

```
┌─────────┐     ┌────────┐     ┌───────┐     ┌──────────┐     ┌────────┐
│ device  │ ──> │ comsrv │ ──> │ Redis │ ──> │ modsrv   │ ──> │ Redis  │
└─────────┘     └────────┘     └───────┘     └──────────┘     └────┬───┘
                                    │                              │
                                    v                              v
                              ┌───────────┐                 ┌─────────────┐
                              │  hissrv   │                 │ apigateway  │
                              │(InfluxDB) │                 │ (WebSocket) │
                              └───────────┘                 └─────────────┘
```

### 2. 控制命令数据流

从操作界面到设备执行的控制流程。

```
┌─────────┐     ┌────────────┐     ┌───────────┐     ┌─────────┐     ┌────────┐
│ web UI  │ ──> │ apigateway │ ──> │   Redis   │ ──> │ comsrv  │ ──> │ device │
└─────────┘     └────────────┘     │ (Pub/Sub) │     └─────────┘     └────────┘
                                   └───────────┘
```

### 3. 告警事件流

实时数据触发告警的处理流程。

```
┌───────┐     ┌──────────┐     ┌─────────────┐     ┌──────────┐
│ Redis │ ──> │ alarmsrv │ ──> │ Redis Queue │ ──> │ 通知系统   │
└───────┘     └──────────┘     └─────────────┘     └──────────┘
```

## 数据采集流程

### comsrv 数据采集

```rust
// 1. 协议插件采集数据
let data = protocol_plugin.read_data().await?;

// 2. 数据标准化
let point_data = PointData {
    channel_id,
    point_type: "m",  // measurement
    point_id: 10001,
    value: data.value,
    timestamp: Utc::now().timestamp_millis(),
    quality: Quality::Good,
};

// 3. 写入 Redis
storage.set_point(
    point_data.channel_id,
    &point_data.point_type,
    point_data.point_id,
    point_data.value
).await?;

// 4. 发布更新事件
redis.publish("point:update", &point_data).await?;
```

### 批量优化

- 使用 Redis Pipeline 批量写入
- 聚合小批次数据减少网络开销
- 异步处理不阻塞采集线程

## 实时计算流程

### modsrv 计算引擎

#### 1. 数据订阅

```rust
// 订阅点位更新
let mut pubsub = redis.get_async_pubsub().await?;
pubsub.subscribe("point:update").await?;

// 处理更新
while let Some(msg) = pubsub.on_message().next().await {
    let update: PointUpdate = serde_json::from_str(&msg.get_payload()?)?;
    process_update(update).await?;
}
```

#### 2. 计算执行

```rust
// DAG 计算图
let dag = ComputationDAG {
    nodes: vec![
        // 输入节点
        Node::Input { point_id: "1001:m:10001" },
        Node::Input { point_id: "1001:m:10002" },

        // 计算节点
        Node::Compute {
            id: "total_power",
            expression: "input1 + input2",
            inputs: vec!["1001:m:10001", "1001:m:10002"],
        },
    ],
};

// 执行计算
let results = dag.execute(&input_data).await?;
```

#### 3. 结果存储

```rust
// 存储计算结果
for (output_id, value) in results {
    storage.set_point(
        channel_id,
        "m",  // 计算结果也是 measurement
        output_id,
        value
    ).await?;
}
```

## 历史数据存储

### hissrv 数据桥接

#### 1. 批量缓冲

```rust
// 缓冲区管理
struct BatchBuffer {
    points: Vec<PointData>,
    max_size: usize,
    max_duration: Duration,
}

// 添加数据点
buffer.add(point_data);

// 触发批量写入
if buffer.should_flush() {
    influxdb.write_batch(&buffer.points).await?;
    buffer.clear();
}
```

#### 2. 数据压缩

- 时间戳压缩：相对时间戳
- 值压缩：增量编码
- 批量压缩：Gzip

#### 3. 写入策略

```rust
// InfluxDB 行协议
let line_protocol = format!(
    "{},channel={},type={},point={} value={} {}",
    measurement,
    channel_id,
    point_type,
    point_id,
    value,
    timestamp_ns
);

// 批量写入
influxdb.write_batch(&lines).await?;
```

## 控制命令处理

### 命令下发流程

#### 1. API 接收

```rust
// RESTful API
#[post("/control/{channel_id}/{point_id}")]
async fn send_control(
    channel_id: u16,
    point_id: u32,
    command: ControlCommand,
) -> Result<Response> {
    // 验证权限
    check_permission(&user, &command)?;

    // 发布命令
    let channel = format!("cmd:{}:control", channel_id);
    redis.publish(&channel, &command).await?;

    Ok(Response::success())
}
```

#### 2. comsrv 执行

```rust
// 订阅控制命令
let channel = format!("cmd:{}:control", self.channel_id);
pubsub.subscribe(&channel).await?;

// 处理命令
while let Some(msg) = pubsub.on_message().next().await {
    let cmd: ControlCommand = serde_json::from_str(&msg.get_payload()?)?;

    // 协议转换
    let device_cmd = protocol.translate_command(&cmd)?;

    // 执行命令
    transport.write(&device_cmd).await?;

    // 反馈结果
    publish_command_result(&cmd.id, &result).await?;
}
```

## 实时推送机制

### WebSocket 推送

#### 1. 订阅管理

```rust
// 客户端订阅
struct Subscription {
    client_id: String,
    patterns: Vec<String>,  // 如 "1001:m:*"
    filters: Vec<Filter>,
}

// 匹配推送
fn should_push(sub: &Subscription, update: &PointUpdate) -> bool {
    sub.patterns.iter().any(|p| matches_pattern(p, &update.key))
}
```

#### 2. 推送优化

- 合并推送：聚合 100ms 内的更新
- 增量推送：只推送变化的数据
- 压缩推送：使用 MessagePack

## 数据一致性保证

### 1. 写入确认

```rust
// Pipeline 确认
let results: Vec<Result<()>> = pipe.query_async(&mut conn).await?;
for (i, result) in results.iter().enumerate() {
    if result.is_err() {
        log::error!("Failed to write point {}: {:?}", updates[i].id, result);
        // 重试逻辑
    }
}
```

### 2. 事务支持

```rust
// Redis 事务
let mut pipe = redis::pipe();
pipe.atomic()
    .set(&key1, &value1)
    .set(&key2, &value2)
    .query_async(&mut conn)
    .await?;
```

### 3. 幂等性设计

- 使用唯一命令 ID
- 重复命令检测
- 状态机管理

## 性能优化策略

### 1. 缓存层次

```
L1: 进程内缓存 (HashMap)
L2: Redis 缓存
L3: InfluxDB 持久化
```

### 2. 并发控制

```rust
// 限流器
let semaphore = Arc::new(Semaphore::new(100));

// 并发处理
let tasks: Vec<_> = updates.into_iter().map(|update| {
    let sem = semaphore.clone();
    tokio::spawn(async move {
        let _permit = sem.acquire().await?;
        process_update(update).await
    })
}).collect();

futures::future::join_all(tasks).await;
```

### 3. 批处理优化

- 自适应批大小
- 动态超时调整
- 背压控制

## 监控与调试

### 1. 数据流追踪

```rust
// OpenTelemetry 集成
let tracer = global::tracer("data-flow");
let span = tracer.start("process_update");
span.set_attribute("point_id", point_id.to_string());
// ... 处理逻辑
span.end();
```

### 2. 指标收集

- 数据延迟分布
- 处理吞吐量
- 错误率统计

### 3. 日志聚合

```rust
// 结构化日志
info!(
    point_id = %point_id,
    latency_ms = %latency.as_millis(),
    "Point update processed"
);
```

## 故障处理

### 1. 断线重连

- 自动重连机制
- 指数退避策略
- 连接池管理

### 2. 数据补偿

- 本地缓存队列
- 断点续传
- 数据去重

### 3. 降级策略

- 优先保证核心数据
- 降低采集频率
- 跳过非关键计算
