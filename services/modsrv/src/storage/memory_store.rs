use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::error::{Result, ModelSrvError};
use super::{DataStore, pattern_to_regex};

/// Memory store implementation
pub struct MemoryStore {
    data: Arc<RwLock<HashMap<String, String>>>,
    hash_data: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl MemoryStore {
    /// Create a new memory store
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            hash_data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Clear all data
    pub fn clear(&self) -> Result<()> {
        let mut data = self.data.write().map_err(|_| ModelSrvError::LockError)?;
        data.clear();
        
        let mut hash_data = self.hash_data.write().map_err(|_| ModelSrvError::LockError)?;
        hash_data.clear();
        
        Ok(())
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore for MemoryStore {
    fn get_string(&self, key: &str) -> Result<String> {
        let data = self.data.read().map_err(|_| ModelSrvError::LockError)?;
        data.get(key)
            .cloned()
            .ok_or_else(|| ModelSrvError::KeyNotFound(key.to_string()))
    }
    
    fn set_string(&self, key: &str, value: &str) -> Result<()> {
        let mut data = self.data.write().map_err(|_| ModelSrvError::LockError)?;
        data.insert(key.to_string(), value.to_string());
        Ok(())
    }
    
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>> {
        let hash_data = self.hash_data.read().map_err(|_| ModelSrvError::LockError)?;
        hash_data.get(key)
            .cloned()
            .ok_or_else(|| ModelSrvError::KeyNotFound(key.to_string()))
    }
    
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()> {
        let mut hash_data = self.hash_data.write().map_err(|_| ModelSrvError::LockError)?;
        hash_data.insert(key.to_string(), hash.clone());
        Ok(())
    }
    
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut hash_data = self.hash_data.write().map_err(|_| ModelSrvError::LockError)?;
        
        let hash = hash_data.entry(key.to_string()).or_insert_with(HashMap::new);
        hash.insert(field.to_string(), value.to_string());
        
        Ok(())
    }
    
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let data = self.data.read().map_err(|_| ModelSrvError::LockError)?;
        let hash_data = self.hash_data.read().map_err(|_| ModelSrvError::LockError)?;
        let regex = pattern_to_regex(pattern);
        
        let mut keys: Vec<String> = data.keys()
            .filter(|k| regex.is_match(k))
            .cloned()
            .collect();
            
        let hash_keys: Vec<String> = hash_data.keys()
            .filter(|k| regex.is_match(k))
            .cloned()
            .collect();
            
        keys.extend(hash_keys);
        keys.sort();
        keys.dedup();
        
        Ok(keys)
    }
    
    fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().map_err(|_| ModelSrvError::LockError)?;
        let hash_data = self.hash_data.read().map_err(|_| ModelSrvError::LockError)?;
        
        Ok(data.contains_key(key) || hash_data.contains_key(key))
    }
    
    fn delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().map_err(|_| ModelSrvError::LockError)?;
        data.remove(key);
        
        let mut hash_data = self.hash_data.write().map_err(|_| ModelSrvError::LockError)?;
        hash_data.remove(key);
        
        Ok(())
    }
} 