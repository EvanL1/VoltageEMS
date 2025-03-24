use std::sync::{Arc, RwLock};
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
use crate::template::TemplateManager;

/// HybridStore: Combines memory and Redis storage.
/// - WriteThrough: data is stored in memory and immediately synced to Redis
/// - WriteBack(x): data is written to memory, and synced to Redis every `x` duration
/// - OnDemand: data is written to memory, and synced manually
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
            // Create JSON configuration
            let redis_config = serde_json::json!({
                "host": config.redis.host,
                "port": config.redis.port
            });
            
            // Create a RedisConnection from the config
            let redis_conn = RedisConnection::from_config(&redis_config)?;
            
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
    
    /// Get a rule by ID
    pub fn get_rule(&self, rule_id: &str) -> Result<serde_json::Value> {
        let key = format!("rule:{}", rule_id);
        match self.get_string(&key) {
            Ok(rule_str) => {
                serde_json::from_str(&rule_str)
                    .map_err(|e| ModelSrvError::SerdeError(e))
            },
            Err(e) => Err(e)
        }
    }

    /// Add execution history for a rule
    pub fn add_execution_history(&self, rule_id: &str, history: &serde_json::Value) -> Result<()> {
        let key = format!("rule:{}:history", rule_id);
        let history_str = serde_json::to_string(history)
            .map_err(|e| ModelSrvError::SerdeError(e))?;
        
        // Add to list in Redis or memory
        if let Some(redis) = &self.redis {
            let key_list = format!("rule:{}:history", rule_id);
            self.memory.set_string(&key_list, &history_str)?;
            
            // 如果是 WriteThrough 模式，也写入 Redis
            if matches!(self.sync_mode, SyncMode::WriteThrough) {
                redis.set_string(&key_list, &history_str)?;
            }
        } else {
            // If we don't have Redis, store in memory as a string with the execution ID as a field
            let execution_id = history["execution_id"].as_str()
                .ok_or_else(|| ModelSrvError::InvalidData("Missing execution_id in history".to_string()))?;
            
            let existing_hash = match self.get_hash(&key) {
                Ok(hash) => hash,
                Err(_) => HashMap::new()
            };
            
            let mut new_hash = existing_hash;
            new_hash.insert(execution_id.to_string(), history_str);
            self.set_hash(&key, &new_hash)?;
        }
        
        Ok(())
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
                        match &self.sync_mode {
                            SyncMode::WriteThrough => {
                                redis.set_string(&key, &value)?;
                            },
                            SyncMode::WriteBack(_) => {
                                // Will be handled by the background thread
                            },
                            SyncMode::OnDemand => {
                                // Do nothing, we'll sync on demand
                            }
                        }
                        synced += 1;
                    },
                    Err(_) => {
                        // Try to get hash value
                        match self.memory.get_hash(&key) {
                            Ok(hash) => {
                                match &self.sync_mode {
                                    SyncMode::WriteThrough => {
                                        redis.set_hash(&key, &hash)?;
                                    },
                                    SyncMode::WriteBack(_) => {
                                        // Will be handled by the background thread
                                    },
                                    SyncMode::OnDemand => {
                                        // Do nothing, wait for explicit sync call
                                    }
                                }
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

    /// Get a Redis connection
    pub fn get_redis_connection(&self) -> Option<redis::Connection> {
        if let Some(redis) = &self.redis {
            match redis.get_connection() {
                Ok(conn) => Some(conn),
                Err(_) => None
            }
        } else {
            None
        }
    }

    /// List all rules in the store
    pub fn list_rules(&self) -> Result<Vec<serde_json::Value>> {
        let mut rules = Vec::new();
        
        // Get all rule IDs
        let keys = self.get_keys("rule:*")?;
        
        for key in keys {
            // Skip history records and other non-rule keys
            if key.contains(":history:") {
                continue;
            }
            
            // Extract rule ID from key
            let rule_id = key.strip_prefix("rule:").unwrap_or(&key);
            
            // Try to get the rule
            match self.get_rule(rule_id) {
                Ok(rule) => {
                    rules.push(rule);
                },
                Err(_) => {
                    // Skip rules that can't be retrieved
                    continue;
                }
            }
        }
        
        Ok(rules)
    }
    
    /// Create a new rule
    pub fn create_rule(&self, rule_json: &serde_json::Value) -> Result<()> {
        // Ensure rule contains ID
        let rule_id = rule_json.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ModelSrvError::InvalidInput("Rule must contain an id field".to_string()))?;
            
        // Check if rule already exists
        let rule_key = format!("rule:{}", rule_id);
        if self.exists(&rule_key)? {
            return Err(ModelSrvError::ModelAlreadyExists(rule_id.to_string()));
        }
        
        // Store rule
        self.set_string(&rule_key, &serde_json::to_string(rule_json)?)?;
        
        Ok(())
    }
    
    /// Update an existing rule
    pub fn update_rule(&self, rule_id: &str, rule_json: &serde_json::Value) -> Result<()> {
        // Check if rule exists
        let rule_key = format!("rule:{}", rule_id);
        if !self.exists(&rule_key)? {
            return Err(ModelSrvError::RuleNotFound(rule_id.to_string()));
        }
        
        // Store updated rule
        self.set_string(&rule_key, &serde_json::to_string(rule_json)?)?;
        
        Ok(())
    }
    
    /// Delete a rule
    pub fn delete_rule(&self, rule_id: &str) -> Result<()> {
        let rule_key = format!("rule:{}", rule_id);
        
        // Delete rule
        self.delete(&rule_key)?;
        
        // Try to delete related history
        let history_key = format!("rule:{}:history", rule_id);
        let _ = self.delete(&history_key); // Ignore errors
        
        Ok(())
    }
    
    /// Get execution history for a rule
    pub fn get_execution_history(&self, rule_id: &str) -> Result<Vec<serde_json::Value>> {
        let history_key = format!("rule:{}:history", rule_id);
        
        // Check if rule exists
        let rule_key = format!("rule:{}", rule_id);
        if !self.exists(&rule_key)? {
            return Err(ModelSrvError::RuleNotFound(rule_id.to_string()));
        }
        
        // Try to get history from memory store
        if let Ok(history_json) = self.get_string(&history_key) {
            // Single record
            if history_json.starts_with('{') {
                let record = serde_json::from_str(&history_json)?;
                return Ok(vec![record]);
            }
            
            // Multiple records, try to parse as array
            if history_json.starts_with('[') {
                let records = serde_json::from_str(&history_json)?;
                return Ok(records);
            }
        }
        
        // Try to get as hash (for history in memory store)
        if let Ok(hash) = self.get_hash(&history_key) {
            let mut records = Vec::new();
            for (_, value) in hash {
                if let Ok(record) = serde_json::from_str(&value) {
                    records.push(record);
                }
            }
            return Ok(records);
        }
        
        Ok(Vec::new())
    }

    /// Get a template manager instance
    pub fn get_template_manager(&self) -> Result<TemplateManager> {
        // get templates_dir and key_prefix from config
        let templates_dir = std::env::var("TEMPLATES_DIR").unwrap_or_else(|_| "templates".to_string());
        let key_prefix = "ems:";
        
        Ok(TemplateManager::new(&templates_dir, key_prefix))
    }
}

impl DataStore for HybridStore {
    fn get_string(&self, key: &str) -> Result<String> {
        self.memory.get_string(key)
    }
    
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        // Store in memory
        self.memory.set_string(key, value)?;
        
        // If in WriteThrough mode, also write to Redis
        if let (Some(redis), SyncMode::WriteThrough) = (&self.redis, self.sync_mode) {
            if let Err(e) = redis.set_string(key, value) {
                error!("Failed to write to Redis: {}", e);
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
                },
                SyncMode::WriteBack(_) => {
                    // Handled by background thread, not here
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
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
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
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
                },
                SyncMode::OnDemand => {
                    // Do nothing, wait for explicit sync call
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