use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// Export sub-modules
pub mod redis_store;

/// Synchronization mode for storing model instances
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncMode {
    /// Immediately sync to persistent storage
    WriteThrough,
    /// Sync to persistent storage at regular intervals
    WriteBack(Duration),
    /// Manually sync to persistent storage on demand
    OnDemand,
}

/// Define the regular expression pattern for key matching
pub fn pattern_to_regex(pattern: &str) -> regex::Regex {
    let pattern = pattern
        .replace(".", "\\.")
        .replace("*", ".*")
        .replace("?", ".");

    regex::Regex::new(&format!("^{}$", pattern))
        .unwrap_or_else(|_| regex::Regex::new(".*").unwrap())
}

/// DataStore trait for storage implementations
pub trait DataStore {
    /// Get a string value
    fn get_string(&self, key: &str) -> Result<String>;

    /// Set a string value
    fn set_string(&self, key: &str, value: &str) -> Result<()>;

    /// Get a hash value
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>>;

    /// Set a hash value
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()>;

    /// Set a field in a hash
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()>;

    /// Get keys matching a pattern
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>>;

    /// Check if a key exists
    fn exists(&self, key: &str) -> Result<bool>;

    /// Delete a key
    fn delete(&self, key: &str) -> Result<()>;
}
