use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::interval;
use log::{error, info, debug};
use std::collections::HashMap;

use crate::error::{Result, ModelSrvError};
use crate::config::Config;
use super::{DataStore, SyncMode};
use super::memory_store::MemoryStore;
use super::redis_store::RedisStore;

/// 混合存储实现，结合内存和Redis
pub struct HybridStore {
    memory: Arc<MemoryStore>,
    redis: Option<Arc<RedisStore>>,
    sync_mode: SyncMode,
}

impl HybridStore {
    /// 创建新的混合存储
    pub fn new(config: &Config, sync_mode: SyncMode) -> Result<Self> {
        let memory = Arc::new(MemoryStore::new());
        
        let redis = if config.use_redis {
            Some(Arc::new(RedisStore::new(config)?))
        } else {
            None
        };
        
        Ok(Self {
            memory,
            redis,
            sync_mode,
        })
    }
    
    /// 从Redis加载数据到内存
    pub fn load_from_redis(&self, pattern: &str) -> Result<()> {
        if let Some(redis) = &self.redis {
            let keys = redis.get_keys(pattern)?;
            let mut keys_count = 0;
            
            for key in &keys {
                // 检查键的类型
                let key_type = redis.get_type(key)?;
                
                match key_type {
                    crate::redis_handler::RedisType::String => {
                        let value = redis.get_string(key)?;
                        self.memory.set_string(key, &value)?;
                        keys_count += 1;
                    },
                    crate::redis_handler::RedisType::Hash => {
                        let hash = redis.get_hash(key)?;
                        self.memory.set_hash(key, &hash)?;
                        keys_count += 1;
                    },
                    _ => {
                        // 其他类型暂不处理
                        debug!("Skipping key '{}' with unsupported type", key);
                    }
                }
            }
            
            info!("Loaded {} keys from Redis to memory", keys_count);
        }
        Ok(())
    }
    
    /// 将内存数据同步到Redis
    pub fn sync_to_redis(&self, pattern: &str) -> Result<()> {
        if let Some(redis) = &self.redis {
            let keys = self.memory.get_keys(pattern)?;
            let mut synced = 0;
            
            for key in keys {
                // 尝试获取字符串值
                match self.memory.get_string(&key) {
                    Ok(value) => {
                        redis.set_string(&key, &value)?;
                        synced += 1;
                    },
                    Err(_) => {
                        // 尝试获取哈希值
                        match self.memory.get_hash(&key) {
                            Ok(hash) => {
                                redis.set_hash(&key, &hash)?;
                                synced += 1;
                            },
                            Err(e) => {
                                debug!("Failed to sync key '{}': {}", key, e);
                            }
                        }
                    }
                }
            }
            
            debug!("Synced {} keys from memory to Redis", synced);
        }
        Ok(())
    }
    
    /// 获取内存存储
    pub fn memory_store(&self) -> Arc<MemoryStore> {
        self.memory.clone()
    }
    
    /// 获取Redis存储
    pub fn redis_store(&self) -> Option<Arc<RedisStore>> {
        self.redis.clone()
    }
    
    /// 获取同步模式
    pub fn sync_mode(&self) -> &SyncMode {
        &self.sync_mode
    }
}

impl DataStore for HybridStore {
    fn get_string(&self, key: &str) -> Result<String> {
        self.memory.get_string(key)
    }
    
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        // 先写入内存
        self.memory.set_string(key, value)?;
        
        // 根据同步模式决定是否写入Redis
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_string(key, value)?;
                    debug!("WriteThrough: Synced key '{}' to Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // 在后台线程中处理，不在这里同步
                    debug!("WriteBack: Key '{}' will be synced later", key);
                },
                SyncMode::OnDemand => {
                    // 不做任何事，等待显式同步调用
                    debug!("OnDemand: Key '{}' will be synced on demand", key);
                }
            }
        }
        
        Ok(())
    }
    
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>> {
        self.memory.get_hash(key)
    }
    
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        // 先写入内存
        self.memory.set_hash(key, hash)?;
        
        // 根据同步模式决定是否写入Redis
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_hash(key, hash)?;
                    debug!("WriteThrough: Synced hash '{}' to Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // 在后台线程中处理，不在这里同步
                    debug!("WriteBack: Hash '{}' will be synced later", key);
                },
                SyncMode::OnDemand => {
                    // 不做任何事，等待显式同步调用
                    debug!("OnDemand: Hash '{}' will be synced on demand", key);
                }
            }
        }
        
        Ok(())
    }
    
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()> {
        // 先写入内存
        self.memory.set_hash_field(key, field, value)?;
        
        // 根据同步模式决定是否写入Redis
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_hash_field(key, field, value)?;
                    debug!("WriteThrough: Synced hash field '{}:{}' to Redis", key, field);
                },
                SyncMode::WriteBack(_) => {
                    // 在后台线程中处理，不在这里同步
                    debug!("WriteBack: Hash field '{}:{}' will be synced later", key, field);
                },
                SyncMode::OnDemand => {
                    // 不做任何事，等待显式同步调用
                    debug!("OnDemand: Hash field '{}:{}' will be synced on demand", key, field);
                }
            }
        }
        
        Ok(())
    }
    
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        self.memory.get_keys(pattern)
    }
    
    fn exists(&self, key: &str) -> Result<bool> {
        self.memory.exists(key)
    }
    
    fn delete(&self, key: &str) -> Result<()> {
        // 先从内存删除
        self.memory.delete(key)?;
        
        // 根据同步模式决定是否从Redis删除
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.delete(key)?;
                    debug!("WriteThrough: Deleted key '{}' from Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // 在后台线程中处理，不在这里同步
                    debug!("WriteBack: Key '{}' will be deleted from Redis later", key);
                },
                SyncMode::OnDemand => {
                    // 不做任何事，等待显式同步调用
                    debug!("OnDemand: Key '{}' will be deleted from Redis on demand", key);
                }
            }
        }
        
        Ok(())
    }
}

/// 同步服务，负责定期将内存数据同步到Redis
pub struct SyncService {
    store: Arc<HybridStore>,
    interval: Duration,
    patterns: Vec<String>,
    shutdown: Arc<AtomicBool>,
    handle: RwLock<Option<JoinHandle<()>>>,
}

impl SyncService {
    /// 创建新的同步服务
    pub fn new(store: Arc<HybridStore>, interval: Duration, patterns: Vec<String>) -> Self {
        Self {
            store,
            interval,
            patterns,
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: RwLock::new(None),
        }
    }
    
    /// 启动同步服务
    pub fn start(&self) -> Result<()> {
        let store = self.store.clone();
        let interval_duration = self.interval;
        let patterns = self.patterns.clone();
        let shutdown = self.shutdown.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval_timer = interval(interval_duration);
            
            loop {
                interval_timer.tick().await;
                
                if shutdown.load(Ordering::Relaxed) {
                    info!("Sync service shutting down");
                    break;
                }
                
                for pattern in &patterns {
                    if let Err(e) = store.sync_to_redis(pattern) {
                        error!("Failed to sync to Redis: {}", e);
                    }
                }
            }
        });
        
        let mut handle_guard = self.handle.write().map_err(|_| ModelSrvError::LockError)?;
        *handle_guard = Some(handle);
        
        info!("Sync service started with interval {:?}", interval_duration);
        Ok(())
    }
    
    /// 停止同步服务
    pub fn stop(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        
        let mut handle_guard = self.handle.write().map_err(|_| ModelSrvError::LockError)?;
        if let Some(handle) = handle_guard.take() {
            // 在实际应用中，可能需要等待任务完成
            handle.abort();
            info!("Sync service stopped");
        }
        
        Ok(())
    }
} 