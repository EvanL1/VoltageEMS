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
use crate::redis_handler::RedisConnection;

/// Hybrid store implementation, combining memory and Redis
pub struct HybridStore {
    memory: Arc<MemoryStore>,
    redis: Option<Arc<RedisStore>>,
    sync_mode: SyncMode,
}

impl HybridStore {
    /// Create a new hybrid store
    pub fn new(config: &Config, sync_mode: SyncMode) -> Result<Self> {
        let memory = Arc::new(MemoryStore::new());
        
        let redis = if config.use_redis {
            // Create a RedisConnection from the config
            let redis_conn = RedisConnection::from_config(&config.redis)?;
            
            // Create the RedisStore with the connection
            Some(Arc::new(RedisStore::new(redis_conn)))
        } else {
            None
        };
        
        Ok(Self {
            memory,
            redis,
            sync_mode,
        })
    }
    
    /// Load data from Redis to memory
    pub fn load_from_redis(&self, pattern: &str) -> Result<()> {
        if let Some(redis) = &self.redis {
            let keys = redis.get_keys(pattern)?;
            let mut keys_count = 0;
            
            for key in &keys {
                // Check key type
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
                        // Skip other types for now
                        debug!("Skipping key '{}' with unsupported type", key);
                    }
                }
            }
            
            info!("Loaded {} keys from Redis to memory", keys_count);
        }
        Ok(())
    }
    
    /// Sync memory data to Redis
    pub fn sync_to_redis(&self, pattern: &str) -> Result<()> {
        if let Some(redis) = &self.redis {
            let keys = self.memory.get_keys(pattern)?;
            let mut synced = 0;
            
            for key in keys {
                // Try to get string value
                match self.memory.get_string(&key) {
                    Ok(value) => {
                        redis.set_string(&key, &value)?;
                        synced += 1;
                    },
                    Err(_) => {
                        // Try to get hash value
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
    
    /// Get the memory store
    pub fn memory_store(&self) -> Arc<MemoryStore> {
        self.memory.clone()
    }
    
    /// Get the Redis store
    pub fn redis_store(&self) -> Option<Arc<RedisStore>> {
        self.redis.clone()
    }
    
    /// Get the sync mode
    pub fn sync_mode(&self) -> &SyncMode {
        &self.sync_mode
    }

    /// Print memory data for debugging purposes
    pub fn dump_memory_data(&self, pattern: &str) -> Result<HashMap<String, serde_json::Value>> {
        let keys = self.memory.get_keys(pattern)?;
        let mut result = HashMap::new();
        
        for key in keys {
            // Try to get string value
            match self.memory.get_string(&key) {
                Ok(value) => {
                    result.insert(key, serde_json::Value::String(value));
                },
                Err(_) => {
                    // Try to get hash value
                    match self.memory.get_hash(&key) {
                        Ok(hash) => {
                            let hash_map: serde_json::Map<String, serde_json::Value> = hash
                                .into_iter()
                                .map(|(k, v)| (k, serde_json::Value::String(v)))
                                .collect();
                            result.insert(key, serde_json::Value::Object(hash_map));
                        },
                        Err(e) => {
                            debug!("Failed to dump key '{}': {}", key, e);
                        }
                    }
                }
            }
        }
        
        Ok(result)
    }
}

impl DataStore for HybridStore {
    fn get_string(&self, key: &str) -> Result<String> {
        self.memory.get_string(key)
    }
    
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        // Write to memory first
        self.memory.set_string(key, value)?;
        
        // Decide whether to write to Redis based on sync mode
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_string(key, value)?;
                    debug!("WriteThrough: Synced key '{}' to Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // Handled by background thread, not here
                    debug!("WriteBack: Key '{}' will be synced later", key);
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
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
        // Write to memory first
        self.memory.set_hash(key, hash)?;
        
        // Decide whether to write to Redis based on sync mode
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_hash(key, hash)?;
                    debug!("WriteThrough: Synced hash '{}' to Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // Handled by background thread, not here
                    debug!("WriteBack: Hash '{}' will be synced later", key);
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
                    debug!("OnDemand: Hash '{}' will be synced on demand", key);
                }
            }
        }
        
        Ok(())
    }
    
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()> {
        // Write to memory first
        self.memory.set_hash_field(key, field, value)?;
        
        // Decide whether to write to Redis based on sync mode
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.set_hash_field(key, field, value)?;
                    debug!("WriteThrough: Synced hash field '{}:{}' to Redis", key, field);
                },
                SyncMode::WriteBack(_) => {
                    // Handled by background thread, not here
                    debug!("WriteBack: Hash field '{}:{}' will be synced later", key, field);
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
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
        // Delete from memory first
        self.memory.delete(key)?;
        
        // Decide whether to delete from Redis based on sync mode
        if let Some(redis) = &self.redis {
            match &self.sync_mode {
                SyncMode::WriteThrough => {
                    redis.delete(key)?;
                    debug!("WriteThrough: Deleted key '{}' from Redis", key);
                },
                SyncMode::WriteBack(_) => {
                    // Handled by background thread, not here
                    debug!("WriteBack: Key '{}' will be deleted from Redis later", key);
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
                    debug!("OnDemand: Key '{}' will be deleted from Redis on demand", key);
                }
            }
        }
        
        Ok(())
    }
}

/// Sync service, responsible for periodically syncing memory data to Redis
pub struct SyncService {
    store: Arc<HybridStore>,
    interval: Duration,
    patterns: Vec<String>,
    shutdown: Arc<AtomicBool>,
    handle: RwLock<Option<JoinHandle<()>>>,
}

impl SyncService {
    /// Create a new sync service
    pub fn new(store: Arc<HybridStore>, interval: Duration, patterns: Vec<String>) -> Self {
        Self {
            store,
            interval,
            patterns,
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: RwLock::new(None),
        }
    }
    
    /// Start the sync service
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
    
    /// Stop the sync service
    pub fn stop(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        
        let mut handle_guard = self.handle.write().map_err(|_| ModelSrvError::LockError)?;
        if let Some(handle) = handle_guard.take() {
            // In a real application, you might want to wait for the task to complete
            handle.abort();
            info!("Sync service stopped");
        }
        
        Ok(())
    }
}