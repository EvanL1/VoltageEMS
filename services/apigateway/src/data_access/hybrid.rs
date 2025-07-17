use super::{
    cache::TieredCache, AccessOptions, AccessStrategy, CacheStats, DataAccessError,
    DataAccessLayer, DataAccessResult, DataType,
};
use async_trait::async_trait;
use log::{debug, error, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::timeout;

/// HTTP回源函数类型
type HttpFallbackFn = Box<
    dyn Fn(&str) -> futures::future::BoxFuture<'static, DataAccessResult<Value>>
        + Send
        + Sync,
>;

/// 混合数据访问实现
pub struct HybridDataAccess {
    /// 分层缓存
    cache: Arc<TieredCache>,
    /// Redis客户端
    redis_client: Arc<crate::redis_client::RedisClient>,
    /// HTTP客户端
    http_client: Arc<reqwest::Client>,
    /// InfluxDB客户端
    influxdb_client: Arc<crate::influxdb_client::InfluxDbClient>,
    /// 服务URL映射
    service_urls: HashMap<String, String>,
    /// HTTP回源函数映射
    http_fallbacks: HashMap<String, HttpFallbackFn>,
}

impl HybridDataAccess {
    pub fn new(
        redis_client: Arc<crate::redis_client::RedisClient>,
        http_client: Arc<reqwest::Client>,
        service_urls: HashMap<String, String>,
        influxdb_client: Arc<crate::influxdb_client::InfluxDbClient>,
    ) -> Self {
        let cache = Arc::new(TieredCache::new(
            1000,                       // 本地缓存1000项
            redis_client.clone(),
            "cache".to_string(),        // Redis缓存前缀
        ));

        let mut hybrid = Self {
            cache,
            redis_client,
            http_client,
            influxdb_client,
            service_urls,
            http_fallbacks: HashMap::new(),
        };

        // 注册默认的HTTP回源函数
        hybrid.register_default_fallbacks();
        hybrid
    }

    /// 注册HTTP回源函数
    pub fn register_fallback<F>(&mut self, prefix: &str, fallback: F)
    where
        F: Fn(&str) -> futures::future::BoxFuture<'static, DataAccessResult<Value>>
            + Send
            + Sync
            + 'static,
    {
        self.http_fallbacks.insert(prefix.to_string(), Box::new(fallback));
    }

    /// 注册默认的HTTP回源函数
    fn register_default_fallbacks(&mut self) {
        // TODO: Implement HTTP fallback registration
        // This requires a different approach to type-safe callback registration
        debug!("HTTP fallback functions will be implemented when needed");
    }

    /// 智能路由：根据数据类型和键选择访问策略
    fn get_access_strategy(&self, key: &str, options: &AccessOptions) -> AccessStrategy {
        // 如果明确指定了数据类型，使用对应策略
        if options.data_type != DataType::Config {
            return AccessStrategy::from_data_type(&options.data_type);
        }

        // 根据键前缀判断数据类型
        if key.starts_with("cfg:") {
            AccessStrategy::RedisWithHttpFallback
        } else if key.contains(":m:") || key.contains(":s:") || key.contains(":c:") || key.contains(":a:") {
            // 实时数据：仅Redis
            AccessStrategy::RedisOnly
        } else if key.starts_with("alarm:active:") || key.starts_with("alarm:stats:") {
            // 活动告警和统计：仅Redis
            AccessStrategy::RedisOnly
        } else if key.starts_with("alarm:config:") {
            // 告警配置：Redis缓存+HTTP回源
            AccessStrategy::RedisWithHttpFallback
        } else if key.starts_with("model:def:") || key.starts_with("model:instance:") {
            // 模型定义和实例：Redis缓存+HTTP回源
            AccessStrategy::RedisWithHttpFallback
        } else if key.starts_with("model:calc:") || key.starts_with("model:event:") {
            // 模型计算结果和事件：仅Redis
            AccessStrategy::RedisOnly
        } else if key.starts_with("rule:def:") {
            // 规则定义：Redis缓存+HTTP回源
            AccessStrategy::RedisWithHttpFallback
        } else if key.starts_with("rule:instance:") || key.starts_with("rule:trigger:") {
            // 规则实例和触发器：仅Redis
            AccessStrategy::RedisOnly
        } else if key.starts_with("his:") {
            // 历史数据：直接InfluxDB查询
            AccessStrategy::InfluxDbQuery
        } else if key.starts_with("stats:") || key.starts_with("report:") {
            // 统计和报表：直接HTTP
            AccessStrategy::HttpOnly
        } else if key.starts_with("meta:") {
            // 元数据：Redis缓存+HTTP回源
            AccessStrategy::RedisWithHttpFallback
        } else if key.starts_with("cache:") || key.starts_with("temp:") {
            // 缓存和临时数据：仅Redis
            AccessStrategy::RedisOnly
        } else {
            // 默认策略
            AccessStrategy::RedisWithHttpFallback
        }
    }

    /// Redis直接访问
    async fn redis_direct_get(&self, key: &str) -> DataAccessResult<Value> {
        match self.redis_client.get(key).await {
            Ok(Some(data)) => {
                serde_json::from_str(&data)
                    .map_err(|e| DataAccessError::Serialization(e.to_string()))
            }
            Ok(None) => Err(DataAccessError::NotFound(format!("Key not found: {}", key))),
            Err(e) => Err(DataAccessError::Redis(e.to_string())),
        }
    }

    /// Redis缓存访问（带HTTP回源）
    async fn redis_cached_get(&self, key: &str, options: &AccessOptions) -> DataAccessResult<Value> {
        // 1. 尝试从分层缓存获取
        if options.use_cache {
            if let Ok(Some(value)) = self.cache.get(key).await {
                debug!("Cache hit for key: {}", key);
                return Ok(value);
            }
            debug!("Cache miss for key: {}", key);
        }

        // 2. 如果启用HTTP回源，尝试从源站获取
        if options.fallback_http {
            if let Some(value) = self.http_fallback_get(key).await? {
                // 写入缓存
                if options.use_cache {
                    let cache_ttl = options.cache_ttl.unwrap_or(300);
                    if let Err(e) = self.cache.set(key, value.clone(), Some(cache_ttl)).await {
                        warn!("Failed to cache value for key {}: {}", key, e);
                    }
                }
                return Ok(value);
            }
        }

        Err(DataAccessError::NotFound(format!("Key not found: {}", key)))
    }

    /// HTTP回源获取
    async fn http_fallback_get(&self, key: &str) -> DataAccessResult<Option<Value>> {
        // 查找匹配的回源函数
        for (prefix, fallback) in &self.http_fallbacks {
            if key.starts_with(prefix) {
                debug!("Using HTTP fallback for key: {} with prefix: {}", key, prefix);
                return match fallback(key).await {
                    Ok(value) => Ok(Some(value)),
                    Err(DataAccessError::NotFound(_)) => Ok(None),
                    Err(e) => Err(e),
                };
            }
        }

        Ok(None)
    }

    /// HTTP直接查询
    async fn http_direct_query(&self, key: &str) -> DataAccessResult<Value> {
        // 解析服务名和路径
        let (service, path) = parse_service_key(key)?;
        
        let service_url = self.service_urls
            .get(&service)
            .ok_or_else(|| DataAccessError::NotFound(format!("Service not found: {}", service)))?;

        let full_url = format!("{}{}", service_url, path);
        
        debug!("HTTP direct query to: {}", full_url);

        match self.http_client.get(&full_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!(
                        "HTTP error: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    }

    /// InfluxDB查询
    async fn influxdb_query(&self, key: &str, options: &AccessOptions) -> DataAccessResult<Value> {
        use crate::influxdb_client::{parse_historical_key, HistoricalQuery};

        // 1. 先检查Redis缓存
        if options.use_cache {
            if let Ok(Some(cached_value)) = self.cache.get(key).await {
                debug!("InfluxDB query cache hit for key: {}", key);
                return Ok(cached_value);
            }
        }

        // 2. 解析历史数据键
        let historical_query = parse_historical_key(key)
            .ok_or_else(|| DataAccessError::NotFound(format!("Invalid historical key: {}", key)))?;

        debug!("InfluxDB query for key: {} -> {:?}", key, historical_query);

        // 3. 执行InfluxDB查询
        let result = match historical_query {
            HistoricalQuery::Index { channel_id, date } => {
                // 查询指定通道和日期的历史数据
                self.influxdb_client
                    .query_historical_data(channel_id, None, None, None, Some(1000))
                    .await
                    .map_err(|e| DataAccessError::Http(e.to_string()))?
            }
            HistoricalQuery::Statistics { channel_id, date } => {
                // 查询统计数据
                self.influxdb_client
                    .query_statistics(channel_id, &date)
                    .await
                    .map_err(|e| DataAccessError::Http(e.to_string()))?
            }
            HistoricalQuery::CachedQuery { query_id: _ } => {
                // 对于缓存查询，返回空结果（实际应该查询缓存）
                serde_json::json!({
                    "data": [],
                    "count": 0,
                    "message": "Cached query not implemented"
                })
            }
        };

        // 4. 写入Redis缓存
        if options.use_cache {
            let cache_ttl = options.cache_ttl.unwrap_or(60);
            if let Err(e) = self.cache.set(key, result.clone(), Some(cache_ttl)).await {
                warn!("Failed to cache InfluxDB result for key {}: {}", key, e);
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl DataAccessLayer for HybridDataAccess {
    async fn get_data(&self, key: &str, options: AccessOptions) -> DataAccessResult<Value> {
        let strategy = self.get_access_strategy(key, &options);
        
        debug!("Getting data for key: {} with strategy: {:?}", key, strategy);

        let operation = async {
            match strategy {
                AccessStrategy::RedisOnly => self.redis_direct_get(key).await,
                AccessStrategy::HttpOnly => self.http_direct_query(key).await,
                AccessStrategy::InfluxDbQuery => self.influxdb_query(key, &options).await,
                AccessStrategy::RedisWithHttpFallback => self.redis_cached_get(key, &options).await,
                AccessStrategy::HttpWithRedisCache => {
                    // HTTP优先，结果缓存到Redis
                    match self.http_direct_query(key).await {
                        Ok(value) => {
                            // 异步写入缓存
                            if options.use_cache {
                                let cache = self.cache.clone();
                                let key_clone = key.to_string();
                                let value_clone = value.clone();
                                let cache_ttl = options.cache_ttl.unwrap_or(300);
                                
                                tokio::spawn(async move {
                                    if let Err(e) = cache.set(&key_clone, value_clone, Some(cache_ttl)).await {
                                        warn!("Failed to cache HTTP result for key {}: {}", key_clone, e);
                                    }
                                });
                            }
                            Ok(value)
                        }
                        Err(e) => Err(e),
                    }
                }
            }
        };

        match timeout(options.timeout, operation).await {
            Ok(result) => result,
            Err(_) => Err(DataAccessError::Timeout(format!(
                "Operation timed out for key: {}",
                key
            ))),
        }
    }

    async fn set_data(&self, key: &str, value: Value, options: AccessOptions) -> DataAccessResult<()> {
        let strategy = self.get_access_strategy(key, &options);

        match strategy {
            AccessStrategy::RedisOnly | AccessStrategy::RedisWithHttpFallback => {
                let data = serde_json::to_string(&value)
                    .map_err(|e| DataAccessError::Serialization(e.to_string()))?;

                if let Some(ttl) = options.cache_ttl {
                    self.redis_client
                        .set_ex(key, &data, ttl)
                        .await
                        .map_err(|e| DataAccessError::Redis(e.to_string()))?;
                } else {
                    self.redis_client
                        .set(key, &data)
                        .await
                        .map_err(|e| DataAccessError::Redis(e.to_string()))?;
                }

                // 同时更新缓存
                if options.use_cache {
                    let _ = self.cache.set(key, value, options.cache_ttl).await;
                }

                Ok(())
            }
            AccessStrategy::InfluxDbQuery => {
                // InfluxDB写入暂时不实现，历史数据通常由hissrv负责写入
                Err(DataAccessError::Http("InfluxDB write not implemented - use hissrv".to_string()))
            }
            AccessStrategy::HttpOnly | AccessStrategy::HttpWithRedisCache => {
                // HTTP设置暂时不实现，因为大部分配置应该通过专门的配置API修改
                Err(DataAccessError::Http("HTTP set not implemented".to_string()))
            }
        }
    }

    async fn batch_get(&self, keys: Vec<String>, options: AccessOptions) -> DataAccessResult<Vec<Option<Value>>> {
        let mut results = Vec::with_capacity(keys.len());
        
        // 并发获取所有键
        let tasks: Vec<_> = keys.into_iter().map(|key| {
            let options = options.clone();
            async move {
                match self.get_data(&key, options).await {
                    Ok(value) => Some(value),
                    Err(_) => None,
                }
            }
        }).collect();

        let futures_results = futures::future::join_all(tasks).await;
        results.extend(futures_results);

        Ok(results)
    }

    async fn batch_set(&self, pairs: Vec<(String, Value)>, options: AccessOptions) -> DataAccessResult<()> {
        // 并发设置所有键值对
        let tasks: Vec<_> = pairs.into_iter().map(|(key, value)| {
            let options = options.clone();
            async move {
                self.set_data(&key, value, options).await
            }
        }).collect();

        let results = futures::future::join_all(tasks).await;
        
        // 检查是否有失败的操作
        for result in results {
            result?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> DataAccessResult<()> {
        // 从Redis删除
        if let Err(e) = self.redis_client.del(&[key]).await {
            error!("Failed to delete key {} from Redis: {}", key, e);
        }

        // 从缓存删除
        if let Err(e) = self.cache.remove(key).await {
            warn!("Failed to delete key {} from cache: {}", key, e);
        }

        Ok(())
    }

    async fn exists(&self, key: &str) -> DataAccessResult<bool> {
        // 首先检查缓存
        if let Ok(Some(_)) = self.cache.get(key).await {
            return Ok(true);
        }

        // 然后检查Redis
        self.redis_client
            .exists(key)
            .await
            .map_err(|e| DataAccessError::Redis(e.to_string()))
    }

    async fn clear_cache(&self, pattern: &str) -> DataAccessResult<u64> {
        self.cache
            .clear_pattern(pattern)
            .await
    }

    async fn cache_stats(&self) -> DataAccessResult<CacheStats> {
        Ok(self.cache.stats().await)
    }
}

/// 从通道键中提取通道ID
fn extract_channel_id(key: &str) -> Option<u32> {
    if let Some(id_str) = key.strip_prefix("cfg:channel:") {
        if let Some(id_part) = id_str.split(':').next() {
            id_part.parse().ok()
        } else {
            id_str.parse().ok()
        }
    } else {
        None
    }
}

/// 从模块键中提取模块名
fn extract_module_name(key: &str) -> Option<String> {
    key.strip_prefix("cfg:module:").map(|s| s.to_string())
}

/// 从模型键中提取模型名
fn extract_model_name(key: &str) -> Option<String> {
    key.strip_prefix("model:def:").map(|s| s.to_string())
}

/// 从告警规则键中提取规则ID
fn extract_alarm_rule_id(key: &str) -> Option<String> {
    key.strip_prefix("alarm:config:").map(|s| s.to_string())
}

/// 从控制规则键中提取规则ID
fn extract_rule_id(key: &str) -> Option<String> {
    key.strip_prefix("rule:def:").map(|s| s.to_string())
}

/// 解析服务键
fn parse_service_key(key: &str) -> DataAccessResult<(String, String)> {
    let parts: Vec<&str> = key.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(DataAccessError::NotFound(format!("Invalid service key format: {}", key)));
    }
    
    Ok((parts[0].to_string(), format!("/{}", parts[1])))
}

/// 获取通道配置的HTTP回源函数
async fn fetch_channel_config(
    client: Arc<reqwest::Client>,
    service_urls: HashMap<String, String>,
    channel_id: u32,
) -> DataAccessResult<Value> {
    if let Some(comsrv_url) = service_urls.get("comsrv") {
        let url = format!("{}/api/v1/channels/{}", comsrv_url, channel_id);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!("HTTP error: {}", response.status())))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    } else {
        Err(DataAccessError::NotFound("comsrv service URL not configured".to_string()))
    }
}

/// 获取模块配置的HTTP回源函数
async fn fetch_module_config(
    client: Arc<reqwest::Client>,
    service_urls: HashMap<String, String>,
    module_name: &str,
) -> DataAccessResult<Value> {
    if let Some(modsrv_url) = service_urls.get("modsrv") {
        let url = format!("{}/api/v1/modules/{}", modsrv_url, module_name);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!("HTTP error: {}", response.status())))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    } else {
        Err(DataAccessError::NotFound("modsrv service URL not configured".to_string()))
    }
}

/// 获取设备模型定义的HTTP回源函数
async fn fetch_model_definition(
    client: Arc<reqwest::Client>,
    service_urls: HashMap<String, String>,
    model_name: &str,
) -> DataAccessResult<Value> {
    if let Some(modsrv_url) = service_urls.get("modsrv") {
        let url = format!("{}/api/v1/models/{}", modsrv_url, model_name);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!("HTTP error: {}", response.status())))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    } else {
        Err(DataAccessError::NotFound("modsrv service URL not configured".to_string()))
    }
}

/// 获取告警规则配置的HTTP回源函数
async fn fetch_alarm_rule_config(
    client: Arc<reqwest::Client>,
    service_urls: HashMap<String, String>,
    rule_id: &str,
) -> DataAccessResult<Value> {
    if let Some(alarmsrv_url) = service_urls.get("alarmsrv") {
        let url = format!("{}/api/v1/rules/{}", alarmsrv_url, rule_id);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!("HTTP error: {}", response.status())))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    } else {
        Err(DataAccessError::NotFound("alarmsrv service URL not configured".to_string()))
    }
}

/// 获取控制规则定义的HTTP回源函数
async fn fetch_rule_definition(
    client: Arc<reqwest::Client>,
    service_urls: HashMap<String, String>,
    rule_id: &str,
) -> DataAccessResult<Value> {
    if let Some(rulesrv_url) = service_urls.get("rulesrv") {
        let url = format!("{}/api/v1/rules/{}", rulesrv_url, rule_id);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(value) => Ok(value),
                        Err(e) => Err(DataAccessError::Http(format!("JSON decode error: {}", e))),
                    }
                } else {
                    Err(DataAccessError::Http(format!("HTTP error: {}", response.status())))
                }
            }
            Err(e) => Err(DataAccessError::Http(e.to_string())),
        }
    } else {
        Err(DataAccessError::NotFound("rulesrv service URL not configured".to_string()))
    }
}