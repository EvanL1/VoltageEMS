# hissrv 架构设计

## 概述

hissrv（Historical Service）是 VoltageEMS 的历史数据服务，负责将 Redis 中的实时数据持久化到时序数据库 InfluxDB，提供历史数据查询、聚合分析和数据归档功能。

## 架构特点

1. **高效数据桥接**：优化的批量写入机制
2. **灵活的存储策略**：支持多种保留策略
3. **智能降采样**：自动数据聚合和压缩
4. **查询优化**：缓存热点查询结果
5. **故障恢复**：断点续传和数据补偿

## 系统架构图

```
┌────────────────────────────────────────────────────────────┐
│                         hissrv                              │
├────────────────────────────────────────────────────────────┤
│                    Data Bridge Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Redis Subscriber│ │Batch Buffer  │  │Write Manager │    │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │
│         └──────────────────┴──────────────────┘            │
│                            │                                │
│                     Storage Layer                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │InfluxDB Writer│ │Retention Mgr │  │Downsampler   │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
│                            │                                │
│                      Query Layer                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Query Engine  │  │Aggregator    │  │Cache Manager │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└────────────────────────────────────────────────────────────┘
                             │
                    ┌────────┴────────┐
                    │   Data Flow     │
                    │ Redis→InfluxDB  │
                    └─────────────────┘
```

## 核心组件

### 1. Data Bridge（数据桥接）

#### Redis 订阅器
```rust
pub struct RedisSubscriber {
    redis_client: Arc<RedisClient>,
    buffer: Arc<BatchBuffer>,
    patterns: Vec<String>,
    is_running: Arc<AtomicBool>,
}

impl RedisSubscriber {
    /// 启动订阅
    pub async fn start(&self) -> Result<()> {
        let mut pubsub = self.redis_client.get_async_pubsub().await?;
        
        // 订阅点位更新通道
        for pattern in &self.patterns {
            pubsub.psubscribe(pattern).await?;
        }
        
        // 消息处理循环
        while self.is_running.load(Ordering::Relaxed) {
            match pubsub.on_message().next().await {
                Some(msg) => {
                    if let Ok(update) = self.parse_update(&msg) {
                        self.buffer.add(update).await;
                    }
                }
                None => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        
        Ok(())
    }
}
```

#### 批量缓冲区
```rust
pub struct BatchBuffer {
    points: Arc<RwLock<Vec<PointData>>>,
    config: BufferConfig,
    write_trigger: Arc<Notify>,
}

pub struct BufferConfig {
    /// 最大批次大小
    max_batch_size: usize,
    
    /// 最大等待时间
    max_wait_time: Duration,
    
    /// 内存限制
    max_memory_mb: usize,
}

impl BatchBuffer {
    /// 添加数据点
    pub async fn add(&self, point: PointData) {
        let mut points = self.points.write().await;
        points.push(point);
        
        // 检查是否需要触发写入
        if self.should_flush(&points) {
            self.write_trigger.notify_one();
        }
    }
    
    /// 获取并清空缓冲区
    pub async fn drain(&self) -> Vec<PointData> {
        let mut points = self.points.write().await;
        std::mem::take(&mut *points)
    }
}
```

### 2. InfluxDB Writer（写入器）

#### 写入管理器
```rust
pub struct WriteManager {
    client: Arc<InfluxDbClient>,
    buffer: Arc<BatchBuffer>,
    config: WriteConfig,
    metrics: Arc<WriteMetrics>,
}

impl WriteManager {
    /// 运行写入循环
    pub async fn run(&self) -> Result<()> {
        let mut interval = tokio::time::interval(self.config.check_interval);
        
        loop {
            tokio::select! {
                _ = self.buffer.write_trigger.notified() => {
                    self.flush_buffer().await?;
                }
                _ = interval.tick() => {
                    // 定期检查并写入
                    if self.buffer.size().await > 0 {
                        self.flush_buffer().await?;
                    }
                }
            }
        }
    }
    
    /// 批量写入数据
    async fn flush_buffer(&self) -> Result<()> {
        let points = self.buffer.drain().await;
        if points.is_empty() {
            return Ok(());
        }
        
        let start = Instant::now();
        
        // 转换为 InfluxDB 行协议
        let lines = self.convert_to_line_protocol(&points)?;
        
        // 批量写入
        match self.write_with_retry(&lines).await {
            Ok(_) => {
                let duration = start.elapsed();
                self.metrics.record_write(points.len(), duration);
                info!("Wrote {} points in {:?}", points.len(), duration);
            }
            Err(e) => {
                error!("Write failed: {}", e);
                self.handle_write_failure(points).await?;
            }
        }
        
        Ok(())
    }
}
```

#### 行协议转换
```rust
/// 转换为 InfluxDB 行协议
fn convert_to_line_protocol(&self, points: &[PointData]) -> Result<Vec<String>> {
    let mut lines = Vec::with_capacity(points.len());
    
    for point in points {
        // measurement,tag1=value1,tag2=value2 field1=value1,field2=value2 timestamp
        let line = format!(
            "{},channel={},type={},point={} value={} {}",
            self.get_measurement_name(&point.point_type),
            point.channel_id,
            point.point_type,
            point.point_id,
            point.value,
            point.timestamp * 1_000_000  // 转换为纳秒
        );
        
        lines.push(line);
    }
    
    Ok(lines)
}

/// 根据点类型获取测量名称
fn get_measurement_name(&self, point_type: &str) -> &'static str {
    match point_type {
        "m" => "measurement",
        "s" => "signal",
        "c" => "control",
        "a" => "adjustment",
        _ => "unknown",
    }
}
```

### 3. Data Retention（数据保留）

#### 保留策略管理
```rust
pub struct RetentionManager {
    client: Arc<InfluxDbClient>,
    policies: Vec<RetentionPolicy>,
}

pub struct RetentionPolicy {
    /// 策略名称
    name: String,
    
    /// 数据保留时长
    duration: Duration,
    
    /// 副本数
    replication: u32,
    
    /// 分片持续时间
    shard_duration: Option<Duration>,
    
    /// 应用的测量
    measurements: Vec<String>,
}

impl RetentionManager {
    /// 创建保留策略
    pub async fn create_policies(&self) -> Result<()> {
        for policy in &self.policies {
            let query = format!(
                "CREATE RETENTION POLICY \"{}\" ON \"{}\" DURATION {} REPLICATION {} {}",
                policy.name,
                self.database,
                self.format_duration(&policy.duration),
                policy.replication,
                policy.shard_duration
                    .map(|d| format!("SHARD DURATION {}", self.format_duration(&d)))
                    .unwrap_or_default()
            );
            
            self.client.query(&query).await?;
        }
        
        Ok(())
    }
}
```

#### 数据降采样
```rust
pub struct Downsampler {
    client: Arc<InfluxDbClient>,
    tasks: Vec<DownsampleTask>,
}

pub struct DownsampleTask {
    /// 任务名称
    name: String,
    
    /// 源保留策略
    source_rp: String,
    
    /// 目标保留策略
    target_rp: String,
    
    /// 聚合间隔
    interval: Duration,
    
    /// 聚合函数
    aggregations: Vec<Aggregation>,
}

impl Downsampler {
    /// 创建连续查询
    pub async fn create_continuous_queries(&self) -> Result<()> {
        for task in &self.tasks {
            let cq_query = self.build_cq_query(task)?;
            self.client.query(&cq_query).await?;
        }
        
        Ok(())
    }
    
    /// 构建连续查询语句
    fn build_cq_query(&self, task: &DownsampleTask) -> Result<String> {
        let aggregations = task.aggregations.iter()
            .map(|agg| format!("{}(value) as {}", agg.function, agg.alias))
            .collect::<Vec<_>>()
            .join(", ");
        
        Ok(format!(
            r#"CREATE CONTINUOUS QUERY "{}" ON "{}"
            BEGIN
                SELECT {}
                INTO "{}"."{}"."autogen"
                FROM "{}"."{}"."measurement"
                GROUP BY time({}), *
            END"#,
            task.name,
            self.database,
            aggregations,
            self.database,
            task.target_rp,
            self.database,
            task.source_rp,
            self.format_duration(&task.interval)
        ))
    }
}
```

### 4. Query Engine（查询引擎）

#### 查询接口
```rust
pub struct QueryEngine {
    client: Arc<InfluxDbClient>,
    cache: Arc<QueryCache>,
    config: QueryConfig,
}

impl QueryEngine {
    /// 查询时间序列数据
    pub async fn query_series(
        &self,
        request: SeriesQueryRequest,
    ) -> Result<SeriesQueryResponse> {
        // 检查缓存
        let cache_key = request.cache_key();
        if let Some(cached) = self.cache.get(&cache_key).await {
            return Ok(cached);
        }
        
        // 构建查询
        let query = self.build_series_query(&request)?;
        
        // 执行查询
        let start = Instant::now();
        let result = self.client.query(&query).await?;
        let duration = start.elapsed();
        
        // 解析结果
        let response = self.parse_series_result(result)?;
        
        // 缓存结果
        if request.cacheable {
            self.cache.put(cache_key, response.clone()).await;
        }
        
        // 记录指标
        metrics::histogram!("hissrv.query.duration", duration.as_secs_f64());
        
        Ok(response)
    }
    
    /// 构建查询语句
    fn build_series_query(&self, req: &SeriesQueryRequest) -> Result<String> {
        let mut query = format!(
            "SELECT {} FROM {} WHERE time >= '{}' AND time <= '{}'",
            req.fields.join(", "),
            req.measurement,
            req.start_time.to_rfc3339(),
            req.end_time.to_rfc3339()
        );
        
        // 添加标签过滤
        for (tag, value) in &req.tags {
            query.push_str(&format!(" AND {} = '{}'", tag, value));
        }
        
        // 添加分组
        if !req.group_by.is_empty() {
            query.push_str(&format!(" GROUP BY {}", req.group_by.join(", ")));
        }
        
        // 添加排序
        query.push_str(" ORDER BY time DESC");
        
        // 添加限制
        if let Some(limit) = req.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        Ok(query)
    }
}
```

#### 聚合查询
```rust
pub struct Aggregator {
    client: Arc<InfluxDbClient>,
}

impl Aggregator {
    /// 执行聚合查询
    pub async fn aggregate(
        &self,
        request: AggregateRequest,
    ) -> Result<AggregateResponse> {
        let query = match request.aggregate_type {
            AggregateType::TimeWindow => {
                self.build_time_window_query(&request)?
            }
            AggregateType::Statistical => {
                self.build_statistical_query(&request)?
            }
            AggregateType::Custom => {
                request.custom_query.ok_or(Error::InvalidQuery)?
            }
        };
        
        let result = self.client.query(&query).await?;
        self.parse_aggregate_result(result, request.aggregate_type)
    }
    
    /// 构建时间窗口聚合查询
    fn build_time_window_query(&self, req: &AggregateRequest) -> Result<String> {
        Ok(format!(
            r#"SELECT 
                {}(value) as value,
                COUNT(value) as count,
                MIN(value) as min,
                MAX(value) as max,
                STDDEV(value) as stddev
            FROM {}
            WHERE time >= '{}' AND time <= '{}'
            {}
            GROUP BY time({}), *"#,
            req.function.unwrap_or("MEAN".to_string()),
            req.measurement,
            req.start_time.to_rfc3339(),
            req.end_time.to_rfc3339(),
            self.build_where_clause(&req.filters),
            req.interval.unwrap_or("1h".to_string())
        ))
    }
}
```

## 数据流示例

### 1. 实时数据写入流程

```
Redis Point Update → Subscriber → Buffer → Writer → InfluxDB
     │                    │          │        │         │
     └─ point:update ─────┘          │        │         │
                                     │        │         │
                         Batch(1000) │        │         │
                                     └────────┘         │
                                                        │
                                     Line Protocol      │
                                                        └─ measurement
```

### 2. 查询处理流程

```
API Request → Query Engine → Cache Check → InfluxDB → Response
     │             │             │            │           │
     └─ /query ────┘             │            │           │
                                 │            │           │
                    Cache Hit ───┘            │           │
                                              │           │
                              SQL Generation  │           │
                                              └───────────┘
```

## 性能优化

### 1. 批量写入优化

```rust
pub struct OptimizedWriter {
    /// 动态批次大小
    dynamic_batch_size: AtomicUsize,
    
    /// 性能统计
    perf_stats: Arc<RwLock<PerfStats>>,
}

impl OptimizedWriter {
    /// 自适应批次大小
    async fn adjust_batch_size(&self) {
        let stats = self.perf_stats.read().await;
        
        let current_size = self.dynamic_batch_size.load(Ordering::Relaxed);
        let avg_latency = stats.avg_write_latency();
        
        let new_size = if avg_latency < Duration::from_millis(100) {
            // 延迟低，增加批次大小
            (current_size * 1.2).min(10000)
        } else if avg_latency > Duration::from_millis(500) {
            // 延迟高，减少批次大小
            (current_size * 0.8).max(100)
        } else {
            current_size
        };
        
        self.dynamic_batch_size.store(new_size, Ordering::Relaxed);
    }
}
```

### 2. 查询缓存

```rust
pub struct QueryCache {
    cache: Arc<RwLock<LruCache<String, CachedResult>>>,
    config: CacheConfig,
}

impl QueryCache {
    /// 智能缓存策略
    pub async fn should_cache(&self, query: &Query) -> bool {
        // 热点数据缓存
        if query.is_hot_data() {
            return true;
        }
        
        // 聚合查询缓存
        if query.has_aggregation() && query.time_range() > Duration::from_hours(1) {
            return true;
        }
        
        // 重复查询缓存
        if self.query_frequency(query).await > 3 {
            return true;
        }
        
        false
    }
}
```

### 3. 并发控制

```rust
/// 并发写入控制
pub struct ConcurrentWriter {
    semaphore: Arc<Semaphore>,
    write_queue: Arc<SegQueue<WriteBatch>>,
}

impl ConcurrentWriter {
    pub async fn write_concurrent(&self, batches: Vec<WriteBatch>) -> Vec<Result<()>> {
        let tasks: Vec<_> = batches.into_iter()
            .map(|batch| {
                let sem = self.semaphore.clone();
                let writer = self.clone();
                
                tokio::spawn(async move {
                    let _permit = sem.acquire().await?;
                    writer.write_single_batch(batch).await
                })
            })
            .collect();
        
        futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap_or_else(|e| Err(e.into())))
            .collect()
    }
}
```

## 配置管理

### 服务配置
```yaml
# hissrv 配置
influxdb:
  url: "http://localhost:8086"
  token: "${INFLUXDB_TOKEN}"
  org: "voltageems"
  bucket: "telemetry"

redis:
  url: "redis://localhost:6379"
  patterns:
    - "point:update:*"
    - "calc:result:*"

buffer:
  max_batch_size: 5000
  max_wait_time: 10s
  max_memory_mb: 100

writer:
  num_workers: 4
  retry_attempts: 3
  retry_delay: 1s

retention:
  policies:
    - name: "realtime"
      duration: "7d"
      replication: 1
      
    - name: "hourly"
      duration: "30d"
      replication: 1
      
    - name: "daily"
      duration: "365d"
      replication: 1
```

### 降采样配置
```yaml
downsampling:
  tasks:
    - name: "hourly_avg"
      source: "realtime"
      target: "hourly"
      interval: "1h"
      aggregations:
        - function: "MEAN"
          alias: "avg"
        - function: "MAX"
          alias: "max"
        - function: "MIN"
          alias: "min"
          
    - name: "daily_summary"
      source: "hourly"
      target: "daily"
      interval: "1d"
      aggregations:
        - function: "MEAN"
          alias: "avg"
        - function: "SUM"
          alias: "total"
```

## 监控指标

### 写入指标
- 批次大小分布
- 写入延迟
- 写入吞吐量
- 错误率

### 查询指标
- 查询延迟
- 缓存命中率
- 并发查询数
- 结果集大小

### 系统指标
- 内存使用
- 连接池状态
- 队列长度
- CPU 使用率

## 故障处理

### 1. 写入失败处理
```rust
/// 写入失败重试
async fn handle_write_failure(&self, points: Vec<PointData>) -> Result<()> {
    // 1. 本地持久化
    self.persist_to_disk(&points).await?;
    
    // 2. 启动恢复任务
    self.schedule_recovery().await;
    
    // 3. 告警通知
    self.alert_manager.send_alert(
        AlertLevel::Warning,
        "InfluxDB write failure, data persisted locally"
    ).await;
    
    Ok(())
}
```

### 2. 数据恢复
```rust
/// 数据恢复任务
async fn recovery_task(&self) -> Result<()> {
    let pending_files = self.scan_pending_files().await?;
    
    for file in pending_files {
        match self.recover_from_file(&file).await {
            Ok(_) => {
                info!("Recovered data from {}", file);
                self.remove_file(&file).await?;
            }
            Err(e) => {
                error!("Failed to recover {}: {}", file, e);
            }
        }
    }
    
    Ok(())
}
```

## 扩展指南

### 1. 添加新的存储后端
- 实现 `StorageBackend` trait
- 配置后端参数
- 实现数据转换

### 2. 自定义聚合函数
- 扩展聚合类型
- 实现计算逻辑
- 注册到查询引擎

### 3. 集成告警系统
- 实现告警规则
- 配置通知渠道
- 集成监控指标