//! gRPC 插件管理器
//!
//! 负责插件的生命周期管理、健康检查和负载均衡

use crate::utils::error::{ComSrvError, Result};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::client::GrpcPluginClient;

/// 插件实例信息
#[derive(Clone, Debug)]
struct PluginInstance {
    /// 插件端点
    endpoint: String,
    /// 插件客户端
    client: Arc<RwLock<GrpcPluginClient>>,
    /// 健康状态
    healthy: Arc<RwLock<bool>>,
    /// 最后健康检查时间
    last_check: Arc<RwLock<std::time::Instant>>,
}

/// 插件管理器
#[derive(Debug)]
pub struct PluginManager {
    /// 协议类型到插件实例的映射
    plugins: Arc<DashMap<String, Vec<PluginInstance>>>,
    /// 健康检查间隔
    health_check_interval: Duration,
}

impl PluginManager {
    /// 创建新的插件管理器
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(DashMap::new()),
            health_check_interval: Duration::from_secs(30),
        }
    }

    /// 注册插件
    pub async fn register_plugin(&self, protocol_type: &str, endpoint: &str) -> Result<()> {
        info!(
            "Registering plugin for protocol {} at {}",
            protocol_type, endpoint
        );

        // 创建客户端
        let client = GrpcPluginClient::new(endpoint).await?;

        // 获取插件信息验证
        let mut client_guard = client.clone();
        let info = client_guard.get_info().await?;

        if info.protocol_type != protocol_type {
            return Err(ComSrvError::config(format!(
                "Protocol type mismatch: expected {}, got {}",
                protocol_type, info.protocol_type
            )));
        }

        let instance = PluginInstance {
            endpoint: endpoint.to_string(),
            client: Arc::new(RwLock::new(client)),
            healthy: Arc::new(RwLock::new(true)),
            last_check: Arc::new(RwLock::new(std::time::Instant::now())),
        };

        // 添加到插件列表
        self.plugins
            .entry(protocol_type.to_string())
            .or_default()
            .push(instance);

        info!(
            "Successfully registered plugin {} v{} for protocol {}",
            info.name, info.version, protocol_type
        );

        Ok(())
    }

    /// 注销插件
    pub async fn unregister_plugin(&self, protocol_type: &str, endpoint: &str) {
        info!(
            "Unregistering plugin for protocol {} at {}",
            protocol_type, endpoint
        );

        if let Some(mut instances) = self.plugins.get_mut(protocol_type) {
            instances.retain(|instance| instance.endpoint != endpoint);
        }
    }

    /// 获取健康的插件客户端
    pub async fn get_client(&self, protocol_type: &str) -> Result<Arc<RwLock<GrpcPluginClient>>> {
        let instances = self.plugins.get(protocol_type).ok_or_else(|| {
            ComSrvError::config(format!(
                "No plugins registered for protocol {protocol_type}"
            ))
        })?;

        // 查找健康的实例
        for instance in instances.iter() {
            if *instance.healthy.read().await {
                return Ok(instance.client.clone());
            }
        }

        Err(ComSrvError::protocol(format!(
            "No healthy plugins available for protocol {protocol_type}"
        )))
    }

    /// 启动健康检查任务
    pub fn start_health_check(&self) {
        let plugins = self.plugins.clone();
        let interval_duration = self.health_check_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                // 检查所有插件
                for entry in plugins.iter() {
                    let _protocol_type = entry.key();
                    let instances = entry.value();

                    for instance in instances {
                        Self::check_instance_health(instance).await;
                    }
                }
            }
        });
    }

    /// 检查单个实例健康状态
    async fn check_instance_health(instance: &PluginInstance) {
        debug!("Checking health of plugin at {}", instance.endpoint);

        let mut client = instance.client.write().await;
        match client.health_check().await {
            Ok(status) => {
                let was_healthy = *instance.healthy.read().await;
                *instance.healthy.write().await = status.healthy;
                *instance.last_check.write().await = std::time::Instant::now();

                if status.healthy && !was_healthy {
                    info!("Plugin at {} is now healthy", instance.endpoint);
                } else if !status.healthy && was_healthy {
                    warn!(
                        "Plugin at {} is now unhealthy: {}",
                        instance.endpoint, status.message
                    );
                }
            }
            Err(e) => {
                error!(
                    "Health check failed for plugin at {}: {}",
                    instance.endpoint, e
                );
                *instance.healthy.write().await = false;
                *instance.last_check.write().await = std::time::Instant::now();
            }
        }
    }

    /// 获取所有注册的插件信息
    pub async fn list_plugins(&self) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();

        for entry in self.plugins.iter() {
            let protocol_type = entry.key().clone();
            let endpoints: Vec<String> = entry
                .value()
                .iter()
                .map(|instance| instance.endpoint.clone())
                .collect();

            result.push((protocol_type, endpoints));
        }

        result
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
