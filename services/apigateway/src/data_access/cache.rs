use crate::data_access::{CacheStats, DataAccessError, DataAccessResult};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 缓存项
#[derive(Debug, Clone)]
struct CacheItem {
    value: Value,
    created_at: Instant,
    ttl: Option<Duration>,
    access_count: u64,
}

impl CacheItem {
    fn new(value: Value, ttl: Option<Duration>) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
        }
    }

    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed() > ttl
        } else {
            false
        }
    }

    fn touch(&mut self) {
        self.access_count += 1;
    }
}

/// 本地LRU缓存
#[derive(Clone)]
pub struct LocalCache {
    data: Arc<RwLock<HashMap<String, CacheItem>>>,
    max_size: usize,
    stats: Arc<RwLock<CacheStats>>,
}

impl LocalCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            stats: Arc::new(RwLock::new(CacheStats::new())),
        }
    }

    /// 获取缓存项
    pub async fn get(&self, key: &str) -> DataAccessResult<Option<Value>> {
        let mut data = self.data.write().await;
        
        if let Some(item) = data.get_mut(key) {
            if item.is_expired() {
                data.remove(key);
                self.stats.write().await.record_miss();
                return Ok(None);
            }
            
            item.touch();
            self.stats.write().await.record_hit();
            Ok(Some(item.value.clone()))
        } else {
            self.stats.write().await.record_miss();
            Ok(None)
        }
    }

    /// 设置缓存项
    pub async fn set(&self, key: String, value: Value, ttl: Option<Duration>) -> DataAccessResult<()> {
        let mut data = self.data.write().await;
        
        // 如果超过最大大小，清理旧项
        if data.len() >= self.max_size {
            self.evict_lru(&mut data).await;
        }
        
        data.insert(key, CacheItem::new(value, ttl));
        Ok(())
    }

    /// 删除缓存项
    pub async fn remove(&self, key: &str) -> DataAccessResult<bool> {
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }

    /// 清理过期项
    pub async fn cleanup_expired(&self) -> DataAccessResult<usize> {
        let mut data = self.data.write().await;
        let mut expired_keys = Vec::new();
        
        for (key, item) in data.iter() {
            if item.is_expired() {
                expired_keys.push(key.clone());
            }
        }
        
        let count = expired_keys.len();
        for key in expired_keys {
            data.remove(&key);
        }
        
        Ok(count)
    }

    /// LRU驱逐策略
    async fn evict_lru(&self, data: &mut HashMap<String, CacheItem>) {
        if data.is_empty() {
            return;
        }
        
        // 找到最少使用的项
        let lru_key = data
            .iter()
            .min_by_key(|(_, item)| (item.access_count, item.created_at))
            .map(|(k, _)| k.clone());
            
        if let Some(key) = lru_key {
            data.remove(&key);
        }
    }

    /// 获取缓存统计
    pub async fn stats(&self) -> CacheStats {
        let mut stats = self.stats.read().await.clone();
        let data = self.data.read().await;
        
        stats.total_keys = data.len();
        stats.memory_usage = data.len() * std::mem::size_of::<CacheItem>(); // 简化计算
        
        stats
    }

    /// 清空缓存
    pub async fn clear(&self) -> DataAccessResult<()> {
        self.data.write().await.clear();
        Ok(())
    }
}

/// Redis缓存包装器
pub struct RedisCache {
    client: Arc<crate::redis_client::RedisClient>,
    key_prefix: String,
}

impl RedisCache {
    pub fn new(client: Arc<crate::redis_client::RedisClient>, key_prefix: String) -> Self {
        Self {
            client,
            key_prefix,
        }
    }

    fn make_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }

    /// 获取缓存项
    pub async fn get(&self, key: &str) -> DataAccessResult<Option<Value>> {
        let redis_key = self.make_key(key);
        
        match self.client.get(&redis_key).await {
            Ok(Some(data)) => {
                match serde_json::from_str(&data) {
                    Ok(value) => Ok(Some(value)),
                    Err(e) => Err(DataAccessError::Serialization(e.to_string())),
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(DataAccessError::Redis(e.to_string())),
        }
    }

    /// 设置缓存项
    pub async fn set(&self, key: &str, value: &Value, ttl: Option<u64>) -> DataAccessResult<()> {
        let redis_key = self.make_key(key);
        let data = serde_json::to_string(value)
            .map_err(|e| DataAccessError::Serialization(e.to_string()))?;

        let result = if let Some(ttl_seconds) = ttl {
            self.client.set_ex(&redis_key, &data, ttl_seconds).await
        } else {
            self.client.set(&redis_key, &data).await
        };

        result.map_err(|e| DataAccessError::Redis(e.to_string()))
    }

    /// 删除缓存项
    pub async fn remove(&self, key: &str) -> DataAccessResult<bool> {
        let redis_key = self.make_key(key);
        
        match self.client.del(&[redis_key.as_str()]).await {
            Ok(_) => Ok(true),
            Err(e) => Err(DataAccessError::Redis(e.to_string())),
        }
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> DataAccessResult<bool> {
        let redis_key = self.make_key(key);
        
        self.client
            .exists(&redis_key)
            .await
            .map_err(|e| DataAccessError::Redis(e.to_string()))
    }

    /// 批量删除（支持模式匹配）
    pub async fn clear_pattern(&self, pattern: &str) -> DataAccessResult<u64> {
        let redis_pattern = self.make_key(pattern);
        
        match self.client.keys(&redis_pattern).await {
            Ok(keys) => {
                let mut count = 0;
                for key in keys {
                    if self.client.del(&[key.as_str()]).await.is_ok() {
                        count += 1;
                    }
                }
                Ok(count)
            }
            Err(e) => Err(DataAccessError::Redis(e.to_string())),
        }
    }
}

/// 分层缓存管理器
pub struct TieredCache {
    local: LocalCache,
    redis: RedisCache,
}

impl TieredCache {
    pub fn new(
        local_size: usize,
        redis_client: Arc<crate::redis_client::RedisClient>,
        redis_prefix: String,
    ) -> Self {
        Self {
            local: LocalCache::new(local_size),
            redis: RedisCache::new(redis_client, redis_prefix),
        }
    }

    /// 获取数据（L1本地缓存 -> L2 Redis缓存）
    pub async fn get(&self, key: &str) -> DataAccessResult<Option<Value>> {
        // 1. 尝试本地缓存
        if let Ok(Some(value)) = self.local.get(key).await {
            return Ok(Some(value));
        }

        // 2. 尝试Redis缓存
        if let Ok(Some(value)) = self.redis.get(key).await {
            // 写入本地缓存（异步，不阻塞）
            let local = self.local.clone();
            let key_clone = key.to_string();
            let value_clone = value.clone();
            tokio::spawn(async move {
                let _ = local.set(key_clone, value_clone, Some(Duration::from_secs(60))).await;
            });
            
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// 设置数据（同时写入L1和L2）
    pub async fn set(&self, key: &str, value: Value, ttl: Option<u64>) -> DataAccessResult<()> {
        // 写入本地缓存
        let local_ttl = ttl.map(|t| Duration::from_secs(t.min(300))); // 本地缓存最多5分钟
        self.local.set(key.to_string(), value.clone(), local_ttl).await?;

        // 写入Redis缓存
        self.redis.set(key, &value, ttl).await?;

        Ok(())
    }

    /// 删除数据
    pub async fn remove(&self, key: &str) -> DataAccessResult<()> {
        // 从两级缓存都删除
        let _ = self.local.remove(key).await;
        let _ = self.redis.remove(key).await;
        Ok(())
    }

    /// 清理缓存
    pub async fn clear_pattern(&self, pattern: &str) -> DataAccessResult<u64> {
        // 清理本地缓存（简单起见，清空所有）
        let _ = self.local.clear().await;
        
        // 清理Redis缓存
        self.redis.clear_pattern(pattern).await
    }

    /// 获取统计信息
    pub async fn stats(&self) -> CacheStats {
        self.local.stats().await
    }
}

/// 缓存清理任务
pub struct CacheCleanupTask {
    local_cache: Arc<LocalCache>,
    cleanup_interval: Duration,
}

impl CacheCleanupTask {
    pub fn new(local_cache: Arc<LocalCache>, cleanup_interval: Duration) -> Self {
        Self {
            local_cache,
            cleanup_interval,
        }
    }

    /// 启动清理任务
    pub async fn start(&self) {
        let cache = self.local_cache.clone();
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut cleanup_timer = tokio::time::interval(interval);

            loop {
                cleanup_timer.tick().await;
                
                if let Ok(cleaned) = cache.cleanup_expired().await {
                    if cleaned > 0 {
                        log::debug!("Cleaned {} expired cache items", cleaned);
                    }
                }
            }
        });
    }
}