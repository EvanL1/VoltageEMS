# netsrv 架构设计

## 概述

netsrv 采用插件化架构设计，将数据源、格式化器和协议适配器解耦，实现灵活的数据转发能力。服务从 Redis 读取系统数据，经过过滤、格式化和批量处理后，通过多种协议推送到外部系统。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      netsrv                             │
├─────────────────────────────────────────────────────────┤
│                   API Server                            │
│            (Config/Status/Control)                      │
├─────────────────────────────────────────────────────────┤
│                 Data Pipeline                           │
│     ┌──────────┬──────────┬──────────┬──────────┐     │
│     │ Source   │ Filter   │Formatter │ Batcher  │     │
│     │ Manager  │ Engine   │ Registry │ Manager  │     │
│     └──────────┴──────────┴──────────┴──────────┘     │
├─────────────────────────────────────────────────────────┤
│                Protocol Adapters                        │
│     ┌──────────┬──────────┬──────────┬──────────┐     │
│     │  MQTT    │  HTTP    │ AWS IoT  │ Aliyun   │     │
│     │ Adapter  │ Adapter  │ Adapter  │   IoT    │     │
│     └──────────┴──────────┴──────────┴──────────┘     │
├─────────────────────────────────────────────────────────┤
│               Connection Manager                        │
│     ┌──────────┬──────────┬──────────┬──────────┐     │
│     │Connection│ Failover │ Health   │ Metrics  │     │
│     │  Pool    │ Manager  │ Monitor  │Collector │     │
│     └──────────┴──────────┴──────────┴──────────┘     │
├─────────────────────────────────────────────────────────┤
│                  Redis Client                           │
│          ┌──────────────┬──────────────┐               │
│          │ Key Monitor  │   Pub/Sub    │               │
│          └──────────────┴──────────────┘               │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Data Pipeline

数据管道是 netsrv 的核心，负责数据的全流程处理：

```rust
pub struct DataPipeline {
    source_manager: Arc<SourceManager>,
    filter_engine: Arc<FilterEngine>,
    formatter_registry: Arc<FormatterRegistry>,
    batcher_manager: Arc<BatcherManager>,
}

impl DataPipeline {
    pub async fn process(&self, config: &PipelineConfig) -> Result<()> {
        // 1. 从数据源读取
        let data_stream = self.source_manager.create_stream(config).await?;
        
        // 2. 过滤数据
        let filtered_stream = self.filter_engine.apply(data_stream, &config.filters);
        
        // 3. 格式化数据
        let formatter = self.formatter_registry.get(&config.format)?;
        let formatted_stream = filtered_stream.map(|data| formatter.format(data));
        
        // 4. 批量处理
        let batched_stream = self.batcher_manager.batch(formatted_stream, &config.batch);
        
        // 5. 发送到目标
        self.send_to_targets(batched_stream, &config.targets).await
    }
}
```

### 2. Source Manager

管理多个数据源的订阅和读取：

```rust
pub struct SourceManager {
    redis_client: Arc<RedisClient>,
    sources: HashMap<String, Box<dyn DataSource>>,
}

#[async_trait]
pub trait DataSource: Send + Sync {
    async fn create_stream(&self, redis: &RedisClient) -> Result<DataStream>;
}

pub struct ComsrvSource {
    patterns: Vec<String>,
}

#[async_trait]
impl DataSource for ComsrvSource {
    async fn create_stream(&self, redis: &RedisClient) -> Result<DataStream> {
        let mut streams = Vec::new();
        
        for pattern in &self.patterns {
            // 监控 Hash 键的变化
            let stream = self.monitor_hash_changes(redis, pattern).await?;
            streams.push(stream);
        }
        
        Ok(DataStream::merge(streams))
    }
    
    async fn monitor_hash_changes(
        &self,
        redis: &RedisClient,
        pattern: &str,
    ) -> Result<impl Stream<Item = DataPoint>> {
        // 使用 Redis keyspace notifications
        let mut pubsub = redis.get_async_pubsub().await?;
        pubsub.psubscribe(format!("__keyspace@0__:{}", pattern)).await?;
        
        Ok(stream! {
            while let Some(msg) = pubsub.on_message().next().await {
                let key = msg.get_channel_name()?;
                if let Some(data) = self.read_hash_data(redis, &key).await? {
                    yield data;
                }
            }
        })
    }
}
```

### 3. Filter Engine

灵活的数据过滤引擎：

```rust
pub struct FilterEngine {
    filters: Vec<Box<dyn DataFilter>>,
}

#[async_trait]
pub trait DataFilter: Send + Sync {
    fn matches(&self, data: &DataPoint) -> bool;
}

pub struct ChannelFilter {
    include: HashSet<u32>,
    exclude: HashSet<u32>,
}

impl DataFilter for ChannelFilter {
    fn matches(&self, data: &DataPoint) -> bool {
        if !self.include.is_empty() && !self.include.contains(&data.channel_id) {
            return false;
        }
        if self.exclude.contains(&data.channel_id) {
            return false;
        }
        true
    }
}

pub struct ValueRangeFilter {
    field: String,
    min: Option<f64>,
    max: Option<f64>,
}

impl DataFilter for ValueRangeFilter {
    fn matches(&self, data: &DataPoint) -> bool {
        match data.get_field(&self.field) {
            Some(value) => {
                if let Some(min) = self.min {
                    if value < min { return false; }
                }
                if let Some(max) = self.max {
                    if value > max { return false; }
                }
                true
            }
            None => false,
        }
    }
}
```

### 4. Formatter Registry

格式化器注册表和实现：

```rust
pub struct FormatterRegistry {
    formatters: HashMap<String, Box<dyn DataFormatter>>,
}

#[async_trait]
pub trait DataFormatter: Send + Sync {
    fn format(&self, data: &DataPoint) -> Result<Vec<u8>>;
    fn content_type(&self) -> &str;
}

pub struct JsonFormatter {
    config: JsonFormatConfig,
}

impl DataFormatter for JsonFormatter {
    fn format(&self, data: &DataPoint) -> Result<Vec<u8>> {
        let json_value = match &self.config.structure {
            JsonStructure::Standard => json!({
                "timestamp": data.timestamp.to_rfc3339(),
                "channel": data.channel_id,
                "type": data.data_type,
                "point": data.point_id,
                "value": format!("{:.6}", data.value),
            }),
            JsonStructure::Flat => {
                let key = format!("channel_{}_{}_{}", 
                    data.channel_id, data.data_type, data.point_id);
                json!({
                    "timestamp": data.timestamp.to_rfc3339(),
                    key: format!("{:.6}", data.value),
                })
            }
        };
        
        Ok(serde_json::to_vec(&json_value)?)
    }
    
    fn content_type(&self) -> &str {
        "application/json"
    }
}
```

### 5. Protocol Adapters

协议适配器的统一接口和实现：

```rust
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn send(&self, data: &[u8]) -> Result<()>;
    async fn send_batch(&self, batch: &[Vec<u8>]) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
}

pub struct MqttAdapter {
    client: Option<AsyncClient>,
    config: MqttConfig,
}

#[async_trait]
impl ProtocolAdapter for MqttAdapter {
    async fn connect(&mut self) -> Result<()> {
        let create_opts = CreateOptionsBuilder::new()
            .server_uri(&self.config.broker)
            .client_id(&self.config.client_id)
            .finalize();
            
        let client = AsyncClient::new(create_opts)?;
        
        let conn_opts = ConnectOptionsBuilder::new()
            .user_name(&self.config.username)
            .password(&self.config.password)
            .keep_alive_interval(Duration::from_secs(30))
            .clean_session(true)
            .finalize();
            
        client.connect(conn_opts).await?;
        self.client = Some(client);
        
        Ok(())
    }
    
    async fn send(&self, data: &[u8]) -> Result<()> {
        if let Some(client) = &self.client {
            let msg = MessageBuilder::new()
                .topic(&self.config.topic)
                .payload(data)
                .qos(self.config.qos)
                .retained(self.config.retain)
                .finalize();
                
            client.publish(msg).await?;
        }
        
        Ok(())
    }
}
```

### 6. Connection Manager

连接管理和故障恢复：

```rust
pub struct ConnectionManager {
    adapters: HashMap<String, AdapterWrapper>,
    health_monitor: Arc<HealthMonitor>,
    failover_manager: Arc<FailoverManager>,
}

pub struct AdapterWrapper {
    adapter: Arc<Mutex<Box<dyn ProtocolAdapter>>>,
    config: ConnectionConfig,
    status: Arc<AtomicU8>, // 0=断开, 1=连接中, 2=已连接
    last_error: Arc<Mutex<Option<String>>>,
}

impl ConnectionManager {
    pub async fn ensure_connected(&self, name: &str) -> Result<()> {
        let wrapper = self.adapters.get(name)
            .ok_or_else(|| Error::AdapterNotFound(name.to_string()))?;
            
        let status = wrapper.status.load(Ordering::Relaxed);
        
        if status != 2 {
            self.reconnect(wrapper).await?;
        }
        
        Ok(())
    }
    
    async fn reconnect(&self, wrapper: &AdapterWrapper) -> Result<()> {
        let mut attempt = 0;
        let config = &wrapper.config.reconnect;
        
        loop {
            attempt += 1;
            
            match wrapper.adapter.lock().await.connect().await {
                Ok(_) => {
                    wrapper.status.store(2, Ordering::Relaxed);
                    info!("Connected successfully after {} attempts", attempt);
                    return Ok(());
                }
                Err(e) => {
                    let delay = self.calculate_backoff(attempt, config);
                    
                    if config.max_attempts > 0 && attempt >= config.max_attempts {
                        return Err(Error::MaxRetriesExceeded);
                    }
                    
                    warn!("Connection failed (attempt {}): {}", attempt, e);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}
```

## 数据流程

### 1. 数据采集流程

```
Redis Hash 变化 → Keyspace Notification → Source Manager → Data Point
                                                ↓
                                          Filter Engine
                                                ↓
                                          Formatter Registry
                                                ↓
                                          Batcher Manager
                                                ↓
                                          Protocol Adapter → 外部系统
```

### 2. 批量处理流程

```rust
pub struct BatcherManager {
    batchers: HashMap<String, Batcher>,
}

pub struct Batcher {
    config: BatchConfig,
    buffer: Arc<Mutex<Vec<FormattedData>>>,
    last_flush: Arc<Mutex<Instant>>,
}

impl Batcher {
    pub async fn add(&self, data: FormattedData) -> Option<Vec<FormattedData>> {
        let mut buffer = self.buffer.lock().await;
        buffer.push(data);
        
        // 检查是否需要flush
        if self.should_flush(&buffer).await {
            let batch = std::mem::take(&mut *buffer);
            *self.last_flush.lock().await = Instant::now();
            Some(batch)
        } else {
            None
        }
    }
    
    async fn should_flush(&self, buffer: &[FormattedData]) -> bool {
        // 大小限制
        if buffer.len() >= self.config.max_batch_size {
            return true;
        }
        
        // 时间限制
        let elapsed = self.last_flush.lock().await.elapsed();
        if elapsed >= Duration::from_millis(self.config.max_wait_time_ms) {
            return true;
        }
        
        false
    }
}
```

## 性能优化

### 1. 零拷贝设计

```rust
pub struct ZeroCopyFormatter {
    buffer_pool: Arc<BufferPool>,
}

impl ZeroCopyFormatter {
    pub fn format_into(&self, data: &DataPoint, buf: &mut BytesMut) -> Result<()> {
        // 直接写入缓冲区，避免中间分配
        write!(buf, "{},{},{},{:.6}\n",
            data.timestamp.timestamp(),
            data.channel_id,
            data.point_id,
            data.value
        )?;
        
        Ok(())
    }
}
```

### 2. 并行处理

```rust
pub struct ParallelProcessor {
    workers: Vec<Worker>,
    dispatcher: Arc<Dispatcher>,
}

impl ParallelProcessor {
    pub async fn process_parallel(
        &self,
        data_stream: impl Stream<Item = DataPoint>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel(1000);
        
        // 分发数据到工作线程
        tokio::spawn(async move {
            data_stream
                .for_each_concurrent(None, |data| async {
                    tx.send(data).await.ok();
                })
                .await;
        });
        
        // 并行处理
        let handles: Vec<_> = self.workers.iter()
            .map(|worker| {
                let rx = rx.clone();
                tokio::spawn(worker.run(rx))
            })
            .collect();
            
        futures::future::join_all(handles).await;
        
        Ok(())
    }
}
```

### 3. 内存池

```rust
pub struct BufferPool {
    pool: Arc<Mutex<Vec<BytesMut>>>,
    buffer_size: usize,
}

impl BufferPool {
    pub async fn acquire(&self) -> BytesMut {
        let mut pool = self.pool.lock().await;
        
        pool.pop().unwrap_or_else(|| {
            BytesMut::with_capacity(self.buffer_size)
        })
    }
    
    pub async fn release(&self, mut buffer: BytesMut) {
        buffer.clear();
        
        let mut pool = self.pool.lock().await;
        if pool.len() < 100 {  // 限制池大小
            pool.push(buffer);
        }
    }
}
```

## 监控指标

```rust
pub struct Metrics {
    // 数据流指标
    data_points_received: IntCounter,
    data_points_filtered: IntCounter,
    data_points_sent: IntCounter,
    
    // 批量指标
    batches_created: IntCounter,
    batch_size: Histogram,
    batch_wait_time: Histogram,
    
    // 连接指标
    connection_attempts: IntCounterVec,
    connection_failures: IntCounterVec,
    connection_duration: GaugeVec,
    
    // 性能指标
    processing_duration: Histogram,
    formatting_duration: Histogram,
    send_duration: HistogramVec,
}
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum NetSrvError {
    #[error("Adapter not found: {0}")]
    AdapterNotFound(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Format error: {0}")]
    FormatError(String),
    
    #[error("Send failed: {0}")]
    SendFailed(String),
    
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
}
```

## 扩展性设计

### 添加新协议

```rust
// 1. 实现 ProtocolAdapter trait
pub struct CustomProtocolAdapter {
    // ... 自定义字段
}

#[async_trait]
impl ProtocolAdapter for CustomProtocolAdapter {
    // ... 实现方法
}

// 2. 注册适配器
adapter_registry.register("custom", Box::new(CustomProtocolAdapter::new));
```

### 添加新格式

```rust
// 1. 实现 DataFormatter trait
pub struct CustomFormatter {
    // ... 自定义配置
}

impl DataFormatter for CustomFormatter {
    // ... 实现方法
}

// 2. 注册格式化器
formatter_registry.register("custom", Box::new(CustomFormatter::new));
```