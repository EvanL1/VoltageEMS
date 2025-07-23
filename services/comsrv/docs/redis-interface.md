# comsrv Redis 接口规范

## 概述

comsrv 使用 Redis 作为实时数据存储和消息传递中间件。通过 Hash 结构存储点位数据，实现 O(1) 的查询性能，并使用 Pub/Sub 机制推送数据变化。

## 数据存储

### Hash 结构设计

**键格式**: `comsrv:{channelID}:{type}`

**类型映射**:
- `m` - 测量值 (measurement/telemetry)
- `s` - 信号值 (signal)
- `c` - 控制值 (control)
- `a` - 调节值 (adjustment)

**存储示例**:
```bash
# 通道 1001 的测量值
comsrv:1001:m → {
    "10001": "25.123456",
    "10002": "380.500000",
    "10003": "50.250000"
}

# 通道 1001 的信号值
comsrv:1001:s → {
    "20001": "1.000000",
    "20002": "0.000000"
}
```

### 数据格式

使用 `StandardFloat` 确保所有浮点数值为 6 位小数精度：

```rust
use voltage_libs::types::StandardFloat;

// 创建标准化数值
let value = StandardFloat::new(25.1);  // 存储为 "25.100000"

// 写入 Redis
let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
redis_client.hset(&hash_key, point_id.to_string(), value.to_redis()).await?;
```

## 批量操作

### 批量写入

```rust
pub async fn batch_update_points(
    &mut self,
    channel_id: u16,
    point_type: &str,
    updates: Vec<(u32, StandardFloat)>,
) -> Result<()> {
    let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
    
    // 使用 pipeline 批量更新
    let mut pipe = redis::pipe();
    pipe.atomic();
    
    for (point_id, value) in updates {
        pipe.hset(&hash_key, point_id.to_string(), value.to_redis());
    }
    
    pipe.query_async(&mut self.conn).await?;
    Ok(())
}
```

### 批量读取

```rust
pub async fn batch_read_points(
    &mut self,
    channel_id: u16,
    point_type: &str,
    point_ids: Vec<u32>,
) -> Result<HashMap<u32, StandardFloat>> {
    let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
    
    // 构建字段列表
    let fields: Vec<String> = point_ids.iter()
        .map(|id| id.to_string())
        .collect();
    
    // 批量获取
    let values: Vec<Option<String>> = redis::cmd("HMGET")
        .arg(&hash_key)
        .arg(&fields)
        .query_async(&mut self.conn)
        .await?;
    
    // 解析结果
    let mut result = HashMap::new();
    for (id, value) in point_ids.iter().zip(values.iter()) {
        if let Some(val) = value {
            if let Ok(parsed) = val.parse::<f64>() {
                result.insert(*id, StandardFloat::new(parsed));
            }
        }
    }
    
    Ok(result)
}
```

## 发布订阅

### 发布格式

**通道**: `comsrv:{channelID}:{type}`  
**消息**: `{pointID}:{value}`

```rust
pub async fn publish_point_update(
    &mut self,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
    value: StandardFloat,
) -> Result<()> {
    let channel = format!("comsrv:{}:{}", channel_id, point_type);
    let message = format!("{}:{}", point_id, value.to_redis());
    
    self.conn.publish(&channel, message).await?;
    Ok(())
}
```

### 批量发布

```rust
pub async fn publish_batch_updates(
    &mut self,
    channel_id: u16,
    point_type: &str,
    updates: Vec<(u32, StandardFloat)>,
) -> Result<()> {
    let channel = format!("comsrv:{}:{}", channel_id, point_type);
    
    // 使用 pipeline 批量发布
    let mut pipe = redis::pipe();
    
    for (point_id, value) in updates {
        let message = format!("{}:{}", point_id, value.to_redis());
        pipe.publish(&channel, &message);
    }
    
    pipe.query_async(&mut self.conn).await?;
    Ok(())
}
```

### 订阅示例

其他服务订阅数据变化：

```rust
pub async fn subscribe_channel_updates(
    channel_id: u16,
    point_type: &str,
) -> Result<()> {
    let mut pubsub = redis_client.get_async_pubsub().await?;
    let pattern = format!("comsrv:{}:{}", channel_id, point_type);
    
    pubsub.subscribe(&pattern).await?;
    
    while let Some(msg) = pubsub.on_message().next().await {
        let payload: String = msg.get_payload()?;
        
        // 解析消息: "pointID:value"
        if let Some((point_id, value)) = payload.split_once(':') {
            let point_id: u32 = point_id.parse()?;
            let value: f64 = value.parse()?;
            
            // 处理数据更新
            handle_point_update(channel_id, point_type, point_id, value).await?;
        }
    }
    
    Ok(())
}
```

## 控制命令

### 订阅控制通道

comsrv 订阅控制命令：

```rust
pub async fn subscribe_control_commands(channel_id: u16) -> Result<()> {
    let mut pubsub = redis_client.get_async_pubsub().await?;
    
    // 订阅控制和调节命令
    pubsub.subscribe(format!("cmd:{}:control", channel_id)).await?;
    pubsub.subscribe(format!("cmd:{}:adjustment", channel_id)).await?;
    
    while let Some(msg) = pubsub.on_message().next().await {
        let channel: String = msg.get_channel_name()?;
        let payload: String = msg.get_payload()?;
        
        // 解析命令
        let command: ControlCommand = serde_json::from_str(&payload)?;
        
        // 执行命令
        if channel.ends_with(":control") {
            execute_control_command(channel_id, command).await?;
        } else if channel.ends_with(":adjustment") {
            execute_adjustment_command(channel_id, command).await?;
        }
    }
    
    Ok(())
}
```

### 命令格式

```json
{
    "point_id": 30001,
    "value": 1.0,
    "timestamp": 1642592400000,
    "source": "modsrv",
    "command_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## 性能优化

### 1. 连接池管理

```rust
pub struct RedisPool {
    pool: deadpool_redis::Pool,
}

impl RedisPool {
    pub fn new(redis_url: &str, max_size: usize) -> Result<Self> {
        let config = deadpool_redis::Config {
            url: Some(redis_url.to_string()),
            pool: Some(deadpool_redis::PoolConfig {
                max_size,
                timeouts: deadpool_redis::Timeouts {
                    wait: Some(Duration::from_secs(10)),
                    create: Some(Duration::from_secs(10)),
                    recycle: Some(Duration::from_secs(10)),
                },
            }),
            ..Default::default()
        };
        
        let pool = config.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }
}
```

### 2. 缓存策略

```rust
pub struct PointCache {
    data: HashMap<String, CachedPoint>,
    max_age: Duration,
}

struct CachedPoint {
    value: StandardFloat,
    timestamp: Instant,
}

impl PointCache {
    pub fn should_update(&self, key: &str, new_value: StandardFloat) -> bool {
        match self.data.get(key) {
            Some(cached) => {
                // 值变化或缓存过期才更新
                cached.value != new_value || 
                cached.timestamp.elapsed() > self.max_age
            }
            None => true,
        }
    }
}
```

### 3. 批量优化

```rust
// 批量大小配置
const BATCH_SIZE: usize = 1000;
const BATCH_TIMEOUT: Duration = Duration::from_millis(100);

// 批量收集器
pub struct BatchCollector {
    buffer: Vec<(u32, StandardFloat)>,
    last_flush: Instant,
}

impl BatchCollector {
    pub async fn add(&mut self, point_id: u32, value: StandardFloat) -> Result<()> {
        self.buffer.push((point_id, value));
        
        if self.buffer.len() >= BATCH_SIZE || 
           self.last_flush.elapsed() > BATCH_TIMEOUT {
            self.flush().await?;
        }
        
        Ok(())
    }
    
    async fn flush(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            // 批量写入 Redis
            batch_update_points(&self.buffer).await?;
            self.buffer.clear();
            self.last_flush = Instant::now();
        }
        Ok(())
    }
}
```

## 监控和调试

### Redis 命令示例

```bash
# 查看所有 comsrv 键
redis-cli --scan --pattern "comsrv:*"

# 查看特定通道的测量值
redis-cli hgetall "comsrv:1001:m"

# 监控实时变化
redis-cli monitor | grep comsrv

# 订阅特定通道
redis-cli psubscribe "comsrv:1001:*"

# 查看 Hash 大小
redis-cli hlen "comsrv:1001:m"

# 获取特定点位值
redis-cli hget "comsrv:1001:m" "10001"
```

### 性能分析

```bash
# 查看内存使用
redis-cli memory usage "comsrv:1001:m"

# 查看键的 TTL
redis-cli ttl "comsrv:1001:m"

# 分析慢查询
redis-cli slowlog get 10
```

## 错误处理

### 连接错误

```rust
// 自动重连
loop {
    match redis_client.ping().await {
        Ok(_) => break,
        Err(e) => {
            error!("Redis connection lost: {}", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
            redis_client.reconnect().await?;
        }
    }
}
```

### 数据验证

```rust
// 验证点位值范围
fn validate_point_value(
    point_type: &str,
    value: f64,
) -> Result<StandardFloat> {
    match point_type {
        "m" => {
            // 测量值范围检查
            if value < -1e6 || value > 1e6 {
                return Err(Error::ValueOutOfRange);
            }
        }
        "s" => {
            // 信号值只能是 0 或 1
            if value != 0.0 && value != 1.0 {
                return Err(Error::InvalidSignalValue);
            }
        }
        _ => {}
    }
    
    Ok(StandardFloat::new(value))
}
```