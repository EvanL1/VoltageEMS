# hissrv Redis Hash结构设计

## 概述
hissrv（历史服务）负责从Redis读取实时数据并归档到InfluxDB时序数据库。它不写入Redis，只从comsrv和modsrv的Hash结构中读取数据，优化了批量读取和数据转换流程。

## 读取的Hash结构

### 从comsrv读取
```
Source Keys: comsrv:realtime:channel:{channel_id}
读取方式: HGETALL 或 HMGET
数据格式: JSON格式的点位数据
```

### 从modsrv读取
```
Source Keys: modsrv:realtime:module:{module_id}
读取方式: HGETALL 或 HMGET
数据格式: JSON格式的计算结果
```

## 数据读取策略

### 批量读取优化
```rust
pub async fn get_channels_batch(&self, channel_ids: &[u16]) -> Result<HashMap<u16, HashMap<String, Value>>> {
    let mut result = HashMap::new();
    
    // 使用Pipeline批量读取
    let mut pipe = self.redis.pipeline();
    
    for &channel_id in channel_ids {
        let key = format!("comsrv:realtime:channel:{}", channel_id);
        pipe.hgetall(&key);
    }
    
    let values: Vec<HashMap<String, String>> = pipe.query_async(&mut self.conn).await?;
    
    // 解析结果
    for (idx, channel_data) in values.into_iter().enumerate() {
        let channel_id = channel_ids[idx];
        let mut parsed_data = HashMap::new();
        
        for (field, value) in channel_data {
            if let Ok(json_value) = serde_json::from_str::<Value>(&value) {
                parsed_data.insert(field, json_value);
            }
        }
        
        result.insert(channel_id, parsed_data);
    }
    
    Ok(result)
}
```

### 增量读取
```rust
pub struct IncrementalReader {
    last_read_time: HashMap<String, DateTime<Utc>>,
    read_interval: Duration,
}

impl IncrementalReader {
    pub async fn read_updated_data(&mut self) -> Result<Vec<PointData>> {
        let mut updated_points = Vec::new();
        
        for (key, last_time) in &self.last_read_time {
            let data = self.redis.hgetall(key).await?;
            
            for (field, value) in data {
                if let Ok(point) = serde_json::from_str::<PointData>(&value) {
                    if point.timestamp > *last_time {
                        updated_points.push(point);
                    }
                }
            }
        }
        
        // 更新读取时间
        self.update_read_times();
        
        Ok(updated_points)
    }
}
```

## InfluxDB数据转换

### 数据模型映射
```rust
pub struct InfluxDBPoint {
    measurement: String,
    tags: HashMap<String, String>,
    fields: HashMap<String, FieldValue>,
    timestamp: i64,
}

impl From<PointData> for InfluxDBPoint {
    fn from(point: PointData) -> Self {
        let mut tags = HashMap::new();
        tags.insert("channel_id".to_string(), point.channel_id.to_string());
        tags.insert("point_id".to_string(), point.id.clone());
        tags.insert("telemetry_type".to_string(), point.telemetry_type.to_string());
        
        let mut fields = HashMap::new();
        
        // 根据数据类型转换
        match point.telemetry_type {
            TelemetryType::Measurement => {
                if let Ok(value) = point.value.parse::<f64>() {
                    fields.insert("value".to_string(), FieldValue::Float(value));
                }
            }
            TelemetryType::Signal => {
                if let Ok(value) = point.value.parse::<i64>() {
                    fields.insert("value".to_string(), FieldValue::Integer(value));
                }
            }
            _ => {
                fields.insert("value".to_string(), FieldValue::String(point.value));
            }
        }
        
        fields.insert("quality".to_string(), FieldValue::String(point.quality));
        
        InfluxDBPoint {
            measurement: "telemetry".to_string(),
            tags,
            fields,
            timestamp: point.timestamp.timestamp_nanos(),
        }
    }
}
```

### 批量写入优化
```rust
pub struct BatchWriter {
    buffer: Vec<InfluxDBPoint>,
    max_batch_size: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl BatchWriter {
    pub async fn write(&mut self, points: Vec<InfluxDBPoint>) -> Result<()> {
        self.buffer.extend(points);
        
        // 检查是否需要刷新
        if self.should_flush() {
            self.flush().await?;
        }
        
        Ok(())
    }
    
    fn should_flush(&self) -> bool {
        self.buffer.len() >= self.max_batch_size ||
        self.last_flush.elapsed() >= self.flush_interval
    }
    
    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        
        // 批量写入InfluxDB
        let points = std::mem::take(&mut self.buffer);
        self.influxdb_client.write_points(points).await?;
        
        self.last_flush = Instant::now();
        Ok(())
    }
}
```

## 数据聚合策略

### 降采样配置
```yaml
downsampling:
  - name: "1分钟聚合"
    source: "telemetry"
    interval: "1m"
    retention: "7d"
    aggregations: ["mean", "max", "min", "count"]
    
  - name: "5分钟聚合"
    source: "telemetry_1m"
    interval: "5m"
    retention: "30d"
    aggregations: ["mean", "max", "min"]
    
  - name: "1小时聚合"
    source: "telemetry_5m"
    interval: "1h"
    retention: "365d"
    aggregations: ["mean", "max", "min"]
```

### 连续查询
```sql
-- 1分钟平均值
CREATE CONTINUOUS QUERY "cq_1m_mean" ON "ems"
BEGIN
  SELECT mean("value") AS "value_mean",
         max("value") AS "value_max",
         min("value") AS "value_min",
         count("value") AS "value_count"
  INTO "telemetry_1m"
  FROM "telemetry"
  GROUP BY time(1m), *
END

-- 电能累计
CREATE CONTINUOUS QUERY "cq_energy_daily" ON "ems"
BEGIN
  SELECT integral("value") / 3600000 AS "energy_kwh"
  INTO "energy_daily"
  FROM "telemetry"
  WHERE "telemetry_type" = 'Measurement' 
    AND "unit" = 'kW'
  GROUP BY time(1d), "channel_id", "point_id"
END
```

## 性能监控

### 读取性能指标
```
Key: hissrv:metrics:read_performance
Fields:
  channels_per_second: "156.7"
  points_per_second: "15670.3"
  avg_read_latency_ms: "2.3"
  max_read_latency_ms: "45.6"
  redis_connection_pool_size: "10"
  redis_connection_pool_used: "3"
```

### 写入性能指标
```
Key: hissrv:metrics:write_performance
Fields:
  points_per_second: "14523.6"
  batches_per_minute: "120"
  avg_batch_size: "7261"
  avg_write_latency_ms: "15.4"
  max_write_latency_ms: "156.7"
  influxdb_queue_size: "0"
  influxdb_dropped_points: "0"
```

## 数据完整性保证

### 数据验证
```rust
pub struct DataValidator {
    pub check_timestamp: bool,
    pub check_value_range: bool,
    pub check_quality: bool,
    pub max_age: Duration,
}

impl DataValidator {
    pub fn validate(&self, point: &PointData) -> ValidationResult {
        let mut result = ValidationResult::default();
        
        // 时间戳检查
        if self.check_timestamp {
            let age = Utc::now() - point.timestamp;
            if age > self.max_age {
                result.add_warning("Data too old");
            }
            if point.timestamp > Utc::now() {
                result.add_error("Future timestamp");
            }
        }
        
        // 值范围检查
        if self.check_value_range {
            if let Some(range) = self.get_value_range(&point.id) {
                if !range.contains(&point.value) {
                    result.add_warning("Value out of range");
                }
            }
        }
        
        // 质量检查
        if self.check_quality && point.quality != "good" {
            result.add_info("Non-good quality");
        }
        
        result
    }
}
```

### 数据补齐
```rust
pub async fn fill_missing_data(&mut self) -> Result<()> {
    let channels = self.get_configured_channels();
    
    for channel_id in channels {
        let expected_points = self.get_channel_points(channel_id);
        let actual_data = self.get_channel_realtime_data(channel_id).await?;
        
        for point_id in expected_points {
            if !actual_data.contains_key(&point_id) {
                // 创建缺失数据标记
                let missing_point = PointData {
                    id: point_id,
                    value: "NaN".to_string(),
                    quality: "missing",
                    timestamp: Utc::now(),
                    ..Default::default()
                };
                
                self.write_to_influxdb(missing_point).await?;
            }
        }
    }
    
    Ok(())
}
```

## 查询优化

### 并行查询
```rust
pub async fn parallel_read_channels(&self, channel_ids: Vec<u16>) -> Result<HashMap<u16, Value>> {
    let mut tasks = Vec::new();
    
    for channel_id in channel_ids {
        let redis = self.redis.clone();
        tasks.push(tokio::spawn(async move {
            let key = format!("comsrv:realtime:channel:{}", channel_id);
            (channel_id, redis.hgetall(&key).await)
        }));
    }
    
    let mut results = HashMap::new();
    for task in tasks {
        let (channel_id, data) = task.await?;
        if let Ok(channel_data) = data {
            results.insert(channel_id, parse_channel_data(channel_data));
        }
    }
    
    Ok(results)
}
```

### 缓存策略
```rust
pub struct ReadCache {
    cache: HashMap<String, (Value, Instant)>,
    ttl: Duration,
}

impl ReadCache {
    pub async fn get_or_fetch(&mut self, key: &str) -> Result<Value> {
        // 检查缓存
        if let Some((value, timestamp)) = self.cache.get(key) {
            if timestamp.elapsed() < self.ttl {
                return Ok(value.clone());
            }
        }
        
        // 从Redis获取
        let data = self.redis.hgetall(key).await?;
        let value = parse_data(data);
        
        // 更新缓存
        self.cache.insert(key.to_string(), (value.clone(), Instant::now()));
        
        Ok(value)
    }
}
```

## 配置示例

### 服务配置
```yaml
hissrv:
  redis:
    # 数据源配置
    sources:
      - pattern: "comsrv:realtime:channel:*"
        type: "channel"
        read_interval: 1s
        
      - pattern: "modsrv:realtime:module:*"
        type: "module"
        read_interval: 5s
    
    # 批量读取配置
    batch:
      size: 100
      timeout: 100ms
      parallel: 4
  
  influxdb:
    # 写入配置
    write:
      batch_size: 5000
      flush_interval: 1s
      max_retries: 3
      
    # 数据保留策略
    retention_policies:
      - name: "raw"
        duration: "7d"
        replication: 1
        
      - name: "aggregated"
        duration: "365d"
        replication: 1
```

## 故障处理

### Redis连接故障
1. 使用连接池和健康检查
2. 自动重连机制
3. 降级到最后已知数据

### InfluxDB写入故障
1. 本地WAL（Write Ahead Log）
2. 重试队列
3. 数据压缩和批量重传

## 最佳实践

1. **合理设置读取间隔**：平衡实时性和系统负载
2. **优化批量大小**：根据网络和InfluxDB性能调整
3. **使用数据压缩**：减少存储空间
4. **实施数据生命周期**：自动清理过期数据
5. **监控关键指标**：及时发现性能瓶颈
6. **数据校验完善**：确保数据质量
7. **灾备方案**：定期备份和恢复测试