//! 模拟存储后端

use async_trait::async_trait;
use crate::error::Result;
use crate::storage::{DataPoint, DataValue, Storage, QueryOptions, QueryResult, AggregateFunction};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// 模拟存储配置
#[derive(Debug, Clone)]
pub struct MockStorageConfig {
    pub fail_after_n_writes: Option<usize>,
    pub fail_after_n_reads: Option<usize>,
    pub write_delay_ms: Option<u64>,
    pub read_delay_ms: Option<u64>,
    pub connection_fail: bool,
}

impl Default for MockStorageConfig {
    fn default() -> Self {
        Self {
            fail_after_n_writes: None,
            fail_after_n_reads: None,
            write_delay_ms: None,
            read_delay_ms: None,
            connection_fail: false,
        }
    }
}

/// 模拟存储后端
pub struct MockStorage {
    data: Arc<RwLock<HashMap<String, VecDeque<DataPoint>>>>,
    config: MockStorageConfig,
    write_count: Arc<Mutex<usize>>,
    read_count: Arc<Mutex<usize>>,
    connected: Arc<RwLock<bool>>,
}

impl MockStorage {
    pub fn new(config: MockStorageConfig) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            config,
            write_count: Arc::new(Mutex::new(0)),
            read_count: Arc::new(Mutex::new(0)),
            connected: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 获取写入次数
    pub fn get_write_count(&self) -> usize {
        *self.write_count.lock().unwrap()
    }
    
    /// 获取读取次数
    pub fn get_read_count(&self) -> usize {
        *self.read_count.lock().unwrap()
    }
    
    /// 获取存储的数据点数量
    pub async fn get_data_count(&self) -> usize {
        let data = self.data.read().await;
        data.values().map(|v| v.len()).sum()
    }
    
    /// 清空所有数据
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
        *self.write_count.lock().unwrap() = 0;
        *self.read_count.lock().unwrap() = 0;
    }
    
    /// 获取指定键的所有数据
    pub async fn get_all_data(&self, key: &str) -> Vec<DataPoint> {
        let data = self.data.read().await;
        data.get(key)
            .map(|deque| deque.iter().cloned().collect())
            .unwrap_or_default()
    }
    
    /// 设置是否应该失败
    pub fn set_should_fail_writes(&mut self, fail_after: Option<usize>) {
        self.config.fail_after_n_writes = fail_after;
    }
    
    /// 设置是否应该失败
    pub fn set_should_fail_reads(&mut self, fail_after: Option<usize>) {
        self.config.fail_after_n_reads = fail_after;
    }
}

#[async_trait]
impl Storage for MockStorage {
    async fn connect(&mut self) -> Result<()> {
        if self.config.connection_fail {
            return Err(crate::error::HisSrvError::ConnectionError(
                "Mock connection failure".to_string(),
            ));
        }
        
        *self.connected.write().await = true;
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        Ok(())
    }
    
    async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    async fn write(&mut self, point: DataPoint) -> Result<()> {
        if !self.is_connected().await {
            return Err(crate::error::HisSrvError::ConnectionError(
                "Not connected".to_string(),
            ));
        }
        
        // 模拟写入延迟
        if let Some(delay) = self.config.write_delay_ms {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
        
        // 检查是否应该失败
        let mut count = self.write_count.lock().unwrap();
        *count += 1;
        if let Some(fail_after) = self.config.fail_after_n_writes {
            if *count > fail_after {
                return Err(crate::error::HisSrvError::WriteError(
                    "Mock write failure".to_string(),
                ));
            }
        }
        
        // 存储数据
        let mut data = self.data.write().await;
        data.entry(point.key.clone())
            .or_insert_with(VecDeque::new)
            .push_back(point);
        
        Ok(())
    }
    
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()> {
        for point in points {
            self.write(point.clone()).await?;
        }
        Ok(())
    }
    
    async fn query(&self, key: &str, options: QueryOptions) -> Result<QueryResult> {
        if !self.is_connected().await {
            return Err(crate::error::HisSrvError::ConnectionError(
                "Not connected".to_string(),
            ));
        }
        
        // 模拟读取延迟
        if let Some(delay) = self.config.read_delay_ms {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
        
        // 检查是否应该失败
        let mut count = self.read_count.lock().unwrap();
        *count += 1;
        if let Some(fail_after) = self.config.fail_after_n_reads {
            if *count > fail_after {
                return Err(crate::error::HisSrvError::QueryError(
                    "Mock read failure".to_string(),
                ));
            }
        }
        
        let data = self.data.read().await;
        let points = data.get(key)
            .map(|deque| {
                deque.iter()
                    .filter(|p| {
                        p.timestamp >= options.start_time && p.timestamp <= options.end_time
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        
        // 应用限制
        let points = if let Some(limit) = options.limit {
            points.into_iter().take(limit).collect()
        } else {
            points
        };
        
        // 应用聚合
        let points = if let Some(aggregate) = options.aggregate {
            apply_aggregation(points, aggregate)
        } else {
            points
        };
        
        Ok(QueryResult {
            key: key.to_string(),
            points,
            metadata: HashMap::new(),
        })
    }
    
    async fn query_batch(&self, keys: &[String], options: QueryOptions) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();
        for key in keys {
            results.push(self.query(key, options.clone()).await?);
        }
        Ok(results)
    }
    
    async fn delete(&mut self, key: &str, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<usize> {
        if !self.is_connected().await {
            return Err(crate::error::HisSrvError::ConnectionError(
                "Not connected".to_string(),
            ));
        }
        
        let mut data = self.data.write().await;
        if let Some(points) = data.get_mut(key) {
            let before_len = points.len();
            points.retain(|p| p.timestamp < start_time || p.timestamp > end_time);
            let deleted = before_len - points.len();
            Ok(deleted)
        } else {
            Ok(0)
        }
    }
    
    fn backend_type(&self) -> &str {
        "mock"
    }
}

/// 应用聚合函数
fn apply_aggregation(points: Vec<DataPoint>, aggregate: AggregateFunction) -> Vec<DataPoint> {
    if points.is_empty() {
        return points;
    }
    
    let values: Vec<f64> = points.iter()
        .filter_map(|p| match &p.value {
            DataValue::Float(v) => Some(*v),
            DataValue::Integer(v) => Some(*v as f64),
            _ => None,
        })
        .collect();
    
    if values.is_empty() {
        return vec![];
    }
    
    let result_value = match aggregate {
        AggregateFunction::Mean => values.iter().sum::<f64>() / values.len() as f64,
        AggregateFunction::Sum => values.iter().sum(),
        AggregateFunction::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        AggregateFunction::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        AggregateFunction::Count => values.len() as f64,
        AggregateFunction::First => values[0],
        AggregateFunction::Last => values[values.len() - 1],
    };
    
    vec![DataPoint {
        key: points[0].key.clone(),
        value: DataValue::Float(result_value),
        timestamp: points[0].timestamp,
        tags: points[0].tags.clone(),
        metadata: {
            let mut meta = points[0].metadata.clone();
            meta.insert("aggregate".to_string(), format!("{:?}", aggregate));
            meta.insert("point_count".to_string(), values.len().to_string());
            meta
        },
    }]
}

/// 创建内存中的测试存储
pub fn create_memory_storage() -> MockStorage {
    MockStorage::new(MockStorageConfig::default())
}

/// 创建会失败的测试存储
pub fn create_failing_storage(fail_after_writes: usize, fail_after_reads: usize) -> MockStorage {
    MockStorage::new(MockStorageConfig {
        fail_after_n_writes: Some(fail_after_writes),
        fail_after_n_reads: Some(fail_after_reads),
        ..Default::default()
    })
}

/// 创建有延迟的测试存储
pub fn create_slow_storage(write_delay_ms: u64, read_delay_ms: u64) -> MockStorage {
    MockStorage::new(MockStorageConfig {
        write_delay_ms: Some(write_delay_ms),
        read_delay_ms: Some(read_delay_ms),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_storage_basic() {
        let mut storage = create_memory_storage();
        storage.connect().await.unwrap();
        
        let point = DataPoint {
            key: "test".to_string(),
            value: DataValue::Float(42.0),
            timestamp: Utc::now(),
            tags: HashMap::new(),
            metadata: HashMap::new(),
        };
        
        storage.write(point.clone()).await.unwrap();
        assert_eq!(storage.get_write_count(), 1);
        assert_eq!(storage.get_data_count().await, 1);
        
        let result = storage.query("test", QueryOptions {
            start_time: Utc::now() - chrono::Duration::hours(1),
            end_time: Utc::now() + chrono::Duration::hours(1),
            limit: None,
            aggregate: None,
            group_by: None,
            fill: None,
        }).await.unwrap();
        
        assert_eq!(result.points.len(), 1);
        assert_eq!(storage.get_read_count(), 1);
    }
    
    #[tokio::test]
    async fn test_mock_storage_failure() {
        let mut storage = create_failing_storage(2, 2);
        storage.connect().await.unwrap();
        
        let point = DataPoint {
            key: "test".to_string(),
            value: DataValue::Float(42.0),
            timestamp: Utc::now(),
            tags: HashMap::new(),
            metadata: HashMap::new(),
        };
        
        // 前两次写入成功
        storage.write(point.clone()).await.unwrap();
        storage.write(point.clone()).await.unwrap();
        
        // 第三次写入失败
        assert!(storage.write(point.clone()).await.is_err());
    }
}