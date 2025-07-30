# HisSrv 改进方案

## 当前状态
- 专注于数据写入，不提供查询功能
- 批量写入 InfluxDB
- 配置热重载支持
- 极简设计，易于维护

## 改进建议

### 1. 动态批处理优化

```rust
pub struct AdaptiveBatcher {
    min_batch_size: usize,
    max_batch_size: usize,
    max_wait_time: Duration,
    current_load: LoadMetrics,
}

impl AdaptiveBatcher {
    pub async fn should_flush(&self, current_batch: &Batch) -> bool {
        // 根据系统负载动态调整
        if self.current_load.is_high() {
            // 高负载时增大批次，减少写入频率
            current_batch.len() >= self.max_batch_size
        } else {
            // 低负载时减小批次，提高实时性
            current_batch.len() >= self.min_batch_size 
                || current_batch.age() > self.max_wait_time
        }
    }
}
```

### 2. 数据压缩与编码

```rust
// 实现差分编码减少存储
pub struct DeltaEncoder {
    last_values: HashMap<String, f64>,
}

impl DeltaEncoder {
    pub fn encode(&mut self, point: &DataPoint) -> EncodedPoint {
        let delta = match self.last_values.get(&point.name) {
            Some(last) => point.value - last,
            None => point.value,
        };
        
        self.last_values.insert(point.name.clone(), point.value);
        
        EncodedPoint {
            name: point.name.clone(),
            delta,
            timestamp: point.timestamp,
        }
    }
}
```

### 3. 写入失败处理

```rust
pub struct WriteBuffer {
    primary: InfluxDbClient,
    buffer_file: PathBuf,
    retry_queue: VecDeque<WriteBatch>,
}

impl WriteBuffer {
    pub async fn write_with_retry(&mut self, batch: WriteBatch) -> Result<()> {
        match self.primary.write(&batch).await {
            Ok(_) => {
                // 成功后尝试重传缓存数据
                self.flush_retry_queue().await;
                Ok(())
            }
            Err(e) => {
                // 失败时缓存到本地
                self.buffer_to_disk(&batch)?;
                self.retry_queue.push_back(batch);
                Err(e)
            }
        }
    }
    
    async fn flush_retry_queue(&mut self) {
        while let Some(batch) = self.retry_queue.pop_front() {
            if self.primary.write(&batch).await.is_err() {
                self.retry_queue.push_front(batch);
                break;
            }
        }
    }
}
```

### 4. 数据分片策略

```yaml
# 配置示例
influxdb:
  sharding:
    strategy: "hash"  # hash | range | tag-based
    buckets:
      - name: "high_frequency"
        retention: "7d"
        points: ["voltage", "current", "frequency"]
      - name: "low_frequency"
        retention: "30d"
        points: ["temperature", "humidity"]
      - name: "events"
        retention: "365d"
        points: ["alarms", "controls"]
```

### 5. 性能监控指标

```rust
// 添加详细的性能指标
pub struct Metrics {
    write_duration: Histogram,
    batch_size: Histogram,
    queue_depth: Gauge,
    failed_writes: Counter,
    data_points_written: Counter,
}

impl HisSrv {
    pub fn export_metrics(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            avg_write_time: self.metrics.write_duration.mean(),
            avg_batch_size: self.metrics.batch_size.mean(),
            current_queue_depth: self.metrics.queue_depth.get(),
            error_rate: self.calculate_error_rate(),
            throughput: self.calculate_throughput(),
        }
    }
}
```

### 6. 数据生命周期管理

```rust
// 自动数据归档
pub struct DataLifecycleManager {
    rules: Vec<LifecycleRule>,
}

pub struct LifecycleRule {
    measurement: String,
    hot_duration: Duration,    // 热数据期
    warm_duration: Duration,   // 温数据期
    cold_storage: ColdStorage, // 冷存储配置
}

impl DataLifecycleManager {
    pub async fn apply_policies(&self) -> Result<()> {
        for rule in &self.rules {
            // 将超过热数据期的数据降采样
            self.downsample_old_data(&rule).await?;
            
            // 将超过温数据期的数据归档到冷存储
            self.archive_to_cold_storage(&rule).await?;
        }
        Ok(())
    }
}
```

## 配置增强

```yaml
service:
  name: "hissrv"
  
performance:
  adaptive_batching:
    enabled: true
    min_batch_size: 100
    max_batch_size: 10000
    max_wait_ms: 5000
    
  compression:
    enabled: true
    algorithm: "delta"  # delta | zstd | snappy
    
  buffer:
    enabled: true
    max_memory_mb: 512
    disk_backup: "/var/lib/hissrv/buffer"
    
lifecycle:
  rules:
    - measurement: "telemetry"
      hot_days: 7
      warm_days: 30
      downsample: "5m"
      cold_storage: "s3://bucket/archive/"
```

## 实施优先级

1. **高**：动态批处理优化（立即改善性能）
2. **高**：写入失败处理（提高可靠性）
3. **中**：数据压缩（降低存储成本）
4. **低**：生命周期管理（长期优化）

## 预期效果

- 写入吞吐量提升 40%
- 存储成本降低 30%
- 零数据丢失保证
- 自动化运维管理