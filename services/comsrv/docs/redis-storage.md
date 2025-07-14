# comsrv Redis 存储设计

## 概述

comsrv 采用扁平化的 Redis 键值对存储方案，每个数据点独立存储，实现高性能的实时数据读写。该设计支持百万级点位的并发访问，是整个系统数据流的源头。

## 存储结构

### 键命名规范

```
实时数据：{channel_id}:{type}:{point_id}
配置数据：cfg:{channel_id}:{type}:{point_id}
```

### 数据类型映射

| 四遥类型 | 缩写 | 英文名称     | 说明           |
|---------|------|-------------|----------------|
| 遥测(YC) | m    | Measurement | 模拟量测量值    |
| 遥信(YX) | s    | Signal      | 数字量状态     |
| 遥控(YK) | c    | Control     | 控制命令       |
| 遥调(YT) | a    | Adjustment  | 模拟量设定值    |

### 存储示例

```bash
# 遥测数据
1001:m:10001 -> "380.5:1704956400"      # 电压值
1001:m:10002 -> "45.2:1704956400"       # 电流值
1001:m:10003 -> "1234.56:1704956400"    # 功率值

# 遥信数据
1001:s:20001 -> "1:1704956400"          # 开关状态
1001:s:20002 -> "0:1704956400"          # 告警信号

# 遥控数据
1001:c:30001 -> "0:1704956400"          # 分闸命令
1001:c:30002 -> "1:1704956400"          # 合闸命令

# 遥调数据
1001:a:40001 -> "50.0:1704956400"       # 功率设定值
1001:a:40002 -> "380.0:1704956400"      # 电压设定值
```

## 核心实现

### RedisStorage 结构

```rust
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Pipeline};

pub struct RedisStorage {
    conn: ConnectionManager,
}

impl RedisStorage {
    /// 创建存储实例
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }
}
```

### 单点操作

```rust
/// 设置单个点位值
pub async fn set_point(
    &mut self,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: f64,
) -> Result<()> {
    let key = format!("{}:{}:{}", channel_id, point_type, point_id);
    let timestamp = chrono::Utc::now().timestamp_millis();
    let data = format!("{}:{}", value, timestamp);
    
    self.conn.set(&key, &data).await?;
    
    Ok(())
}

/// 获取单个点位值
pub async fn get_point(
    &mut self,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) -> Result<Option<(f64, i64)>> {
    let key = format!("{}:{}:{}", channel_id, point_type, point_id);
    
    let data: Option<String> = self.conn.get(&key).await?;
    
    match data {
        Some(s) => {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 2 {
                let value = parts[0].parse::<f64>()?;
                let timestamp = parts[1].parse::<i64>()?;
                Ok(Some((value, timestamp)))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}
```

### 批量操作

```rust
/// 批量更新点位值
pub async fn set_points(&mut self, updates: &[PointUpdate]) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }
    
    let mut pipe = Pipeline::new();
    let timestamp = chrono::Utc::now().timestamp_millis();
    
    for update in updates {
        let key = format!("{}:{}:{}", 
            update.channel_id, 
            update.point_type, 
            update.point_id
        );
        let data = format!("{}:{}", update.value, timestamp);
        pipe.set(&key, &data);
    }
    
    pipe.query_async(&mut self.conn).await?;
    
    Ok(())
}

/// 批量获取点位值
pub async fn get_points(&mut self, keys: &[PointKey]) -> Result<Vec<Option<(f64, i64)>>> {
    if keys.is_empty() {
        return Ok(vec![]);
    }
    
    let redis_keys: Vec<String> = keys.iter()
        .map(|k| format!("{}:{}:{}", k.channel_id, k.point_type, k.point_id))
        .collect();
    
    let values: Vec<Option<String>> = self.conn.mget(&redis_keys).await?;
    
    let results = values.into_iter()
        .map(|opt_str| {
            opt_str.and_then(|s| {
                let parts: Vec<&str> = s.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(value), Ok(ts)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<i64>()
                    ) {
                        Some((value, ts))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        })
        .collect();
    
    Ok(results)
}
```

### 通道扫描

```rust
/// 扫描通道下的所有点位
pub async fn scan_channel_points(
    &mut self,
    channel_id: u16,
    point_type: &str,
) -> Result<Vec<(u32, f64, i64)>> {
    let pattern = format!("{}:{}:*", channel_id, point_type);
    let mut results = Vec::new();
    
    let mut iter = self.conn.scan_match(&pattern).await?;
    while let Some(key) = iter.next_item().await {
        // 解析点位ID
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() == 3 {
            if let Ok(point_id) = parts[2].parse::<u32>() {
                // 获取值
                if let Ok(Some(data)) = self.conn.get::<_, Option<String>>(&key).await {
                    let value_parts: Vec<&str> = data.split(':').collect();
                    if value_parts.len() == 2 {
                        if let (Ok(value), Ok(ts)) = (
                            value_parts[0].parse::<f64>(),
                            value_parts[1].parse::<i64>()
                        ) {
                            results.push((point_id, value, ts));
                        }
                    }
                }
            }
        }
    }
    
    Ok(results)
}
```

## 四遥操作接口

### FourTelemetryOperations Trait

```rust
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    /// 批量更新遥测值
    async fn batch_update_measurements(
        &self, 
        updates: Vec<MeasurementUpdate>
    ) -> Result<()>;
    
    /// 批量更新遥信值
    async fn batch_update_signals(
        &self, 
        updates: Vec<SignalUpdate>
    ) -> Result<()>;
    
    /// 获取遥控状态
    async fn get_control_status(
        &self, 
        point_id: u32
    ) -> Result<Option<ControlStatus>>;
    
    /// 获取遥调值
    async fn get_adjustment_value(
        &self, 
        point_id: u32
    ) -> Result<Option<AdjustmentValue>>;
}
```

### 实现示例

```rust
pub struct RedisOperations {
    storage: Arc<Mutex<RedisStorage>>,
    channel_id: u16,
}

#[async_trait]
impl FourTelemetryOperations for RedisOperations {
    async fn batch_update_measurements(
        &self, 
        updates: Vec<MeasurementUpdate>
    ) -> Result<()> {
        let point_updates: Vec<PointUpdate> = updates.into_iter()
            .map(|u| PointUpdate {
                channel_id: self.channel_id,
                point_type: "m",
                point_id: u.point_id,
                value: u.value,
            })
            .collect();
        
        self.storage.lock().await.set_points(&point_updates).await?;
        
        // 发布更新事件
        self.publish_updates("measurement", &point_updates).await?;
        
        Ok(())
    }
}
```

## 性能优化

### 1. Pipeline 批量写入

```rust
// 优化前：逐个写入
for update in updates {
    storage.set_point(
        update.channel_id,
        &update.point_type,
        update.point_id,
        update.value
    ).await?;
}

// 优化后：Pipeline 批量写入
let mut pipe = Pipeline::new();
for update in updates {
    let key = make_key(update);
    let value = make_value(update);
    pipe.set(&key, &value);
}
pipe.query_async(&mut conn).await?;
```

### 2. MGET 批量读取

```rust
// 优化前：逐个读取
let mut results = Vec::new();
for key in keys {
    let value = storage.get_point(
        key.channel_id,
        &key.point_type,
        key.point_id
    ).await?;
    results.push(value);
}

// 优化后：MGET 批量读取
let results = storage.get_points(&keys).await?;
```

### 3. 连接池管理

```rust
// 使用 ConnectionManager 自动管理连接
let manager = ConnectionManager::new(client).await?;

// 连接池配置
let pool = Pool::builder()
    .max_size(32)
    .min_idle(8)
    .connection_timeout(Duration::from_secs(2))
    .build(manager)?;
```

### 4. 异步并发

```rust
// 并发处理多个通道
let tasks: Vec<_> = channels.iter()
    .map(|channel| {
        let storage = storage.clone();
        tokio::spawn(async move {
            process_channel(storage, channel).await
        })
    })
    .collect();

let results = futures::future::join_all(tasks).await;
```

## 数据发布

### 更新事件发布

```rust
/// 发布数据更新事件
async fn publish_updates(
    &self,
    update_type: &str,
    updates: &[PointUpdate]
) -> Result<()> {
    let channel = format!("updates:{}:{}", self.channel_id, update_type);
    
    let message = serde_json::json!({
        "channel_id": self.channel_id,
        "type": update_type,
        "count": updates.len(),
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "points": updates.iter().map(|u| u.point_id).collect::<Vec<_>>(),
    });
    
    self.redis_client.publish(&channel, message.to_string()).await?;
    
    Ok(())
}
```

### 批量更新通知

```rust
// 聚合更新通知，避免消息风暴
let mut update_buffer = UpdateBuffer::new(Duration::from_millis(100));

for update in stream {
    update_buffer.add(update);
    
    if let Some(batch) = update_buffer.get_batch() {
        publish_batch_update(batch).await?;
    }
}
```

## 监控与诊断

### 性能指标

```rust
// 写入延迟统计
let start = Instant::now();
storage.set_points(&updates).await?;
let duration = start.elapsed();

metrics::histogram!("comsrv.redis.write_duration", duration.as_secs_f64());
metrics::counter!("comsrv.redis.write_points", updates.len() as u64);
```

### 健康检查

```rust
pub async fn health_check(&mut self) -> Result<HealthStatus> {
    // 检查 Redis 连接
    let start = Instant::now();
    let pong: String = self.conn.ping().await?;
    let latency = start.elapsed();
    
    // 检查键空间
    let info: String = self.conn.info("keyspace").await?;
    let keyspace_stats = parse_keyspace_info(&info)?;
    
    Ok(HealthStatus {
        redis_connected: pong == "PONG",
        latency_ms: latency.as_millis() as u32,
        total_keys: keyspace_stats.total_keys,
        memory_usage_mb: keyspace_stats.memory_mb,
    })
}
```

## 错误处理

### 重试机制

```rust
async fn set_point_with_retry(
    storage: &mut RedisStorage,
    key: &str,
    value: &str,
    max_retries: u32,
) -> Result<()> {
    let mut retries = 0;
    let mut backoff = Duration::from_millis(100);
    
    loop {
        match storage.conn.set(key, value).await {
            Ok(_) => return Ok(()),
            Err(e) if retries < max_retries => {
                warn!("Redis write failed, retry {}/{}: {}", 
                    retries + 1, max_retries, e);
                tokio::time::sleep(backoff).await;
                backoff *= 2;
                retries += 1;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### 降级处理

```rust
// 内存缓冲队列
let mut buffer = VecDeque::new();

// Redis 不可用时缓存数据
if !redis_available {
    buffer.push_back(update);
    
    if buffer.len() > MAX_BUFFER_SIZE {
        warn!("Buffer overflow, dropping oldest updates");
        buffer.pop_front();
    }
}

// Redis 恢复后批量写入
if redis_available && !buffer.is_empty() {
    flush_buffer_to_redis(&mut buffer).await?;
}
```

## 最佳实践

1. **批量优先**：尽量使用批量操作减少网络开销
2. **合理分片**：利用 channel_id 自然分片特性
3. **监控先行**：完善的监控指标便于问题定位
4. **优雅降级**：Redis 故障时的本地缓存策略
5. **连接复用**：使用连接池避免频繁建立连接