use crate::error::{Result, ModelSrvError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use regex::Regex;
use log::{error, info, debug};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;

pub mod redis_store;
pub mod memory_store;
pub mod hybrid_store;

/// Data store interface defining basic storage operations
pub trait DataStore: Send + Sync {
    /// Get a string value
    fn get_string(&self, key: &str) -> Result<String>;
    
    /// Set a string value
    fn set_string(&self, key: &str, value: &str) -> Result<()>;
    
    /// Get a hash table
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>>;
    
    /// Set a hash table
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()>;
    
    /// Set a single field in a hash table
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()>;
    
    /// Get the type of a key
    fn get_type(&self, key: &str) -> Result<crate::redis_handler::RedisType> {
        // Default implementation: first try to get a string, if successful it's a string type
        if self.get_string(key).is_ok() {
            return Ok(crate::redis_handler::RedisType::String);
        }
        
        // Try to get a hash table, if successful it's a hash type
        if self.get_hash(key).is_ok() {
            return Ok(crate::redis_handler::RedisType::Hash);
        }
        
        // If the key exists but type is unknown, return None type
        if self.exists(key)? {
            return Ok(crate::redis_handler::RedisType::None);
        }
        
        // Key doesn't exist
        Err(ModelSrvError::KeyNotFound(key.to_string()))
    }
    
    /// Get all keys matching a pattern
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>>;
    
    /// Check if a key exists
    fn exists(&self, key: &str) -> Result<bool>;
    
    /// Delete a key
    fn delete(&self, key: &str) -> Result<()>;
}

/// Sync mode enumeration
#[derive(Clone, Debug)]
pub enum SyncMode {
    /// Write operations are immediately synced to Redis
    WriteThrough,
    
    /// Batch sync to Redis periodically
    WriteBack(Duration),
    
    /// Only sync to Redis when explicitly requested
    OnDemand,
}

/// Convert a wildcard pattern to a regular expression
pub fn pattern_to_regex(pattern: &str) -> Regex {
    let pattern = pattern.replace("*", ".*");
    Regex::new(&format!("^{}$", pattern)).unwrap_or_else(|_| Regex::new(".*").unwrap())
}