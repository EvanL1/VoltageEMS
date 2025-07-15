//! 测试辅助工具

use crate::config::{Config, RedisConfig, RedisConnection, StorageConfig, ServiceConfig, LoggingConfig, ApiConfig};
use crate::storage::{DataPoint, DataValue};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;
use voltage_common::data::{PointData, Quality};

/// 创建测试配置
pub fn create_test_config() -> Config {
    Config {
        service: ServiceConfig {
            name: "hissrv-test".to_string(),
            version: "0.1.0-test".to_string(),
            host: "127.0.0.1".to_string(),
            port: 0, // 使用随机端口
        },
        redis: RedisConfig {
            connection: RedisConnection {
                host: "localhost".to_string(),
                port: 6379,
                password: String::new(),
                socket: String::new(),
                database: 15, // 使用测试数据库
                timeout_seconds: 5,
                max_retries: 3,
            },
            subscription: vec![
                "test:*".to_string(),
                "1001:*:*".to_string(),
            ],
            batch_size: 100,
            flush_interval_ms: 100,
        },
        storage: StorageConfig {
            default: "memory".to_string(),
            backends: crate::config::StorageBackends {
                influxdb: crate::config::InfluxDBConfig {
                    enabled: false,
                    url: "http://localhost:8086".to_string(),
                    token: "test-token".to_string(),
                    org: "test-org".to_string(),
                    bucket: "test-bucket".to_string(),
                    precision: "ns".to_string(),
                    batch_size: 1000,
                    flush_interval_ms: 1000,
                    retention_days: 7,
                    connection_pool_size: 4,
                    request_timeout_ms: 5000,
                    compression: false,
                },
            },
            retention: crate::config::RetentionConfig {
                enabled: false,
                policies: vec![],
                check_interval_minutes: 60,
            },
            cache: crate::config::CacheConfig {
                enabled: true,
                ttl_seconds: 300,
                max_entries: 10000,
                eviction_policy: "lru".to_string(),
            },
        },
        logging: LoggingConfig {
            level: "debug".to_string(),
            file: Some("/tmp/hissrv-test.log".to_string()),
            max_size: "10MB".to_string(),
            max_backups: 3,
            max_age: 7,
            compress: false,
        },
        api: ApiConfig {
            enabled: true,
            auth_enabled: false,
            jwt_secret: "test-secret".to_string(),
            cors_origins: vec!["*".to_string()],
            rate_limit: crate::config::RateLimitConfig {
                enabled: false,
                requests_per_minute: 60,
                burst_size: 10,
            },
            compression: false,
            swagger_enabled: true,
        },
        query_optimizer: crate::config::QueryOptimizerConfig {
            enabled: true,
            cache_enabled: true,
            cache_ttl_seconds: 300,
            parallel_queries: true,
            max_parallel_queries: 4,
            query_timeout_seconds: 30,
            downsampling_enabled: true,
            downsampling_thresholds: vec![
                crate::config::DownsamplingThreshold {
                    time_range_hours: 1,
                    interval: "1m".to_string(),
                },
                crate::config::DownsamplingThreshold {
                    time_range_hours: 24,
                    interval: "5m".to_string(),
                },
            ],
        },
        config_file: "test-config".to_string(),
    }
}

/// 创建测试用的 DataPoint
pub fn create_test_data_point(key: &str, value: f64) -> DataPoint {
    DataPoint {
        key: key.to_string(),
        value: DataValue::Float(value),
        timestamp: Utc::now(),
        tags: HashMap::new(),
        metadata: HashMap::new(),
    }
}

/// 创建带标签的测试 DataPoint
pub fn create_test_data_point_with_tags(
    key: &str,
    value: f64,
    tags: HashMap<String, String>,
) -> DataPoint {
    DataPoint {
        key: key.to_string(),
        value: DataValue::Float(value),
        timestamp: Utc::now(),
        tags,
        metadata: HashMap::new(),
    }
}

/// 创建测试用的 PointData
pub fn create_test_point_data(
    channel_id: u32,
    point_id: u32,
    value: f64,
) -> PointData {
    PointData {
        channel_id,
        point_id,
        value,
        quality: Quality::Good,
        timestamp: Utc::now(),
    }
}

/// 创建批量测试数据
pub fn create_test_batch(size: usize, base_value: f64) -> Vec<DataPoint> {
    (0..size)
        .map(|i| {
            let mut tags = HashMap::new();
            tags.insert("sensor".to_string(), format!("sensor_{}", i % 10));
            tags.insert("location".to_string(), format!("location_{}", i % 5));
            
            DataPoint {
                key: format!("test_metric_{}", i),
                value: DataValue::Float(base_value + i as f64),
                timestamp: Utc::now(),
                tags,
                metadata: HashMap::new(),
            }
        })
        .collect()
}

/// 创建时间序列测试数据
pub fn create_time_series_data(
    key: &str,
    start_time: DateTime<Utc>,
    count: usize,
    interval_seconds: i64,
) -> Vec<DataPoint> {
    (0..count)
        .map(|i| {
            DataPoint {
                key: key.to_string(),
                value: DataValue::Float((i as f64).sin() * 100.0),
                timestamp: start_time + chrono::Duration::seconds(i as i64 * interval_seconds),
                tags: HashMap::new(),
                metadata: HashMap::new(),
            }
        })
        .collect()
}

/// 生成测试用的通道名称
pub fn generate_test_channel(channel_id: u32, msg_type: &str, point_id: u32) -> String {
    format!("{}:{}:{}", channel_id, msg_type, point_id)
}

/// 生成随机的测试数据
pub fn generate_random_data(count: usize) -> Vec<DataPoint> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    (0..count)
        .map(|_| {
            let value = rng.gen_range(0.0..100.0);
            let sensor_id = rng.gen_range(1..=10);
            let location_id = rng.gen_range(1..=5);
            
            let mut tags = HashMap::new();
            tags.insert("sensor".to_string(), format!("sensor_{}", sensor_id));
            tags.insert("location".to_string(), format!("location_{}", location_id));
            
            DataPoint {
                key: format!("metric_{}", Uuid::new_v4()),
                value: DataValue::Float(value),
                timestamp: Utc::now(),
                tags,
                metadata: HashMap::new(),
            }
        })
        .collect()
}

/// 等待条件满足或超时
pub async fn wait_for_condition<F, Fut>(
    condition: F,
    timeout_ms: u64,
    check_interval_ms: u64,
) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_millis(timeout_ms);
    let interval = tokio::time::Duration::from_millis(check_interval_ms);
    
    while start.elapsed() < timeout {
        if condition().await {
            return true;
        }
        tokio::time::sleep(interval).await;
    }
    
    false
}

/// 测试用的时间戳生成器
pub struct TestTimestampGenerator {
    current: DateTime<Utc>,
    increment: chrono::Duration,
}

impl TestTimestampGenerator {
    pub fn new(start: DateTime<Utc>, increment_seconds: i64) -> Self {
        Self {
            current: start,
            increment: chrono::Duration::seconds(increment_seconds),
        }
    }
    
    pub fn next(&mut self) -> DateTime<Utc> {
        let timestamp = self.current;
        self.current = self.current + self.increment;
        timestamp
    }
    
    pub fn skip(&mut self, count: usize) {
        for _ in 0..count {
            self.next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_test_config() {
        let config = create_test_config();
        assert_eq!(config.service.name, "hissrv-test");
        assert_eq!(config.redis.connection.database, 15);
    }
    
    #[test]
    fn test_create_test_data_point() {
        let point = create_test_data_point("test_key", 42.0);
        assert_eq!(point.key, "test_key");
        assert!(matches!(point.value, DataValue::Float(v) if v == 42.0));
    }
    
    #[test]
    fn test_generate_test_channel() {
        assert_eq!(generate_test_channel(1001, "m", 10001), "1001:m:10001");
        assert_eq!(generate_test_channel(2002, "s", 20002), "2002:s:20002");
    }
    
    #[tokio::test]
    async fn test_wait_for_condition() {
        let mut counter = 0;
        let condition = || async {
            counter += 1;
            counter >= 3
        };
        
        let result = wait_for_condition(condition, 1000, 100).await;
        assert!(result);
    }
}