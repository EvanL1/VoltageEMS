# modsrv Redis 接口设计

## 概述

modsrv 通过优化的 Redis 接口实现与 comsrv 的数据交互，包括实时数据读取、控制命令发送和计算结果发布。接口设计强调高性能、低延迟和可靠性。

## 数据读取接口

### ComsrvInterface 结构

```rust
pub struct ComsrvInterface {
    redis_client: Arc<RedisClient>,
    cache_manager: Arc<ModelCacheManager>,
    batch_size: usize,
    timeout: Duration,
}

impl ComsrvInterface {
    /// 创建接口实例
    pub fn new(
        redis_client: Arc<RedisClient>,
        cache_manager: Arc<ModelCacheManager>,
    ) -> Self {
        Self {
            redis_client,
            cache_manager,
            batch_size: 1000,
            timeout: Duration::from_secs(5),
        }
    }
}
```

### 单点读取

```rust
/// 读取单个点位数据
pub async fn read_point(
    &self,
    channel_id: u16,
    point_type: &str,
    point_id: u32,
) -> Result<Option<PointData>> {
    let key = format!("{}:{}:{}", channel_id, point_type, point_id);
    
    // 1. 尝试从缓存获取
    if let Some(cached) = self.cache_manager.get_point(&key).await {
        return Ok(Some(cached));
    }
    
    // 2. 从 Redis 读取
    let value: Option<String> = self.redis_client
        .get(&key)
        .await
        .map_err(|e| ModelSrvError::Redis(e.to_string()))?;
    
    // 3. 解析数据
    match value {
        Some(data) => {
            let point_data = self.parse_point_data(&key, &data)?;
            
            // 4. 更新缓存
            self.cache_manager.put_point(&key, point_data.clone()).await;
            
            Ok(Some(point_data))
        }
        None => Ok(None),
    }
}
```

### 批量读取

```rust
/// 批量读取点位数据
pub async fn batch_read_points(
    &self,
    requests: Vec<PointReadRequest>,
) -> Result<Vec<PointData>> {
    // 1. 分离缓存命中和未命中
    let mut results = Vec::with_capacity(requests.len());
    let mut cache_misses = Vec::new();
    let mut miss_indices = Vec::new();
    
    for (idx, req) in requests.iter().enumerate() {
        let key = req.to_key();
        if let Some(cached) = self.cache_manager.get_point(&key).await {
            results.push(Some(cached));
        } else {
            results.push(None);
            cache_misses.push(key);
            miss_indices.push(idx);
        }
    }
    
    // 2. 批量从 Redis 读取未命中数据
    if !cache_misses.is_empty() {
        let values: Vec<Option<String>> = self.redis_client
            .mget(&cache_misses)
            .await?;
        
        // 3. 处理读取结果
        for (i, value) in values.into_iter().enumerate() {
            if let Some(data) = value {
                let key = &cache_misses[i];
                let point_data = self.parse_point_data(key, &data)?;
                
                // 更新缓存
                self.cache_manager.put_point(key, point_data.clone()).await;
                
                // 填充结果
                let result_idx = miss_indices[i];
                results[result_idx] = Some(point_data);
            }
        }
    }
    
    // 4. 过滤并返回有效数据
    Ok(results.into_iter().flatten().collect())
}
```

### 数据解析

```rust
/// 解析点位数据
fn parse_point_data(&self, key: &str, data: &str) -> Result<PointData> {
    // 解析键名
    let parts: Vec<&str> = key.split(':').collect();
    if parts.len() != 3 {
        return Err(ModelSrvError::InvalidFormat(
            format!("Invalid key format: {}", key)
        ));
    }
    
    let channel_id = parts[0].parse::<u16>()?;
    let point_type = parts[1];
    let point_id = parts[2].parse::<u32>()?;
    
    // 解析值
    let value_parts: Vec<&str> = data.split(':').collect();
    if value_parts.len() != 2 {
        return Err(ModelSrvError::InvalidFormat(
            format!("Invalid data format: {}", data)
        ));
    }
    
    let value = value_parts[0].parse::<f64>()?;
    let timestamp = value_parts[1].parse::<i64>()?;
    
    Ok(PointData {
        channel_id,
        point_type: point_type.to_string(),
        point_id,
        value,
        timestamp,
        quality: Quality::Good,
    })
}
```

## 控制命令接口

### ControlSender 结构

```rust
pub struct ControlSender {
    redis_client: Arc<RedisClient>,
    pending_commands: Arc<RwLock<HashMap<String, PendingCommand>>>,
    retry_config: RetryConfig,
}

pub struct RetryConfig {
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
}
```

### 发送控制命令

```rust
/// 发送控制命令
pub async fn send_control_command(
    &self,
    channel_id: u16,
    point_id: u32,
    value: f64,
    priority: CommandPriority,
) -> Result<String> {
    let command_id = Uuid::new_v4().to_string();
    
    let command = ControlCommand {
        id: command_id.clone(),
        channel_id,
        point_id,
        value,
        priority,
        timestamp: Utc::now().timestamp_millis(),
        source: "modsrv".to_string(),
    };
    
    // 1. 验证命令
    self.validate_command(&command)?;
    
    // 2. 记录待确认命令
    self.track_pending_command(&command).await;
    
    // 3. 发布到控制通道
    let channel = format!("cmd:{}:control", channel_id);
    let message = serde_json::to_string(&command)?;
    
    self.redis_client
        .publish(&channel, &message)
        .await
        .map_err(|e| ModelSrvError::Redis(e.to_string()))?;
    
    // 4. 启动确认监控
    self.monitor_command_confirmation(command_id.clone());
    
    Ok(command_id)
}
```

### 命令确认机制

```rust
/// 监控命令确认
fn monitor_command_confirmation(&self, command_id: String) {
    let pending = self.pending_commands.clone();
    let retry_config = self.retry_config.clone();
    let redis_client = self.redis_client.clone();
    
    tokio::spawn(async move {
        let mut delay = retry_config.initial_delay;
        let mut retries = 0;
        
        loop {
            tokio::time::sleep(delay).await;
            
            // 检查命令状态
            let should_retry = {
                let commands = pending.read().await;
                commands.get(&command_id)
                    .map(|cmd| cmd.status == CommandStatus::Pending)
                    .unwrap_or(false)
            };
            
            if !should_retry {
                break;
            }
            
            if retries >= retry_config.max_retries {
                error!("Command {} failed after {} retries", command_id, retries);
                pending.write().await.remove(&command_id);
                break;
            }
            
            // 重试发送
            if let Some(cmd) = pending.read().await.get(&command_id).cloned() {
                let channel = format!("cmd:{}:control", cmd.command.channel_id);
                let _ = redis_client
                    .publish(&channel, &serde_json::to_string(&cmd.command).unwrap())
                    .await;
                
                retries += 1;
                delay = (delay.as_secs_f64() * retry_config.multiplier)
                    .min(retry_config.max_delay.as_secs_f64());
                delay = Duration::from_secs_f64(delay.as_secs_f64());
            }
        }
    });
}
```

### 批量命令发送

```rust
/// 批量发送控制命令
pub async fn batch_send_commands(
    &self,
    commands: Vec<ControlRequest>,
) -> Result<Vec<Result<String>>> {
    let mut results = Vec::with_capacity(commands.len());
    
    // 按通道分组
    let mut channel_groups: HashMap<u16, Vec<ControlRequest>> = HashMap::new();
    for cmd in commands {
        channel_groups.entry(cmd.channel_id)
            .or_insert_with(Vec::new)
            .push(cmd);
    }
    
    // 并发发送每个通道的命令
    let tasks: Vec<_> = channel_groups.into_iter()
        .map(|(channel_id, cmds)| {
            let sender = self.clone();
            tokio::spawn(async move {
                let mut channel_results = Vec::new();
                for cmd in cmds {
                    let result = sender.send_control_command(
                        cmd.channel_id,
                        cmd.point_id,
                        cmd.value,
                        cmd.priority.unwrap_or(CommandPriority::Normal),
                    ).await;
                    channel_results.push(result);
                }
                channel_results
            })
        })
        .collect();
    
    // 收集结果
    for task in tasks {
        let channel_results = task.await?;
        results.extend(channel_results);
    }
    
    Ok(results)
}
```

## 数据订阅接口

### 订阅管理

```rust
pub struct SubscriptionManager {
    redis_client: Arc<RedisClient>,
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    handlers: Arc<RwLock<HashMap<String, Box<dyn UpdateHandler>>>>,
}

#[async_trait]
pub trait UpdateHandler: Send + Sync {
    async fn handle_update(&self, update: PointUpdate) -> Result<()>;
}
```

### 订阅点位更新

```rust
/// 订阅点位更新
pub async fn subscribe_points(
    &self,
    patterns: Vec<String>,
    handler: Box<dyn UpdateHandler>,
) -> Result<String> {
    let subscription_id = Uuid::new_v4().to_string();
    
    // 1. 创建订阅
    let subscription = Subscription {
        id: subscription_id.clone(),
        patterns: patterns.clone(),
        created_at: Utc::now(),
        is_active: true,
    };
    
    // 2. 注册处理器
    self.handlers.write().await
        .insert(subscription_id.clone(), handler);
    
    // 3. 启动订阅任务
    let manager = self.clone();
    let sub_id = subscription_id.clone();
    tokio::spawn(async move {
        if let Err(e) = manager.run_subscription(sub_id, patterns).await {
            error!("Subscription error: {}", e);
        }
    });
    
    // 4. 记录订阅
    self.subscriptions.write().await
        .insert(subscription_id.clone(), subscription);
    
    Ok(subscription_id)
}
```

### 处理更新消息

```rust
/// 运行订阅循环
async fn run_subscription(
    &self,
    subscription_id: String,
    patterns: Vec<String>,
) -> Result<()> {
    let mut pubsub = self.redis_client.get_async_pubsub().await?;
    
    // 订阅模式
    for pattern in &patterns {
        pubsub.psubscribe(pattern).await?;
    }
    
    // 消息处理循环
    while let Some(msg) = pubsub.on_message().next().await {
        // 检查订阅是否仍然活跃
        let is_active = self.subscriptions.read().await
            .get(&subscription_id)
            .map(|s| s.is_active)
            .unwrap_or(false);
        
        if !is_active {
            break;
        }
        
        // 解析消息
        if let Ok(update) = self.parse_update_message(&msg) {
            // 调用处理器
            if let Some(handler) = self.handlers.read().await.get(&subscription_id) {
                if let Err(e) = handler.handle_update(update).await {
                    error!("Handler error: {}", e);
                }
            }
        }
    }
    
    Ok(())
}
```

## 缓存策略

### 多级缓存

```rust
pub struct ModelCacheManager {
    /// L1: 进程内缓存
    point_cache: Arc<RwLock<HashMap<String, CacheEntry<PointData>>>>,
    
    /// L2: 计算结果缓存
    result_cache: Arc<RwLock<HashMap<String, CacheEntry<Value>>>>,
    
    /// 配置
    config: CacheConfig,
    
    /// 统计信息
    stats: Arc<RwLock<CacheStats>>,
}

pub struct CacheConfig {
    /// 最大缓存条目数
    max_entries: usize,
    
    /// 默认 TTL
    default_ttl: Duration,
    
    /// 清理间隔
    cleanup_interval: Duration,
    
    /// LRU 淘汰阈值
    eviction_threshold: f64,
}
```

### 缓存操作

```rust
impl ModelCacheManager {
    /// 获取缓存数据
    pub async fn get_point(&self, key: &str) -> Option<PointData> {
        let cache = self.point_cache.read().await;
        
        cache.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                // 更新访问计数
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                entry.last_access.store(
                    Instant::now().elapsed().as_secs(),
                    Ordering::Relaxed
                );
                
                Some(entry.value.clone())
            }
        })
    }
    
    /// 缓存数据
    pub async fn put_point(&self, key: &str, value: PointData) {
        let mut cache = self.point_cache.write().await;
        
        // 检查容量
        if cache.len() >= self.config.max_entries {
            self.evict_lru(&mut cache);
        }
        
        // 插入新条目
        cache.insert(key.to_string(), CacheEntry {
            value,
            expires_at: Instant::now() + self.config.default_ttl,
            access_count: AtomicU64::new(0),
            last_access: AtomicU64::new(0),
        });
        
        // 更新统计
        self.stats.write().await.total_entries = cache.len();
    }
    
    /// LRU 淘汰
    fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry<PointData>>) {
        let target_size = (self.config.max_entries as f64 * self.config.eviction_threshold) as usize;
        
        // 收集并排序条目
        let mut entries: Vec<_> = cache.iter()
            .map(|(k, v)| (k.clone(), v.last_access.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|e| e.1);
        
        // 删除最少使用的条目
        let to_remove = cache.len() - target_size;
        for (key, _) in entries.iter().take(to_remove) {
            cache.remove(key);
        }
    }
}
```

### 预热策略

```rust
/// 缓存预热
pub async fn warm_cache(&self, patterns: Vec<String>) -> Result<()> {
    for pattern in patterns {
        // 扫描匹配的键
        let keys = self.scan_keys(&pattern).await?;
        
        // 批量加载
        for chunk in keys.chunks(100) {
            let values = self.redis_client.mget(chunk).await?;
            
            for (key, value) in chunk.iter().zip(values.iter()) {
                if let Some(data) = value {
                    if let Ok(point_data) = self.parse_point_data(key, data) {
                        self.cache_manager.put_point(key, point_data).await;
                    }
                }
            }
        }
    }
    
    Ok(())
}
```

## 性能优化

### 1. 连接池配置

```rust
pub struct RedisClientConfig {
    /// 连接池大小
    pool_size: u32,
    
    /// 连接超时
    connection_timeout: Duration,
    
    /// 命令超时
    command_timeout: Duration,
    
    /// 重试策略
    retry_policy: RetryPolicy,
}
```

### 2. 批量操作优化

```rust
/// 智能批处理
pub struct BatchProcessor {
    batch_size: usize,
    flush_interval: Duration,
    buffer: Arc<Mutex<Vec<PointReadRequest>>>,
}

impl BatchProcessor {
    pub async fn process(&self, request: PointReadRequest) -> Result<PointData> {
        // 添加到缓冲区
        let should_flush = {
            let mut buffer = self.buffer.lock().await;
            buffer.push(request);
            buffer.len() >= self.batch_size
        };
        
        if should_flush {
            self.flush().await?;
        }
        
        // 等待结果
        self.wait_for_result(request.id).await
    }
}
```

### 3. 并发控制

```rust
/// 并发限制
pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl ConcurrencyLimiter {
    pub async fn execute<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let _permit = self.semaphore.acquire().await?;
        f.await
    }
}
```

## 监控指标

### 接口统计

```rust
#[derive(Debug, Clone)]
pub struct InterfaceMetrics {
    /// 读取请求数
    pub read_requests: AtomicU64,
    
    /// 缓存命中数
    pub cache_hits: AtomicU64,
    
    /// 命令发送数
    pub commands_sent: AtomicU64,
    
    /// 错误计数
    pub errors: AtomicU64,
    
    /// 平均延迟
    pub avg_latency_ms: AtomicU64,
}
```

### 指标收集

```rust
// 请求延迟
let start = Instant::now();
let result = self.read_point(channel_id, point_type, point_id).await;
let duration = start.elapsed();

metrics::histogram!("modsrv.redis.read_latency", duration.as_secs_f64());

// 缓存命中率
let hit_rate = cache_hits as f64 / total_requests as f64;
metrics::gauge!("modsrv.cache.hit_rate", hit_rate);

// 命令队列长度
let pending_count = self.pending_commands.read().await.len();
metrics::gauge!("modsrv.commands.pending", pending_count as f64);
```

## 错误处理

### 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    #[error("Redis error: {0}")]
    Redis(String),
    
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
    
    #[error("Command rejected: {0}")]
    CommandRejected(String),
}
```

### 降级策略

```rust
/// 降级读取
async fn read_with_fallback(
    &self,
    key: &str,
) -> Result<Option<PointData>> {
    // 1. 尝试从缓存读取
    if let Some(cached) = self.cache_manager.get_point(key).await {
        return Ok(Some(cached));
    }
    
    // 2. 尝试从 Redis 读取
    match timeout(self.timeout, self.redis_client.get(key)).await {
        Ok(Ok(Some(data))) => {
            // 解析并缓存
            let point_data = self.parse_point_data(key, &data)?;
            self.cache_manager.put_point(key, point_data.clone()).await;
            Ok(Some(point_data))
        }
        Ok(Ok(None)) => Ok(None),
        Ok(Err(e)) => {
            warn!("Redis error, using stale cache: {}", e);
            // 3. 使用过期缓存
            Ok(self.cache_manager.get_stale(key).await)
        }
        Err(_) => {
            warn!("Redis timeout, using stale cache");
            // 4. 超时使用过期缓存
            Ok(self.cache_manager.get_stale(key).await)
        }
    }
}
```