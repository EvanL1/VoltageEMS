pub mod cache;
pub mod hybrid;
pub mod sync;

use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

/// 数据类型分类
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    /// 实时数据：遥测、信号、告警 - 仅Redis访问
    Realtime,
    /// 配置数据：通道配置、模块配置 - Redis缓存+HTTP回源
    Config,
    /// 历史数据：时序数据 - InfluxDB查询
    Historical,
    /// 复杂查询：报表、统计分析 - 直接HTTP
    Complex,
}

/// 数据访问选项
#[derive(Debug, Clone)]
pub struct AccessOptions {
    /// 是否使用Redis缓存
    pub use_cache: bool,
    /// 缓存过期时间（秒）
    pub cache_ttl: Option<u64>,
    /// Redis失败时是否回退到HTTP
    pub fallback_http: bool,
    /// 操作超时时间
    pub timeout: Duration,
    /// 数据类型（用于智能路由）
    pub data_type: DataType,
}

impl AccessOptions {
    /// 实时数据访问选项：仅Redis
    pub fn realtime() -> Self {
        Self {
            use_cache: false,
            cache_ttl: None,
            fallback_http: false,
            timeout: Duration::from_secs(5),
            data_type: DataType::Realtime,
        }
    }

    /// 配置数据访问选项：缓存优先+HTTP回源
    pub fn config_cached(cache_ttl: u64) -> Self {
        Self {
            use_cache: true,
            cache_ttl: Some(cache_ttl),
            fallback_http: true,
            timeout: Duration::from_secs(10),
            data_type: DataType::Config,
        }
    }

    /// 历史数据访问选项：InfluxDB查询
    pub fn historical() -> Self {
        Self {
            use_cache: true,
            cache_ttl: Some(60), // 1分钟缓存
            fallback_http: false,
            timeout: Duration::from_secs(30),
            data_type: DataType::Historical,
        }
    }

    /// 复杂查询选项：直接HTTP
    pub fn complex_query() -> Self {
        Self {
            use_cache: false,
            cache_ttl: None,
            fallback_http: false,
            timeout: Duration::from_secs(30),
            data_type: DataType::Complex,
        }
    }

    /// 通用缓存优先选项
    pub fn cache_first() -> Self {
        Self {
            use_cache: true,
            cache_ttl: Some(300), // 5分钟
            fallback_http: false,
            timeout: Duration::from_secs(10),
            data_type: DataType::Config,
        }
    }

    /// 缓存+降级选项
    pub fn cache_with_fallback() -> Self {
        Self {
            use_cache: true,
            cache_ttl: Some(300),
            fallback_http: true,
            timeout: Duration::from_secs(15),
            data_type: DataType::Config,
        }
    }
}

/// 数据访问错误
#[derive(Error, Debug)]
pub enum DataAccessError {
    #[error("Redis error: {0}")]
    Redis(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Cache miss: {0}")]
    CacheMiss(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

pub type DataAccessResult<T> = Result<T, DataAccessError>;

/// 统一数据访问层接口
#[async_trait]
pub trait DataAccessLayer: Send + Sync {
    /// 获取单个数据
    async fn get_data(&self, key: &str, options: AccessOptions) -> DataAccessResult<Value>;

    /// 设置单个数据
    async fn set_data(&self, key: &str, value: Value, options: AccessOptions) -> DataAccessResult<()>;

    /// 批量获取数据
    async fn batch_get(&self, keys: Vec<String>, options: AccessOptions) -> DataAccessResult<Vec<Option<Value>>>;

    /// 批量设置数据
    async fn batch_set(&self, pairs: Vec<(String, Value)>, options: AccessOptions) -> DataAccessResult<()>;

    /// 删除数据
    async fn delete(&self, key: &str) -> DataAccessResult<()>;

    /// 检查键是否存在
    async fn exists(&self, key: &str) -> DataAccessResult<bool>;

    /// 清理缓存
    async fn clear_cache(&self, pattern: &str) -> DataAccessResult<u64>;

    /// 获取缓存统计
    async fn cache_stats(&self) -> DataAccessResult<CacheStats>;
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub total_keys: usize,
    pub memory_usage: usize,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
            total_keys: 0,
            memory_usage: 0,
        }
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.update_hit_rate();
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
        self.update_hit_rate();
    }

    fn update_hit_rate(&mut self) {
        let total = self.hits + self.misses;
        if total > 0 {
            self.hit_rate = self.hits as f64 / total as f64;
        }
    }
}

/// 数据访问策略
#[derive(Debug, Clone)]
pub enum AccessStrategy {
    /// 仅Redis
    RedisOnly,
    /// 仅HTTP
    HttpOnly,
    /// InfluxDB查询
    InfluxDbQuery,
    /// Redis优先，失败时HTTP
    RedisWithHttpFallback,
    /// HTTP优先，写入Redis缓存
    HttpWithRedisCache,
}

impl AccessStrategy {
    pub fn from_data_type(data_type: &DataType) -> Self {
        match data_type {
            DataType::Realtime => Self::RedisOnly,
            DataType::Config => Self::RedisWithHttpFallback,
            DataType::Historical => Self::InfluxDbQuery,
            DataType::Complex => Self::HttpOnly,
        }
    }
}