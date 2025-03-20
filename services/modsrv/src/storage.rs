use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use serde_json::Value;
use crate::error::Result;
use std::collections::HashMap;

// 导出子模块
pub mod memory_store;
pub mod redis_store;
pub mod hybrid_store;

// 导出具体实现
pub use memory_store::MemoryStore;
pub use redis_store::RedisStore;
pub use hybrid_store::HybridStore;

/// Synchronization mode for storing model instances
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SyncMode {
    /// Automatically sync to persistent storage
    Auto,
    /// Manually sync to persistent storage
    Manual,
    /// Never sync to persistent storage
    None,
}

/// Data store trait
#[async_trait]
pub trait DataStore: Send + Sync {
    /// Set a string value
    fn set_string(&self, key: &str, value: &str) -> Result<()>;
    
    /// Get a string value
    fn get_string(&self, key: &str) -> Result<String>;
    
    /// Delete a key
    fn delete(&self, key: &str) -> Result<bool>;
    
    /// Check if a key exists
    fn exists(&self, key: &str) -> Result<bool>;
    
    /// Get multiple keys matching a pattern
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>>;
    
    /// Set a JSON value
    fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.set_string(key, &json)
    }
    
    /// Get a JSON value
    fn get_json<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<T> {
        let json = self.get_string(key)?;
        let value = serde_json::from_str(&json)?;
        Ok(value)
    }
    
    /// Push to a list
    fn push_list(&self, key: &str, value: &str) -> Result<()>;
    
    /// Get list range
    fn get_list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>>;
    
    /// Get list length
    fn get_list_len(&self, key: &str) -> Result<usize>;
    
    /// Set hash field
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()>;
    
    /// Get hash field
    fn get_hash_field(&self, key: &str, field: &str) -> Result<String>;
    
    /// Get all hash fields
    fn get_hash_all(&self, key: &str) -> Result<HashMap<String, String>>;
    
    /// Delete hash field
    fn delete_hash_field(&self, key: &str, field: &str) -> Result<bool>;
} 