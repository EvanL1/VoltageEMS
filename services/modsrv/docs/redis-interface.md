# modsrv Redis 接口规范

## 概述

modsrv 使用 Redis Hash 结构存储计算结果和设备状态，与 comsrv 不同的是，modsrv 存储的数据**不包含时间戳**，仅存储计算值。这种设计简化了数据结构，提高了查询效率。

## 数据存储

### Hash 结构设计

**键格式**: `modsrv:{modelname}:{type}`

**类型映射**:
- `measurement` - 计算的测量值
- `control` - 控制状态或设定值
- `status` - 设备状态信息
- `config` - 模型配置参数

**存储示例**:
```bash
# 电表模型的测量值
modsrv:power_meter:measurement → {
    "total_power": "1200.500000",
    "power_factor": "0.950000",
    "efficiency": "0.890000",
    "energy_today": "8500.250000"
}

# 电表模型的控制值
modsrv:power_meter:control → {
    "enable": "1.000000",
    "power_limit": "1000.000000",
    "alarm_threshold": "900.000000"
}

# 设备状态
modsrv:power_meter:status → {
    "online": "1.000000",
    "fault_code": "0.000000",
    "last_reset": "0.000000"
}
```

### 数据格式特点

1. **无时间戳存储**: 所有值仅包含数据本身，不存储时间信息
2. **标准化精度**: 使用 `StandardFloat` 确保 6 位小数精度
3. **扁平化结构**: 使用 Hash 字段直接存储，避免嵌套

```rust
use voltage_libs::types::StandardFloat;

// 存储计算结果
let value = StandardFloat::new(1200.5);
let hash_key = format!("modsrv:{}:measurement", model_name);
redis_client.hset(&hash_key, "total_power", value.to_redis()).await?;
// 存储: "1200.500000"
```

## 读取接口

### 从 comsrv 读取数据

```rust
pub async fn read_comsrv_data(
    &mut self,
    channel_id: u16,
    point_type: &str,
    point_ids: Vec<u32>,
) -> Result<HashMap<u32, f64>> {
    let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
    
    // 构建字段列表
    let fields: Vec<String> = point_ids.iter()
        .map(|id| id.to_string())
        .collect();
    
    // 批量读取
    let values: Vec<Option<String>> = self.redis_client
        .hmget(&hash_key, &fields)
        .await?;
    
    // 解析结果
    let mut result = HashMap::new();
    for (id, value) in point_ids.iter().zip(values.iter()) {
        if let Some(val) = value {
            if let Ok(parsed) = val.parse::<f64>() {
                result.insert(*id, parsed);
            }
        }
    }
    
    Ok(result)
}
```

### 读取计算结果

```rust
pub async fn get_model_measurements(
    &mut self,
    model_name: &str,
    fields: Vec<&str>,
) -> Result<HashMap<String, StandardFloat>> {
    let hash_key = format!("modsrv:{}:measurement", model_name);
    
    if fields.is_empty() {
        // 获取所有字段
        let all_data: HashMap<String, String> = self.redis_client
            .hgetall(&hash_key)
            .await?;
        
        let mut result = HashMap::new();
        for (field, value) in all_data {
            if let Ok(parsed) = value.parse::<f64>() {
                result.insert(field, StandardFloat::new(parsed));
            }
        }
        Ok(result)
    } else {
        // 获取指定字段
        let values: Vec<Option<String>> = self.redis_client
            .hmget(&hash_key, &fields)
            .await?;
        
        let mut result = HashMap::new();
        for (field, value) in fields.iter().zip(values.iter()) {
            if let Some(val) = value {
                if let Ok(parsed) = val.parse::<f64>() {
                    result.insert(field.to_string(), StandardFloat::new(parsed));
                }
            }
        }
        Ok(result)
    }
}
```

## 写入接口

### 存储计算结果

```rust
pub async fn store_calculation_results(
    &mut self,
    model_name: &str,
    results: HashMap<String, StandardFloat>,
) -> Result<()> {
    let hash_key = format!("modsrv:{}:measurement", model_name);
    
    // 使用 pipeline 批量写入
    let mut pipe = redis::pipe();
    pipe.atomic();
    
    for (field, value) in results {
        // 仅存储值，不存储时间戳
        pipe.hset(&hash_key, field, value.to_redis());
    }
    
    pipe.query_async(&mut self.conn).await?;
    Ok(())
}
```

### 更新控制状态

```rust
pub async fn update_control_state(
    &mut self,
    model_name: &str,
    field: &str,
    value: StandardFloat,
) -> Result<()> {
    let hash_key = format!("modsrv:{}:control", model_name);
    
    self.redis_client
        .hset(&hash_key, field, value.to_redis())
        .await?;
    
    // 可选：发布状态变化通知
    let channel = format!("modsrv:{}:update", model_name);
    let message = format!("{}:{}", field, value.to_redis());
    self.redis_client.publish(&channel, message).await?;
    
    Ok(())
}
```

## 订阅机制

### 订阅 comsrv 数据

```rust
pub async fn subscribe_comsrv_updates(
    &self,
    patterns: Vec<String>,
) -> Result<()> {
    let mut pubsub = self.redis_client.get_async_pubsub().await?;
    
    // 订阅多个模式
    for pattern in patterns {
        pubsub.psubscribe(&pattern).await?;
    }
    
    // 处理消息
    while let Some(msg) = pubsub.on_message().next().await {
        let channel: String = msg.get_channel_name()?;
        let payload: String = msg.get_payload()?;
        
        // 解析 comsrv 消息格式: "pointID:value"
        if let Some((point_id, value)) = payload.split_once(':') {
            let point_id: u32 = point_id.parse()?;
            let value: f64 = value.parse()?;
            
            // 触发相关计算
            self.handle_data_update(&channel, point_id, value).await?;
        }
    }
    
    Ok(())
}
```

### 发布计算结果（可选）

```rust
pub async fn publish_calculation_update(
    &mut self,
    model_name: &str,
    field: &str,
    value: StandardFloat,
) -> Result<()> {
    let channel = format!("modsrv:{}:update", model_name);
    let message = format!("{}:{}", field, value.to_redis());
    
    self.redis_client.publish(&channel, message).await?;
    Ok(())
}
```

## 批量操作优化

### 批量读取优化

```rust
pub struct BatchReader {
    redis_client: Arc<RedisClient>,
    cache: Arc<RwLock<HashMap<String, CachedData>>>,
}

impl BatchReader {
    pub async fn batch_read_models(
        &self,
        requests: Vec<ModelDataRequest>,
    ) -> Result<Vec<ModelData>> {
        // 按模型分组
        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
        for req in requests {
            grouped.entry(req.model_name.clone())
                .or_insert_with(Vec::new)
                .extend(req.fields);
        }
        
        // 并发读取
        let tasks: Vec<_> = grouped.into_iter()
            .map(|(model, fields)| {
                let client = self.redis_client.clone();
                tokio::spawn(async move {
                    read_model_data(client, &model, fields).await
                })
            })
            .collect();
        
        // 收集结果
        let results = futures::future::join_all(tasks).await;
        
        // 处理结果...
        Ok(vec![])
    }
}
```

### 批量写入优化

```rust
pub struct BatchWriter {
    buffer: Arc<Mutex<Vec<WriteOperation>>>,
    flush_interval: Duration,
    batch_size: usize,
}

impl BatchWriter {
    pub async fn write(&self, model: String, field: String, value: StandardFloat) {
        let op = WriteOperation { model, field, value };
        
        let should_flush = {
            let mut buffer = self.buffer.lock().await;
            buffer.push(op);
            buffer.len() >= self.batch_size
        };
        
        if should_flush {
            self.flush().await;
        }
    }
    
    async fn flush(&self) {
        let ops = {
            let mut buffer = self.buffer.lock().await;
            std::mem::take(&mut *buffer)
        };
        
        if ops.is_empty() {
            return;
        }
        
        // 按模型分组
        let mut grouped: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for op in ops {
            let hash_key = format!("modsrv:{}:measurement", op.model);
            grouped.entry(hash_key)
                .or_insert_with(Vec::new)
                .push((op.field, op.value.to_redis()));
        }
        
        // 批量写入
        let mut pipe = redis::pipe();
        for (key, fields) in grouped {
            for (field, value) in fields {
                pipe.hset(&key, field, value);
            }
        }
        
        pipe.query_async(&mut self.redis_client.get_connection()).await.ok();
    }
}
```

## 性能考虑

### 1. 无时间戳的优势

- 减少存储空间（每个值节省约 13 字节）
- 简化数据结构，提高解析速度
- 避免时间同步问题

### 2. Hash 结构优势

- O(1) 字段访问
- 批量操作效率高
- 内存使用优化

### 3. 建议的优化策略

```rust
// 使用 pipeline 减少往返
let mut pipe = redis::pipe();
pipe.atomic();

// 批量设置多个字段
for (field, value) in updates {
    pipe.hset(&hash_key, field, value.to_redis());
}

// 一次执行
pipe.query_async(&mut conn).await?;
```

## 监控和调试

### Redis 命令示例

```bash
# 查看所有 modsrv 模型
redis-cli --scan --pattern "modsrv:*"

# 查看特定模型的测量值
redis-cli hgetall "modsrv:power_meter:measurement"

# 获取单个字段
redis-cli hget "modsrv:power_meter:measurement" "total_power"

# 监控实时变化
redis-cli monitor | grep modsrv

# 查看 Hash 大小
redis-cli hlen "modsrv:power_meter:measurement"

# 批量获取多个字段
redis-cli hmget "modsrv:power_meter:measurement" "total_power" "power_factor"
```

### 性能分析

```bash
# 查看内存使用
redis-cli memory usage "modsrv:power_meter:measurement"

# 分析 Hash 结构
redis-cli --bigkeys

# 慢查询日志
redis-cli slowlog get 10
```

## 错误处理

### 数据验证

```rust
fn validate_calculation_result(
    field: &str,
    value: f64,
) -> Result<StandardFloat> {
    // 检查 NaN 和无穷大
    if !value.is_finite() {
        return Err(Error::InvalidValue(format!(
            "Non-finite value for {}: {}",
            field, value
        )));
    }
    
    // 检查合理范围（根据具体场景调整）
    match field {
        "power_factor" => {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::OutOfRange(field.to_string()));
            }
        }
        "efficiency" => {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::OutOfRange(field.to_string()));
            }
        }
        _ => {
            // 通用范围检查
            if value.abs() > 1e9 {
                return Err(Error::ValueTooLarge(field.to_string()));
            }
        }
    }
    
    Ok(StandardFloat::new(value))
}
```

### 容错处理

```rust
// 读取失败时使用默认值
pub async fn get_value_or_default(
    &mut self,
    model: &str,
    field: &str,
    default: f64,
) -> StandardFloat {
    let hash_key = format!("modsrv:{}:measurement", model);
    
    match self.redis_client.hget(&hash_key, field).await {
        Ok(Some(value)) => {
            value.parse::<f64>()
                .map(StandardFloat::new)
                .unwrap_or_else(|_| StandardFloat::new(default))
        }
        _ => StandardFloat::new(default),
    }
}
```