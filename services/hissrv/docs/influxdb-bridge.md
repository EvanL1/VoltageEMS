# InfluxDB 桥接设计

## 概述

hissrv 作为 Redis 和 InfluxDB 之间的桥梁，负责将实时数据从 Redis Hash 结构转换为 InfluxDB 时序数据格式。本文档详细介绍数据转换、批处理优化和错误处理机制。

## 数据转换

### Redis Hash 到 InfluxDB 的映射

#### 源数据格式（Redis Hash）

```
键: comsrv:1001:m
字段:
  10001 → "25.123456"
  10002 → "380.500000"
  10003 → "50.250000"
```

#### 目标格式（InfluxDB Line Protocol）

```
telemetry,channel_id=1001,point_id=10001,point_type=m value=25.123456 1642592400000000000
telemetry,channel_id=1001,point_id=10002,point_type=m value=380.500000 1642592400000000000
telemetry,channel_id=1001,point_id=10003,point_type=m value=50.250000 1642592400000000000
```

### 数据结构定义

```rust
use influxdb2::models::DataPoint;
use voltage_libs::types::StandardFloat;

pub struct TelemetryPoint {
    pub channel_id: u16,
    pub point_id: u32,
    pub point_type: String,
    pub value: StandardFloat,
    pub timestamp: DateTime<Utc>,
}

impl Into<DataPoint> for TelemetryPoint {
    fn into(self) -> DataPoint {
        DataPoint::builder("telemetry")
            .tag("channel_id", self.channel_id.to_string())
            .tag("point_id", self.point_id.to_string())
            .tag("point_type", self.point_type)
            .field("value", self.value.into_inner())
            .timestamp(self.timestamp.timestamp_nanos())
            .build()
            .unwrap()
    }
}
```

## InfluxDB 客户端

### 客户端配置

```rust
pub struct InfluxDBConfig {
    pub url: String,
    pub org: String,
    pub bucket: String,
    pub token: Option<String>,
    pub timeout: Duration,
    pub gzip: bool,
}

pub struct InfluxDBClient {
    client: influxdb2::Client,
    config: InfluxDBConfig,
    write_api: Arc<Mutex<WriteApi>>,
}

impl InfluxDBClient {
    pub fn new(config: InfluxDBConfig) -> Result<Self> {
        let mut client_builder = influxdb2::Client::new(
            &config.url,
            &config.org,
            config.token.as_deref().unwrap_or(""),
        );
        
        if config.gzip {
            client_builder = client_builder.with_gzip(true);
        }
        
        let client = client_builder.with_timeout(config.timeout);
        
        let write_api = client
            .write_api(&config.bucket)
            .with_precision(Precision::Nanosecond);
        
        Ok(Self {
            client,
            config,
            write_api: Arc::new(Mutex::new(write_api)),
        })
    }
}
```

### 批量写入实现

```rust
impl InfluxDBClient {
    pub async fn write_points(&self, points: &[DataPoint]) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        
        let write_api = self.write_api.lock().await;
        
        // 分批写入（InfluxDB 有最大请求大小限制）
        const MAX_BATCH_SIZE: usize = 5000;
        
        for chunk in points.chunks(MAX_BATCH_SIZE) {
            self.write_chunk(&write_api, chunk).await?;
        }
        
        Ok(())
    }
    
    async fn write_chunk(
        &self,
        write_api: &WriteApi,
        chunk: &[DataPoint],
    ) -> Result<()> {
        match write_api.write_batch(chunk).await {
            Ok(_) => {
                debug!("Written {} points to InfluxDB", chunk.len());
                Ok(())
            }
            Err(e) => {
                error!("Failed to write to InfluxDB: {}", e);
                Err(Error::InfluxDB(e.to_string()))
            }
        }
    }
}
```

## 批处理优化

### 智能批处理策略

```rust
pub struct SmartBatcher {
    buffer: Arc<Mutex<Vec<DataPoint>>>,
    config: BatcherConfig,
    stats: Arc<BatchStats>,
}

pub struct BatcherConfig {
    pub min_batch_size: usize,
    pub max_batch_size: usize,
    pub max_wait_time: Duration,
    pub adaptive: bool,
}

pub struct BatchStats {
    pub avg_batch_size: AtomicU64,
    pub avg_wait_time: AtomicU64,
    pub write_success_rate: AtomicU64,
}

impl SmartBatcher {
    pub async fn should_flush(&self) -> bool {
        let buffer = self.buffer.lock().await;
        let size = buffer.len();
        
        if size >= self.config.max_batch_size {
            return true;
        }
        
        if size >= self.config.min_batch_size {
            // 自适应策略：根据历史统计调整
            if self.config.adaptive {
                let avg_size = self.stats.avg_batch_size.load(Ordering::Relaxed);
                if size as u64 >= avg_size * 80 / 100 {
                    return true;
                }
            } else {
                return true;
            }
        }
        
        false
    }
}
```

### 时间戳优化

```rust
pub struct TimestampOptimizer {
    precision: TimestampPrecision,
    deduplication: bool,
}

pub enum TimestampPrecision {
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
}

impl TimestampOptimizer {
    pub fn optimize_points(&self, points: &mut Vec<DataPoint>) {
        if self.deduplication {
            // 移除相同时间戳的重复点
            self.deduplicate_by_timestamp(points);
        }
        
        // 调整时间戳精度
        match self.precision {
            TimestampPrecision::Second => {
                for point in points {
                    point.timestamp = point.timestamp / 1_000_000_000 * 1_000_000_000;
                }
            }
            TimestampPrecision::Millisecond => {
                for point in points {
                    point.timestamp = point.timestamp / 1_000_000 * 1_000_000;
                }
            }
            _ => {}
        }
    }
}
```

## 数据压缩

### 值压缩策略

```rust
pub struct ValueCompressor {
    compression_rules: HashMap<String, CompressionRule>,
}

pub struct CompressionRule {
    pub point_type: String,
    pub decimal_places: u8,
    pub delta_encoding: bool,
}

impl ValueCompressor {
    pub fn compress_value(
        &self,
        value: f64,
        point_type: &str,
    ) -> f64 {
        if let Some(rule) = self.compression_rules.get(point_type) {
            // 限制小数位数
            let multiplier = 10_f64.powi(rule.decimal_places as i32);
            (value * multiplier).round() / multiplier
        } else {
            // 默认保持 6 位小数
            (value * 1_000_000.0).round() / 1_000_000.0
        }
    }
}
```

## 错误处理和重试

### 写入失败处理

```rust
pub struct WriteErrorHandler {
    retry_policy: RetryPolicy,
    fallback_storage: Option<FallbackStorage>,
}

pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
    pub jitter: bool,
}

impl WriteErrorHandler {
    pub async fn handle_write_error(
        &self,
        points: Vec<DataPoint>,
        error: Error,
    ) -> Result<()> {
        match error {
            Error::InfluxDB(e) if e.contains("timeout") => {
                // 超时错误 - 重试
                self.retry_with_policy(points).await
            }
            Error::InfluxDB(e) if e.contains("unauthorized") => {
                // 认证错误 - 不重试
                error!("Authentication failed: {}", e);
                Err(error)
            }
            Error::InfluxDB(e) if e.contains("too many requests") => {
                // 限流 - 延迟重试
                tokio::time::sleep(Duration::from_secs(60)).await;
                self.retry_with_policy(points).await
            }
            _ => {
                // 其他错误 - 使用后备存储
                if let Some(fallback) = &self.fallback_storage {
                    fallback.store(points).await?;
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
}
```

### 后备存储

```rust
pub struct FallbackStorage {
    path: PathBuf,
    max_size: u64,
    compression: bool,
}

impl FallbackStorage {
    pub async fn store(&self, points: Vec<DataPoint>) -> Result<()> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("failed_points_{}.json", timestamp);
        let filepath = self.path.join(filename);
        
        let data = serde_json::to_vec(&points)?;
        
        if self.compression {
            // 使用 gzip 压缩
            let compressed = compress_data(&data)?;
            tokio::fs::write(filepath.with_extension("json.gz"), compressed).await?;
        } else {
            tokio::fs::write(filepath, data).await?;
        }
        
        // 检查存储大小限制
        self.cleanup_old_files().await?;
        
        Ok(())
    }
    
    pub async fn recover(&self) -> Result<Vec<Vec<DataPoint>>> {
        let mut recovered = Vec::new();
        
        let mut entries = tokio::fs::read_dir(&self.path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension() == Some(OsStr::new("json")) ||
               path.extension() == Some(OsStr::new("gz")) {
                match self.read_file(&path).await {
                    Ok(points) => recovered.push(points),
                    Err(e) => warn!("Failed to recover {}: {}", path.display(), e),
                }
            }
        }
        
        Ok(recovered)
    }
}
```

## 查询接口

### 简单查询实现

```rust
pub async fn query_recent_data(
    &self,
    measurement: &str,
    duration: Duration,
    limit: usize,
) -> Result<Vec<QueryResult>> {
    let query = format!(
        r#"
        from(bucket: "{}")
            |> range(start: -{})
            |> filter(fn: (r) => r["_measurement"] == "{}")
            |> limit(n: {})
            |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
        "#,
        self.config.bucket,
        duration.as_secs(),
        measurement,
        limit
    );
    
    let response = self.client
        .query(Query::new(query))
        .await?;
    
    self.parse_query_response(response)
}
```

## 性能监控

### 写入性能指标

```rust
pub struct WriteMetrics {
    pub points_per_second: RollingAverage,
    pub batch_size_histogram: Histogram,
    pub write_latency_histogram: Histogram,
    pub error_rate: RollingAverage,
}

impl WriteMetrics {
    pub fn record_write(
        &self,
        points_count: usize,
        duration: Duration,
        success: bool,
    ) {
        self.points_per_second.add(points_count as f64 / duration.as_secs_f64());
        self.batch_size_histogram.observe(points_count as f64);
        self.write_latency_histogram.observe(duration.as_secs_f64());
        
        if !success {
            self.error_rate.add(1.0);
        } else {
            self.error_rate.add(0.0);
        }
    }
}
```

## 最佳实践

### 1. 批量大小优化

- 测量点多：使用较大批量（5000-10000）
- 网络延迟高：增加批量大小减少往返
- 内存有限：减小批量大小

### 2. 时间戳处理

- 使用统一的时间源（避免时钟偏差）
- 考虑降低时间戳精度以节省存储
- 处理乱序数据点

### 3. 标签优化

- 限制标签数量（建议 < 10）
- 使用短标签名和值
- 避免高基数标签

### 4. 错误恢复

- 实现后备存储机制
- 定期尝试恢复失败的数据
- 监控错误率和恢复成功率