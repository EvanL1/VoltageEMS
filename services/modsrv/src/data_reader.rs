//! 高效的数据读取器实现
//!
//! 提供优化的批量读取、缓存和订阅功能

use crate::comsrv_interface::{ComSrvInterface, PointCache, PointValue};
use crate::error::Result;
use crate::redis_handler::RedisConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::warn;

/// 数据读取策略
#[derive(Debug, Clone, PartialEq)]
pub enum ReadStrategy {
    /// 直接读取（无缓存）
    Direct,
    /// 带缓存的读取
    Cached { ttl: Duration },
    /// 批量读取优化
    Batch { size: usize },
    /// 订阅模式（推送）
    Subscribe,
}

/// 点位描述符
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PointDescriptor {
    pub channel_id: u16,
    pub point_type: String,
    pub point_id: u32,
}

impl PointDescriptor {
    pub fn new(channel_id: u16, point_type: &str, point_id: u32) -> Self {
        Self {
            channel_id,
            point_type: point_type.to_string(),
            point_id,
        }
    }

    pub fn to_key(&self) -> String {
        format!("{}:{}:{}", self.channel_id, self.point_type, self.point_id)
    }
}

/// 数据读取结果
#[derive(Debug, Clone)]
pub struct ReadResult {
    pub points: HashMap<PointDescriptor, PointValue>,
    pub read_time: Instant,
    pub strategy_used: ReadStrategy,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// 智能数据读取器
pub struct DataReader {
    interface: ComSrvInterface,
    cache: Arc<RwLock<PointCache>>,
    strategy: ReadStrategy,
    batch_buffer: Vec<PointDescriptor>,
    batch_size: usize,
    stats: ReadStats,
}

/// 读取统计
#[derive(Debug, Default)]
struct ReadStats {
    total_reads: u64,
    cache_hits: u64,
    cache_misses: u64,
    batch_reads: u64,
    avg_read_time: Duration,
}

impl DataReader {
    /// 创建新的数据读取器
    pub fn new(redis: RedisConnection, strategy: ReadStrategy) -> Self {
        let cache_ttl = match &strategy {
            ReadStrategy::Cached { ttl } => *ttl,
            _ => Duration::from_secs(1),
        };

        let batch_size = match &strategy {
            ReadStrategy::Batch { size } => *size,
            _ => 100,
        };

        Self {
            interface: ComSrvInterface::new(redis),
            cache: Arc::new(RwLock::new(PointCache::new(cache_ttl))),
            strategy,
            batch_buffer: Vec::new(),
            batch_size,
            stats: ReadStats::default(),
        }
    }

    /// 读取单个点位值
    pub fn read_point(&mut self, descriptor: &PointDescriptor) -> Result<Option<PointValue>> {
        let start = Instant::now();
        self.stats.total_reads += 1;

        let result = match &self.strategy {
            ReadStrategy::Direct => self.read_direct(descriptor),
            ReadStrategy::Cached { .. } => self.read_cached(descriptor),
            ReadStrategy::Batch { .. } => {
                // 对于批量策略，单个读取降级为直接读取
                self.read_direct(descriptor)
            }
            ReadStrategy::Subscribe => {
                // 订阅模式下，从缓存读取
                self.read_from_cache_only(descriptor)
            }
        };

        self.update_stats(start.elapsed());
        result
    }

    /// 批量读取点位值
    pub fn read_points(&mut self, descriptors: &[PointDescriptor]) -> Result<ReadResult> {
        let start = Instant::now();
        let mut points = HashMap::new();
        let mut cache_hits = 0;
        let mut cache_misses = 0;

        match &self.strategy {
            ReadStrategy::Batch { size } => {
                // 分批读取
                for chunk in descriptors.chunks(*size) {
                    let batch_result = self.read_batch(chunk)?;
                    points.extend(batch_result.0);
                    cache_hits += batch_result.1;
                    cache_misses += batch_result.2;
                }
            }
            _ => {
                // 其他策略逐个读取
                for desc in descriptors {
                    if let Some(value) = self.read_point(desc)? {
                        points.insert(desc.clone(), value);
                    }
                }
            }
        }

        Ok(ReadResult {
            points,
            read_time: start,
            strategy_used: self.strategy.clone(),
            cache_hits,
            cache_misses,
        })
    }

    /// 读取并监视点位变化
    pub fn read_and_watch(
        &mut self,
        descriptors: &[PointDescriptor],
        _callback: impl Fn(&PointDescriptor, &PointValue) + Send + 'static,
    ) -> Result<ReadResult> {
        // 首次读取所有点位
        let result = self.read_points(descriptors)?;

        // 设置监视（需要异步支持）
        warn!("Watch functionality requires async support - not implemented in sync mode");

        Ok(result)
    }

    // ===== 内部方法 =====

    /// 直接从Redis读取
    fn read_direct(&mut self, descriptor: &PointDescriptor) -> Result<Option<PointValue>> {
        self.interface.get_point_value(
            descriptor.channel_id,
            &descriptor.point_type,
            descriptor.point_id,
        )
    }

    /// 从缓存读取，未命中则从Redis读取
    fn read_cached(&mut self, descriptor: &PointDescriptor) -> Result<Option<PointValue>> {
        let key = descriptor.to_key();

        // 尝试从缓存读取
        {
            let cache = self.cache.read().unwrap();
            if let Some(value) = cache.get(&key) {
                self.stats.cache_hits += 1;
                return Ok(Some(value.clone()));
            }
        }

        // 缓存未命中，从Redis读取
        self.stats.cache_misses += 1;
        let value = self.read_direct(descriptor)?;

        // 更新缓存
        if let Some(ref val) = value {
            let mut cache = self.cache.write().unwrap();
            cache.set(key, val.clone());
        }

        Ok(value)
    }

    /// 仅从缓存读取
    fn read_from_cache_only(&self, descriptor: &PointDescriptor) -> Result<Option<PointValue>> {
        let key = descriptor.to_key();
        let cache = self.cache.read().unwrap();
        Ok(cache.get(&key).cloned())
    }

    /// 批量读取实现
    fn read_batch(
        &mut self,
        descriptors: &[PointDescriptor],
    ) -> Result<(HashMap<PointDescriptor, PointValue>, usize, usize)> {
        let mut results = HashMap::new();
        let mut cache_hits = 0;
        let mut to_fetch = Vec::new();

        // 首先检查缓存
        {
            let cache = self.cache.read().unwrap();
            for desc in descriptors {
                let key = desc.to_key();
                if let Some(value) = cache.get(&key) {
                    results.insert(desc.clone(), value.clone());
                    cache_hits += 1;
                } else {
                    to_fetch.push(desc);
                }
            }
        }

        // 批量获取未缓存的数据
        if !to_fetch.is_empty() {
            let fetch_params: Vec<(u16, &str, u32)> = to_fetch
                .iter()
                .map(|d| (d.channel_id, d.point_type.as_str(), d.point_id))
                .collect();

            let batch_result = self.interface.batch_get_points(&fetch_params)?;
            let mut cache = self.cache.write().unwrap();

            for desc in to_fetch {
                let key = desc.to_key();
                if let Some(Some(value)) = batch_result.get(&key) {
                    results.insert(desc.clone(), value.clone());
                    cache.set(key, value.clone());
                }
            }
        }

        let cache_misses = descriptors.len() - cache_hits;
        self.stats.batch_reads += 1;

        Ok((results, cache_hits, cache_misses))
    }

    /// 更新统计信息
    fn update_stats(&mut self, read_time: Duration) {
        let total = self.stats.total_reads as u32;
        if total > 0 {
            let avg_nanos = self.stats.avg_read_time.as_nanos() as u64;
            let new_avg_nanos =
                (avg_nanos * (total - 1) as u64 + read_time.as_nanos() as u64) / total as u64;
            self.stats.avg_read_time = Duration::from_nanos(new_avg_nanos);
        }
    }

    /// 获取读取统计
    pub fn get_stats(&self) -> ReadStatsSummary {
        ReadStatsSummary {
            total_reads: self.stats.total_reads,
            cache_hit_rate: if self.stats.total_reads > 0 {
                self.stats.cache_hits as f64 / self.stats.total_reads as f64
            } else {
                0.0
            },
            avg_read_time: self.stats.avg_read_time,
            batch_reads: self.stats.batch_reads,
        }
    }

    /// 清理过期缓存
    pub fn cleanup_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear_expired();
    }
}

/// 读取统计摘要
#[derive(Debug, Clone)]
pub struct ReadStatsSummary {
    pub total_reads: u64,
    pub cache_hit_rate: f64,
    pub avg_read_time: Duration,
    pub batch_reads: u64,
}

/// 数据聚合器 - 支持多点聚合计算
pub struct DataAggregator {
    reader: DataReader,
}

impl DataAggregator {
    pub fn new(reader: DataReader) -> Self {
        Self { reader }
    }

    /// 读取并聚合多个点位的值
    pub fn aggregate<F>(
        &mut self,
        descriptors: &[PointDescriptor],
        aggregator: F,
    ) -> Result<Option<f64>>
    where
        F: Fn(&[f64]) -> f64,
    {
        let result = self.reader.read_points(descriptors)?;

        if result.points.is_empty() {
            return Ok(None);
        }

        let values: Vec<f64> = result.points.values().map(|pv| pv.value).collect();
        Ok(Some(aggregator(&values)))
    }

    /// 常用聚合函数
    pub fn sum(&mut self, descriptors: &[PointDescriptor]) -> Result<Option<f64>> {
        self.aggregate(descriptors, |values| values.iter().sum())
    }

    pub fn average(&mut self, descriptors: &[PointDescriptor]) -> Result<Option<f64>> {
        self.aggregate(descriptors, |values| {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        })
    }

    pub fn max(&mut self, descriptors: &[PointDescriptor]) -> Result<Option<f64>> {
        self.aggregate(descriptors, |values| {
            values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        })
    }

    pub fn min(&mut self, descriptors: &[PointDescriptor]) -> Result<Option<f64>> {
        self.aggregate(descriptors, |values| {
            values.iter().cloned().fold(f64::INFINITY, f64::min)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_descriptor() {
        let desc = PointDescriptor::new(1001, "m", 10001);
        assert_eq!(desc.to_key(), "1001:m:10001");
    }

    #[test]
    fn test_read_strategy() {
        let strategy1 = ReadStrategy::Direct;
        let strategy2 = ReadStrategy::Cached {
            ttl: Duration::from_secs(60),
        };
        assert_ne!(strategy1, strategy2);
    }

    #[test]
    fn test_stats_calculation() {
        let stats = ReadStatsSummary {
            total_reads: 1000,
            cache_hit_rate: 0.85,
            avg_read_time: Duration::from_micros(50),
            batch_reads: 10,
        };

        assert_eq!(stats.cache_hit_rate, 0.85);
        assert_eq!(stats.total_reads, 1000);
    }
}
