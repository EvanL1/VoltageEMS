//! 缓存模块
//!
//! 为modsrv提供高性能的内存缓存和数据管理

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

/// 缓存项
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    /// 缓存的值
    pub value: T,
    /// 最后更新时间
    pub last_updated: Instant,
    /// 过期时间
    pub expires_at: Option<Instant>,
}

impl<T> CacheEntry<T> {
    /// 创建新的缓存项
    pub fn new(value: T, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        Self {
            value,
            last_updated: now,
            expires_at: ttl.map(|duration| now + duration),
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }

    /// 更新值和时间戳
    pub fn update(&mut self, value: T, ttl: Option<Duration>) {
        self.value = value;
        self.last_updated = Instant::now();
        self.expires_at = ttl.map(|duration| self.last_updated + duration);
    }
}

/// 点位数据缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointData {
    /// 点位ID
    pub point_id: u32,
    /// 通道ID
    pub channel_id: u16,
    /// 当前值
    pub value: f64,
    /// 质量标志
    pub quality: String,
    /// 时间戳（毫秒）
    pub timestamp: i64,
}

/// 模型缓存管理器
pub struct ModelCacheManager {
    /// 点位数据缓存 - key: "{channel_id}:{point_id}"
    point_cache: Arc<RwLock<HashMap<String, CacheEntry<PointData>>>>,
    /// 模型输出缓存 - key: model_id
    model_output_cache: Arc<RwLock<HashMap<String, CacheEntry<serde_json::Value>>>>,
    /// 默认TTL
    default_ttl: Duration,
    /// 缓存统计
    stats: Arc<RwLock<CacheStats>>,
}

/// 缓存统计信息
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub updates: u64,
    pub evictions: u64,
}

impl ModelCacheManager {
    /// 创建新的缓存管理器
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            point_cache: Arc::new(RwLock::new(HashMap::new())),
            model_output_cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// 获取或更新点位数据
    pub async fn get_or_update_point<F>(
        &self,
        key: &str,
        fetch_fn: F,
    ) -> Result<PointData, Box<dyn std::error::Error>>
    where
        F: std::future::Future<Output = Result<PointData, Box<dyn std::error::Error>>>,
    {
        // 先尝试从缓存获取
        {
            let cache = self.point_cache.read().await;
            if let Some(entry) = cache.get(key) {
                if !entry.is_expired() {
                    let mut stats = self.stats.write().await;
                    stats.hits += 1;
                    return Ok(entry.value.clone());
                }
            }
        }

        // 缓存未命中或已过期，从数据源获取
        let mut stats = self.stats.write().await;
        stats.misses += 1;
        drop(stats);

        let data = fetch_fn.await?;

        // 更新缓存
        {
            let mut cache = self.point_cache.write().await;
            let entry = CacheEntry::new(data.clone(), Some(self.default_ttl));
            cache.insert(key.to_string(), entry);

            let mut stats = self.stats.write().await;
            stats.updates += 1;
        }

        Ok(data)
    }

    /// 批量获取点位数据
    pub async fn get_points_batch(&self, keys: &[String]) -> HashMap<String, PointData> {
        let cache = self.point_cache.read().await;
        let mut result = HashMap::new();
        let mut stats = self.stats.write().await;

        for key in keys {
            if let Some(entry) = cache.get(key) {
                if !entry.is_expired() {
                    result.insert(key.clone(), entry.value.clone());
                    stats.hits += 1;
                } else {
                    stats.misses += 1;
                }
            } else {
                stats.misses += 1;
            }
        }

        result
    }

    /// 批量更新点位数据
    pub async fn update_points_batch(&self, updates: HashMap<String, PointData>) {
        let mut cache = self.point_cache.write().await;
        let mut stats = self.stats.write().await;

        for (key, data) in updates {
            let entry = CacheEntry::new(data, Some(self.default_ttl));
            cache.insert(key, entry);
            stats.updates += 1;
        }
    }

    /// 获取模型输出缓存
    pub async fn get_model_output(&self, model_id: &str) -> Option<serde_json::Value> {
        let cache = self.model_output_cache.read().await;
        if let Some(entry) = cache.get(model_id) {
            if !entry.is_expired() {
                let mut stats = self.stats.write().await;
                stats.hits += 1;
                return Some(entry.value.clone());
            }
        }

        let mut stats = self.stats.write().await;
        stats.misses += 1;
        None
    }

    /// 更新模型输出缓存
    pub async fn update_model_output(&self, model_id: String, output: serde_json::Value) {
        let mut cache = self.model_output_cache.write().await;
        let entry = CacheEntry::new(output, Some(self.default_ttl));
        cache.insert(model_id, entry);

        let mut stats = self.stats.write().await;
        stats.updates += 1;
    }

    /// 清理过期缓存项
    pub async fn cleanup_expired(&self) {
        let mut evicted_count = 0;

        // 清理点位缓存
        {
            let mut cache = self.point_cache.write().await;
            cache.retain(|_, entry| {
                if entry.is_expired() {
                    evicted_count += 1;
                    false
                } else {
                    true
                }
            });
        }

        // 清理模型输出缓存
        {
            let mut cache = self.model_output_cache.write().await;
            cache.retain(|_, entry| {
                if entry.is_expired() {
                    evicted_count += 1;
                    false
                } else {
                    true
                }
            });
        }

        if evicted_count > 0 {
            let mut stats = self.stats.write().await;
            stats.evictions += evicted_count;
            info!("Evicted {} expired cache entries", evicted_count);
        }
    }

    /// 获取缓存统计信息
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// 重置缓存统计
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CacheStats::default();
    }

    /// 清空所有缓存
    pub async fn clear_all(&self) {
        self.point_cache.write().await.clear();
        self.model_output_cache.write().await.clear();
        info!("Cleared all caches");
    }

    /// 获取缓存大小信息
    pub async fn get_cache_info(&self) -> CacheInfo {
        let point_cache_size = self.point_cache.read().await.len();
        let model_cache_size = self.model_output_cache.read().await.len();
        let stats = self.stats.read().await.clone();

        CacheInfo {
            point_cache_entries: point_cache_size,
            model_cache_entries: model_cache_size,
            total_entries: point_cache_size + model_cache_size,
            stats,
        }
    }
}

/// 缓存信息
#[derive(Debug, Clone)]
pub struct CacheInfo {
    pub point_cache_entries: usize,
    pub model_cache_entries: usize,
    pub total_entries: usize,
    pub stats: CacheStats,
}

impl CacheStats {
    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64) / (total as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_entry() {
        let entry = CacheEntry::new("test_value", Some(Duration::from_secs(1)));
        assert!(!entry.is_expired());

        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(entry.is_expired());
    }

    #[tokio::test]
    async fn test_point_cache() {
        let manager = ModelCacheManager::new(Duration::from_secs(60));

        let point = PointData {
            point_id: 1001,
            channel_id: 1,
            value: 42.5,
            quality: "Good".to_string(),
            timestamp: 1234567890,
        };

        // 更新缓存
        let mut updates = HashMap::new();
        updates.insert("1:1001".to_string(), point.clone());
        manager.update_points_batch(updates).await;

        // 获取缓存
        let cached = manager.get_points_batch(&["1:1001".to_string()]).await;
        assert_eq!(cached.len(), 1);
        assert_eq!(cached.get("1:1001").unwrap().value, 42.5);

        // 检查统计
        let stats = manager.get_stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.updates, 1);
    }
}
