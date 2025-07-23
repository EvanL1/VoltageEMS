# hissrv 架构设计

## 概述

hissrv 采用事件驱动架构，通过订阅 Redis Hash 键空间通知实现实时数据采集，并使用批处理机制优化 InfluxDB 写入性能。服务设计强调高可用性、可扩展性和数据完整性。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      hissrv                             │
├─────────────────────────────────────────────────────────┤
│                   API Server                            │
│              (Health/Stats/Config)                      │
├─────────────────────────────────────────────────────────┤
│                 Subscription Manager                    │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Keyspace     │ Pattern      │ Connection   │    │
│     │ Monitor      │ Matcher      │ Manager      │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                  Data Processor                         │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Parser    │Filter    │Transform │Validator │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
├─────────────────────────────────────────────────────────┤
│                  Batch Manager                          │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Buffer       │ Timer        │ Writer       │    │
│     │ Management   │ Control      │ Pool         │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│              External Connections                       │
│     ┌──────────────────┬──────────────────┐          │
│     │   Redis Client   │ InfluxDB Client  │          │
│     └──────────────────┴──────────────────┘          │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Subscription Manager

负责管理 Redis 订阅和连接：

```rust
pub struct SubscriptionManager {
    redis_client: Arc<RedisClient>,
    patterns: Vec<String>,
    handlers: Arc<RwLock<HashMap<String, Box<dyn EventHandler>>>>,
    reconnect_policy: ReconnectPolicy,
}

impl SubscriptionManager {
    pub async fn start(&self) -> Result<()> {
        // 配置键空间通知
        self.configure_keyspace_events().await?;
        
        // 订阅模式
        for pattern in &self.patterns {
            self.subscribe_pattern(pattern).await?;
        }
        
        // 启动事件循环
        self.run_event_loop().await
    }
    
    async fn configure_keyspace_events(&self) -> Result<()> {
        // 设置 Redis 键空间通知
        self.redis_client
            .config_set("notify-keyspace-events", "Kh")
            .await?;
        Ok(())
    }
}
```

### 2. Data Processor

处理接收到的键空间事件：

```rust
pub struct DataProcessor {
    filter_manager: Arc<FilterManager>,
    transformer: Arc<DataTransformer>,
    validator: Arc<DataValidator>,
}

impl DataProcessor {
    pub async fn process_event(
        &self,
        event: KeyspaceEvent,
    ) -> Result<Vec<DataPoint>> {
        // 1. 解析事件
        let key_info = self.parse_key(&event.key)?;
        
        // 2. 检查过滤规则
        if !self.filter_manager.should_process(&key_info)? {
            return Ok(vec![]);
        }
        
        // 3. 读取 Hash 数据
        let hash_data = self.read_hash_data(&event.key).await?;
        
        // 4. 转换为数据点
        let mut data_points = Vec::new();
        for (field, value) in hash_data {
            if let Ok(point) = self.transform_to_datapoint(
                &key_info,
                &field,
                &value,
            ) {
                if self.validator.validate(&point)? {
                    data_points.push(point);
                }
            }
        }
        
        Ok(data_points)
    }
    
    fn transform_to_datapoint(
        &self,
        key_info: &KeyInfo,
        field: &str,
        value: &str,
    ) -> Result<DataPoint> {
        let parsed_value = value.parse::<f64>()?;
        
        Ok(DataPoint {
            measurement: "telemetry",
            tags: hashmap! {
                "channel_id" => key_info.channel_id.to_string(),
                "point_id" => field.to_string(),
                "point_type" => key_info.point_type.clone(),
            },
            fields: hashmap! {
                "value" => FieldValue::Float(parsed_value),
            },
            timestamp: Some(Utc::now()),
        })
    }
}
```

### 3. Batch Manager

管理批量写入逻辑：

```rust
pub struct BatchManager {
    buffer: Arc<Mutex<Vec<DataPoint>>>,
    influxdb_client: Arc<InfluxDBClient>,
    config: BatchConfig,
    flush_timer: Arc<Mutex<Option<JoinHandle<()>>>>,
}

pub struct BatchConfig {
    pub max_size: usize,
    pub flush_interval: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

impl BatchManager {
    pub async fn add_points(&self, points: Vec<DataPoint>) -> Result<()> {
        let mut buffer = self.buffer.lock().await;
        buffer.extend(points);
        
        // 检查是否需要立即刷新
        if buffer.len() >= self.config.max_size {
            drop(buffer);
            self.flush_immediate().await?;
        } else {
            // 重置定时器
            self.reset_flush_timer().await;
        }
        
        Ok(())
    }
    
    async fn flush_immediate(&self) -> Result<()> {
        // 取消定时器
        if let Some(timer) = self.flush_timer.lock().await.take() {
            timer.abort();
        }
        
        // 执行刷新
        self.flush_buffer().await
    }
    
    async fn flush_buffer(&self) -> Result<()> {
        let points = {
            let mut buffer = self.buffer.lock().await;
            std::mem::take(&mut *buffer)
        };
        
        if points.is_empty() {
            return Ok(());
        }
        
        // 批量写入 InfluxDB
        match self.write_with_retry(points).await {
            Ok(_) => {
                metrics::counter!("hissrv_points_written_total")
                    .increment(points.len() as u64);
            }
            Err(e) => {
                metrics::counter!("hissrv_write_errors_total").increment(1);
                error!("Failed to write batch: {}", e);
                // 可选：将失败的点重新加入缓冲区
            }
        }
        
        Ok(())
    }
    
    async fn write_with_retry(&self, points: Vec<DataPoint>) -> Result<()> {
        let mut attempts = 0;
        let mut delay = self.config.retry_delay;
        
        loop {
            match self.influxdb_client.write_points(&points).await {
                Ok(_) => return Ok(()),
                Err(e) if attempts < self.config.retry_attempts => {
                    attempts += 1;
                    warn!(
                        "Write failed (attempt {}/{}): {}", 
                        attempts, self.config.retry_attempts, e
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // 指数退避
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

## 数据流

### 订阅流程

1. **键空间通知配置**
   ```bash
   CONFIG SET notify-keyspace-events Kh
   ```

2. **模式订阅**
   ```
   PSUBSCRIBE __keyspace@0__:comsrv:*:m
   PSUBSCRIBE __keyspace@0__:comsrv:*:s
   PSUBSCRIBE __keyspace@0__:modsrv:*:measurement
   ```

3. **事件处理**
   - 接收事件：`__keyspace@0__:comsrv:1001:m` → `hset`
   - 解析键名获取通道和类型
   - 读取完整 Hash 数据
   - 转换并批量写入

### Hash 数据读取

```rust
async fn read_hash_data(
    &self,
    key: &str,
) -> Result<HashMap<String, String>> {
    // 使用 HGETALL 读取所有字段
    let data: HashMap<String, String> = self.redis_client
        .hgetall(key)
        .await?;
    
    Ok(data)
}
```

## 过滤系统

### 多级过滤

```rust
pub struct FilterManager {
    channel_rules: HashMap<u16, ChannelRule>,
    point_rules: HashMap<(u16, u32), PointRule>,
    value_filters: Vec<Box<dyn ValueFilter>>,
    time_filters: Vec<Box<dyn TimeFilter>>,
}

impl FilterManager {
    pub fn should_process(&self, key_info: &KeyInfo) -> bool {
        // 1. 检查通道级规则
        if let Some(rule) = self.channel_rules.get(&key_info.channel_id) {
            if !rule.enabled {
                return false;
            }
            if !rule.point_types.contains(&key_info.point_type) {
                return false;
            }
        }
        
        // 2. 检查点位级规则
        let point_key = (key_info.channel_id, key_info.point_id);
        if let Some(rule) = self.point_rules.get(&point_key) {
            if !rule.enabled {
                return false;
            }
        }
        
        true
    }
    
    pub fn filter_value(&self, value: f64, point_type: &str) -> bool {
        for filter in &self.value_filters {
            if !filter.check(value, point_type) {
                return false;
            }
        }
        true
    }
}
```

### 时间间隔过滤

```rust
pub struct TimeIntervalFilter {
    last_update: Arc<RwLock<HashMap<String, Instant>>>,
    min_interval: Duration,
}

impl TimeFilter for TimeIntervalFilter {
    fn check(&self, key: &str) -> bool {
        let mut last_update = self.last_update.write().unwrap();
        
        match last_update.get(key) {
            Some(last_time) => {
                if last_time.elapsed() < self.min_interval {
                    false
                } else {
                    last_update.insert(key.to_string(), Instant::now());
                    true
                }
            }
            None => {
                last_update.insert(key.to_string(), Instant::now());
                true
            }
        }
    }
}
```

## 高可用性设计

### 自动重连

```rust
pub struct ReconnectPolicy {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    exponential_base: f64,
}

async fn maintain_connection(&self) {
    loop {
        if !self.is_connected() {
            match self.reconnect().await {
                Ok(_) => {
                    info!("Reconnected successfully");
                    self.resubscribe_all().await.ok();
                }
                Err(e) => {
                    error!("Reconnection failed: {}", e);
                }
            }
        }
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

### 数据缓冲

```rust
pub struct PersistentBuffer {
    memory_buffer: Arc<Mutex<VecDeque<DataPoint>>>,
    disk_buffer: Option<DiskBuffer>,
    max_memory_size: usize,
}

impl PersistentBuffer {
    async fn add(&self, points: Vec<DataPoint>) -> Result<()> {
        let mut buffer = self.memory_buffer.lock().await;
        
        if buffer.len() + points.len() > self.max_memory_size {
            // 溢出到磁盘
            if let Some(disk) = &self.disk_buffer {
                disk.write(&points).await?;
                return Ok(());
            }
        }
        
        buffer.extend(points);
        Ok(())
    }
}
```

## 性能优化

### 1. 并发处理

```rust
// 并发处理多个 Hash 键
let handles: Vec<_> = events.into_iter()
    .map(|event| {
        let processor = self.processor.clone();
        tokio::spawn(async move {
            processor.process_event(event).await
        })
    })
    .collect();

let results = futures::future::join_all(handles).await;
```

### 2. 连接池

```rust
pub struct ConnectionPool {
    redis_pool: deadpool_redis::Pool,
    influxdb_pool: Arc<InfluxDBPool>,
}
```

### 3. 内存管理

- 使用环形缓冲区限制内存使用
- 定期清理过期的时间过滤记录
- 批量大小动态调整

## 监控指标

```rust
pub struct Metrics {
    messages_received: IntCounter,
    points_processed: IntCounter,
    points_filtered: IntCounter,
    points_written: IntCounter,
    batch_size: Histogram,
    write_duration: Histogram,
    errors: IntCounterVec,
}
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum HissrvError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("InfluxDB error: {0}")]
    InfluxDB(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Filter error: {0}")]
    Filter(String),
}
```

## 配置热重载

```rust
pub async fn reload_config(&self) -> Result<()> {
    let new_config = load_config_from_file(&self.config_path)?;
    
    // 更新过滤规则
    self.filter_manager.update_rules(new_config.rules)?;
    
    // 更新批处理参数
    self.batch_manager.update_config(new_config.batch)?;
    
    info!("Configuration reloaded successfully");
    Ok(())
}
```