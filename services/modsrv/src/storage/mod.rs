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

/// 数据存储接口，定义了存储操作的基本方法
pub trait DataStore: Send + Sync {
    /// 获取字符串值
    fn get_string(&self, key: &str) -> Result<String>;
    
    /// 设置字符串值
    fn set_string(&self, key: &str, value: &str) -> Result<()>;
    
    /// 获取哈希表
    fn get_hash(&self, key: &str) -> Result<HashMap<String, String>>;
    
    /// 设置哈希表
    fn set_hash(&self, key: &str, hash: &HashMap<String, String>) -> Result<()>;
    
    /// 设置哈希表中的单个字段
    fn set_hash_field(&self, key: &str, field: &str, value: &str) -> Result<()>;
    
    /// 获取键的类型
    fn get_type(&self, key: &str) -> Result<crate::redis_handler::RedisType> {
        // 默认实现：先尝试获取字符串，如果成功则是字符串类型
        if self.get_string(key).is_ok() {
            return Ok(crate::redis_handler::RedisType::String);
        }
        
        // 尝试获取哈希表，如果成功则是哈希类型
        if self.get_hash(key).is_ok() {
            return Ok(crate::redis_handler::RedisType::Hash);
        }
        
        // 如果键存在但类型未知，返回None类型
        if self.exists(key)? {
            return Ok(crate::redis_handler::RedisType::None);
        }
        
        // 键不存在
        Err(ModelSrvError::KeyNotFound(key.to_string()))
    }
    
    /// 获取匹配模式的所有键
    fn get_keys(&self, pattern: &str) -> Result<Vec<String>>;
    
    /// 检查键是否存在
    fn exists(&self, key: &str) -> Result<bool>;
    
    /// 删除键
    fn delete(&self, key: &str) -> Result<()>;
}

/// 同步模式枚举
#[derive(Clone, Debug)]
pub enum SyncMode {
    /// 写操作立即同步到Redis
    WriteThrough,
    
    /// 定期批量同步到Redis
    WriteBack(Duration),
    
    /// 只在需要时同步到Redis
    OnDemand,
}

/// 将通配符模式转换为正则表达式
pub fn pattern_to_regex(pattern: &str) -> Regex {
    let pattern = pattern.replace("*", ".*");
    Regex::new(&format!("^{}$", pattern)).unwrap_or_else(|_| Regex::new(".*").unwrap())
} 