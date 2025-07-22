use super::{DataAccessError, DataAccessResult};
use crate::redis_client::RedisClient;
use chrono::Utc;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Duration序列化辅助模块
mod duration_serde {
    use serde::{Deserializer, Serializer, Deserialize};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

/// 配置变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub event_type: ConfigChangeType,
    pub service: String,
    pub config_key: String,
    pub timestamp: i64,
    pub user: Option<String>,
    pub changes: Option<Value>,
}

/// 配置变更类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeType {
    Created,
    Updated,
    Deleted,
    Refreshed,
}

/// 服务配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub service_name: String,
    pub url: String,
    pub config_endpoints: Vec<String>,
    #[serde(with = "duration_serde")]
    pub sync_interval: Duration,
    #[serde(skip)]
    pub last_sync: Option<std::time::Instant>,
    pub sync_enabled: bool,
}

impl ServiceConfig {
    pub fn new(service_name: String, url: String) -> Self {
        Self {
            service_name,
            url,
            config_endpoints: vec![
                "/api/v1/config".to_string(),
                "/api/v1/status".to_string(),
            ],
            sync_interval: Duration::from_secs(300), // 5分钟
            last_sync: None,
            sync_enabled: true,
        }
    }

    pub fn add_endpoint(&mut self, endpoint: String) {
        if !self.config_endpoints.contains(&endpoint) {
            self.config_endpoints.push(endpoint);
        }
    }

    pub fn should_sync(&self) -> bool {
        if !self.sync_enabled {
            return false;
        }

        match self.last_sync {
            Some(last) => last.elapsed() >= self.sync_interval,
            None => true,
        }
    }
}

/// 配置同步服务
pub struct ConfigSyncService {
    redis_client: Arc<RedisClient>,
    http_client: Arc<reqwest::Client>,
    services: Arc<RwLock<HashMap<String, ServiceConfig>>>,
    sync_stats: Arc<RwLock<SyncStats>>,
}

/// 同步统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub total_syncs: u64,
    pub successful_syncs: u64,
    pub failed_syncs: u64,
    #[serde(skip)]
    pub last_sync_time: Option<std::time::Instant>,
    #[serde(with = "duration_serde")]
    pub avg_sync_duration: Duration,
}

impl Default for SyncStats {
    fn default() -> Self {
        Self {
            total_syncs: 0,
            successful_syncs: 0,
            failed_syncs: 0,
            last_sync_time: None,
            avg_sync_duration: Duration::from_secs(0),
        }
    }
}

impl ConfigSyncService {
    pub fn new(redis_client: Arc<RedisClient>, http_client: Arc<reqwest::Client>) -> Self {
        Self {
            redis_client,
            http_client,
            services: Arc::new(RwLock::new(HashMap::new())),
            sync_stats: Arc::new(RwLock::new(SyncStats::default())),
        }
    }

    /// 注册服务
    pub async fn register_service(&self, service_config: ServiceConfig) {
        let mut services = self.services.write().await;
        services.insert(service_config.service_name.clone(), service_config);
    }

    /// 启动配置同步任务
    pub async fn start_sync_tasks(&self) {
        info!("Starting configuration sync tasks");

        // 启动定期同步任务
        self.start_periodic_sync().await;

        // 启动配置变更监听
        self.start_change_listener().await;

        // 启动服务健康检查
        self.start_health_check().await;
    }

    /// 启动定期同步
    async fn start_periodic_sync(&self) {
        let redis_client = self.redis_client.clone();
        let http_client = self.http_client.clone();
        let services = self.services.clone();
        let stats = self.sync_stats.clone();

        tokio::spawn(async move {
            let mut sync_timer = interval(Duration::from_secs(60)); // 每分钟检查一次

            loop {
                sync_timer.tick().await;

                let services_to_sync = {
                    let services_guard = services.read().await;
                    services_guard
                        .values()
                        .filter(|s| s.should_sync())
                        .cloned()
                        .collect::<Vec<_>>()
                };

                if !services_to_sync.is_empty() {
                    debug!("Syncing {} services", services_to_sync.len());
                    
                    for mut service_config in services_to_sync {
                        let sync_start = std::time::Instant::now();
                        
                        match sync_service_config(
                            &redis_client,
                            &http_client,
                            &service_config,
                        ).await {
                            Ok(_) => {
                                info!("Successfully synced config for service: {}", service_config.service_name);
                                
                                // 更新同步时间
                                service_config.last_sync = Some(std::time::Instant::now());
                                let mut services_guard = services.write().await;
                                services_guard.insert(service_config.service_name.clone(), service_config);

                                // 更新统计
                                let mut stats_guard = stats.write().await;
                                stats_guard.successful_syncs += 1;
                                stats_guard.total_syncs += 1;
                                stats_guard.last_sync_time = Some(std::time::Instant::now());
                                
                                let sync_duration = sync_start.elapsed();
                                stats_guard.avg_sync_duration = 
                                    (stats_guard.avg_sync_duration + sync_duration) / 2;
                            }
                            Err(e) => {
                                error!("Failed to sync config for service {}: {}", service_config.service_name, e);
                                
                                // 更新统计
                                let mut stats_guard = stats.write().await;
                                stats_guard.failed_syncs += 1;
                                stats_guard.total_syncs += 1;
                            }
                        }
                    }
                }
            }
        });
    }

    /// 启动配置变更监听
    async fn start_change_listener(&self) {
        let redis_client = self.redis_client.clone();

        tokio::spawn(async move {
            loop {
                match redis_client.subscribe(&["config:changed"]).await {
                    Ok(_pubsub) => {
                        info!("Started listening for configuration changes");
                        
                        // TODO: Implement proper Redis PubSub message handling
                        // This requires using the correct Redis API for message consumption
                        warn!("Redis PubSub configuration change listener not yet implemented");
                    }
                    Err(e) => {
                        error!("Failed to subscribe to config changes: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    /// 启动服务健康检查
    async fn start_health_check(&self) {
        let http_client = self.http_client.clone();
        let services = self.services.clone();

        tokio::spawn(async move {
            let mut health_timer = interval(Duration::from_secs(30)); // 每30秒检查一次

            loop {
                health_timer.tick().await;

                let services_list = {
                    let services_guard = services.read().await;
                    services_guard.values().cloned().collect::<Vec<_>>()
                };

                for service in services_list {
                    if service.sync_enabled {
                        check_service_health(&http_client, &service).await;
                    }
                }
            }
        });
    }

    /// 手动触发同步
    pub async fn trigger_sync(&self, service_name: &str) -> DataAccessResult<()> {
        let service_config = {
            let services = self.services.read().await;
            services.get(service_name).cloned()
        };

        if let Some(service) = service_config {
            sync_service_config(&self.redis_client, &self.http_client, &service).await?;
            
            // 发布配置刷新事件
            let event = ConfigChangeEvent {
                event_type: ConfigChangeType::Refreshed,
                service: service_name.to_string(),
                config_key: "all".to_string(),
                timestamp: Utc::now().timestamp(),
                user: Some("system".to_string()),
                changes: None,
            };

            self.publish_config_change(event).await?;
            Ok(())
        } else {
            Err(DataAccessError::NotFound(format!("Service not found: {}", service_name)))
        }
    }

    /// 发布配置变更事件
    pub async fn publish_config_change(&self, event: ConfigChangeEvent) -> DataAccessResult<()> {
        let event_json = serde_json::to_string(&event)
            .map_err(|e| DataAccessError::Serialization(e.to_string()))?;

        self.redis_client
            .publish("config:changed", &event_json)
            .await
            .map_err(|e| DataAccessError::Redis(e.to_string()))?;

        Ok(())
    }

    /// 获取同步统计
    pub async fn get_sync_stats(&self) -> SyncStats {
        self.sync_stats.read().await.clone()
    }

    /// 获取服务列表
    pub async fn get_services(&self) -> Vec<ServiceConfig> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }

    /// 禁用/启用服务同步
    pub async fn set_service_sync_enabled(&self, service_name: &str, enabled: bool) -> DataAccessResult<()> {
        let mut services = self.services.write().await;
        
        if let Some(service) = services.get_mut(service_name) {
            service.sync_enabled = enabled;
            info!("{} sync for service: {}", if enabled { "Enabled" } else { "Disabled" }, service_name);
            Ok(())
        } else {
            Err(DataAccessError::NotFound(format!("Service not found: {}", service_name)))
        }
    }
}

/// 同步单个服务的配置
async fn sync_service_config(
    redis_client: &RedisClient,
    http_client: &reqwest::Client,
    service_config: &ServiceConfig,
) -> DataAccessResult<()> {
    for endpoint in &service_config.config_endpoints {
        let url = format!("{}{}", service_config.url, endpoint);
        
        debug!("Fetching config from: {}", url);

        match http_client.get(&url).timeout(Duration::from_secs(10)).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Value>().await {
                        Ok(config) => {
                            // 将配置存储到Redis
                            let key = format!("cfg:service:{}:{}", service_config.service_name, endpoint_to_key(endpoint));
                            let config_json = serde_json::to_string(&config)
                                .map_err(|e| DataAccessError::Serialization(e.to_string()))?;

                            redis_client
                                .set_ex(&key, &config_json, 3600) // 1小时过期
                                .await
                                .map_err(|e| DataAccessError::Redis(e.to_string()))?;

                            debug!("Stored config for {}: {}", service_config.service_name, key);
                        }
                        Err(e) => {
                            warn!("Failed to parse JSON from {}: {}", url, e);
                        }
                    }
                } else {
                    warn!("HTTP error {} from {}", response.status(), url);
                }
            }
            Err(e) => {
                warn!("Failed to fetch config from {}: {}", url, e);
            }
        }
    }

    Ok(())
}

/// 处理配置变更事件
async fn handle_config_change(redis_client: &RedisClient, event: ConfigChangeEvent) {
    debug!("Handling config change for service: {}", event.service);

    // 清理相关缓存
    let cache_pattern = format!("cache:cfg:{}:*", event.service);
    if let Ok(keys) = redis_client.keys(&cache_pattern).await {
        for key in keys {
            if let Err(e) = redis_client.del(&[key.as_str()]).await {
                warn!("Failed to clear cache key {}: {}", key, e);
            }
        }
    }

    info!("Processed config change event for service: {}", event.service);
}

/// 检查服务健康状态
async fn check_service_health(http_client: &reqwest::Client, service: &ServiceConfig) {
    let health_url = format!("{}/health", service.url);
    
    match http_client.get(&health_url).timeout(Duration::from_secs(5)).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                warn!("Service {} health check failed: {}", service.service_name, response.status());
            }
        }
        Err(e) => {
            warn!("Service {} health check error: {}", service.service_name, e);
        }
    }
}

/// 将端点路径转换为Redis键
fn endpoint_to_key(endpoint: &str) -> String {
    endpoint
        .trim_start_matches("/api/v1/")
        .replace("/", "_")
}