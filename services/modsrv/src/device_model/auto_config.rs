//! 设备实例自动配置模块
//!
//! 提供从模板文件自动创建和管理设备实例的功能

use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisHandler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info, warn};

#[allow(unused_imports)]
use super::{CalculationEngine, ModelRegistry};
use super::{DataFlowProcessor, DeviceModel, InstanceManager};

/// 自动配置管理器
pub struct AutoConfigManager {
    redis_client: Arc<RedisHandler>,
    instance_manager: Arc<InstanceManager>,
    data_flow_processor: Arc<DataFlowProcessor>,
    templates_dir: String,
    config_key_prefix: String,
}

/// 设备实例配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInstanceConfig {
    pub instance_id: String,
    pub instance_name: String,
    pub model_id: String,
    pub enabled: bool,
    pub point_mappings: HashMap<String, String>, // telemetry_name -> redis_key
    pub properties: HashMap<String, serde_json::Value>,
    pub channel_config: ChannelConfig,
}

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_id: u16,
    pub protocol: String,
    pub description: String,
    pub point_mappings: HashMap<String, PointMapping>,
}

/// 点位映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMapping {
    pub point_id: u32,
    pub point_type: String, // m, s, c, a
    pub data_type: String,  // float, int, bool, string
    pub description: String,
    pub unit: Option<String>,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
}

/// 自动配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoConfigFile {
    pub version: String,
    pub instances: Vec<DeviceInstanceConfig>,
    pub global_settings: GlobalSettings,
}

/// 全局设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub update_interval_ms: u64,
    pub enable_realtime_subscription: bool,
    pub enable_polling: bool,
    pub redis_key_prefix: String,
}

impl AutoConfigManager {
    /// 创建新的自动配置管理器
    pub fn new(
        redis_client: Arc<RedisHandler>,
        instance_manager: Arc<InstanceManager>,
        data_flow_processor: Arc<DataFlowProcessor>,
        templates_dir: String,
        config_key_prefix: String,
    ) -> Self {
        Self {
            redis_client,
            instance_manager,
            data_flow_processor,
            templates_dir,
            config_key_prefix,
        }
    }

    /// 从配置文件加载设备实例
    pub async fn load_from_config_file(&self, config_file_path: &str) -> Result<Vec<String>> {
        info!(
            "Loading device instances from config file: {}",
            config_file_path
        );

        // 读取配置文件
        let config_content = fs::read_to_string(config_file_path)
            .await
            .map_err(|e| ModelSrvError::IoError(format!("Failed to read config file: {}", e)))?;

        // 解析配置
        let config: AutoConfigFile = if config_file_path.ends_with(".yaml")
            || config_file_path.ends_with(".yml")
        {
            serde_yaml::from_str(&config_content)
                .map_err(|e| ModelSrvError::ConfigError(format!("Invalid YAML config: {}", e)))?
        } else {
            serde_json::from_str(&config_content)
                .map_err(|e| ModelSrvError::ConfigError(format!("Invalid JSON config: {}", e)))?
        };

        let mut created_instances = Vec::new();

        // 创建实例
        for instance_config in config.instances {
            if !instance_config.enabled {
                debug!(
                    "Skipping disabled instance: {}",
                    instance_config.instance_id
                );
                continue;
            }

            match self.create_instance_from_config(&instance_config).await {
                Ok(instance_id) => {
                    created_instances.push(instance_id);
                }
                Err(e) => {
                    error!(
                        "Failed to create instance {}: {}",
                        instance_config.instance_id, e
                    );
                    // 继续处理其他实例
                }
            }
        }

        info!(
            "Successfully created {} device instances",
            created_instances.len()
        );
        Ok(created_instances)
    }

    /// 从单个配置创建设备实例
    async fn create_instance_from_config(&self, config: &DeviceInstanceConfig) -> Result<String> {
        info!(
            "Creating device instance: {} ({})",
            config.instance_id, config.instance_name
        );

        // 加载设备模型
        let _model = self.load_device_model(&config.model_id).await?;

        // 创建设备实例
        let instance_id = self
            .instance_manager
            .create_instance(
                &config.model_id,
                &config.instance_id,
                &config.instance_name,
                Some(config.properties.clone()),
                None,
            )
            .await?;

        // 建立点位映射
        self.setup_point_mappings(&instance_id.instance_id, config)
            .await?;

        // 设置数据流订阅
        self.setup_data_flow_subscription(&instance_id.instance_id, config)
            .await?;

        // 持久化实例配置到Redis
        self.persist_instance_config(&instance_id.instance_id, config)
            .await?;

        info!(
            "Successfully created device instance: {}",
            instance_id.instance_id
        );
        Ok(instance_id.instance_id)
    }

    /// 加载设备模型
    async fn load_device_model(&self, model_id: &str) -> Result<DeviceModel> {
        // 尝试从Redis加载
        let redis_key = format!("{}:model:{}", self.config_key_prefix, model_id);
        if let Ok(Some(model_json)) = self.redis_client.get::<String>(&redis_key).await {
            if let Ok(model) = serde_json::from_str::<DeviceModel>(&model_json) {
                return Ok(model);
            }
        }

        // 从模板文件加载
        let template_path = format!("{}/{}.yaml", self.templates_dir, model_id);
        if Path::new(&template_path).exists() {
            let template_content = fs::read_to_string(&template_path)
                .await
                .map_err(|e| ModelSrvError::IoError(format!("Failed to read template: {}", e)))?;

            let model: DeviceModel = serde_yaml::from_str(&template_content).map_err(|e| {
                ModelSrvError::ConfigError(format!("Invalid template format: {}", e))
            })?;

            // 缓存到Redis
            let model_json = serde_json::to_string(&model)?;
            if let Err(e) = self.redis_client.set(&redis_key, model_json).await {
                warn!("Failed to cache model to Redis: {}", e);
            }

            return Ok(model);
        }

        Err(ModelSrvError::NotFound(format!(
            "Model not found: {}",
            model_id
        )))
    }

    /// 设置点位映射
    async fn setup_point_mappings(
        &self,
        instance_id: &str,
        config: &DeviceInstanceConfig,
    ) -> Result<()> {
        debug!("Setting up point mappings for instance: {}", instance_id);

        // 验证点位映射
        for (telemetry_name, redis_key) in &config.point_mappings {
            // 验证Redis键格式是否符合comsrv标准
            if !self.validate_redis_key_format(redis_key) {
                return Err(ModelSrvError::ConfigError(format!(
                    "Invalid Redis key format: {}. Expected format: channelID:type:pointID",
                    redis_key
                )));
            }

            debug!(
                "Mapped telemetry '{}' to Redis key '{}'",
                telemetry_name, redis_key
            );
        }

        // 将映射信息存储到实例元数据
        let mapping_key = format!(
            "{}:instance:{}:mappings",
            self.config_key_prefix, instance_id
        );
        let mapping_json = serde_json::to_string(&config.point_mappings)?;
        self.redis_client.set(&mapping_key, mapping_json).await?;

        Ok(())
    }

    /// 验证Redis键格式
    fn validate_redis_key_format(&self, redis_key: &str) -> bool {
        let parts: Vec<&str> = redis_key.split(':').collect();
        if parts.len() != 3 {
            return false;
        }

        // 验证通道ID是数字
        if parts[0].parse::<u16>().is_err() {
            return false;
        }

        // 验证点位类型
        if !matches!(parts[1], "m" | "s" | "c" | "a") {
            return false;
        }

        // 验证点位ID是数字
        if parts[2].parse::<u32>().is_err() {
            return false;
        }

        true
    }

    /// 设置数据流订阅
    async fn setup_data_flow_subscription(
        &self,
        instance_id: &str,
        config: &DeviceInstanceConfig,
    ) -> Result<()> {
        debug!(
            "Setting up data flow subscription for instance: {}",
            instance_id
        );

        let update_interval = tokio::time::Duration::from_millis(1000); // 默认1秒

        // 订阅数据更新
        self.data_flow_processor
            .subscribe_instance(
                instance_id.to_string(),
                config.point_mappings.clone(),
                update_interval,
            )
            .await?;

        Ok(())
    }

    /// 持久化实例配置到Redis
    async fn persist_instance_config(
        &self,
        instance_id: &str,
        config: &DeviceInstanceConfig,
    ) -> Result<()> {
        debug!("Persisting instance config for: {}", instance_id);

        let config_key = format!("{}:instance:{}:config", self.config_key_prefix, instance_id);
        let config_json = serde_json::to_string(config)?;
        self.redis_client.set(&config_key, config_json).await?;

        Ok(())
    }

    /// 从目录自动发现并加载配置文件
    pub async fn auto_discover_configs(&self, config_dir: &str) -> Result<Vec<String>> {
        info!("Auto-discovering configuration files in: {}", config_dir);

        let mut all_instances = Vec::new();
        let mut dir_entries = fs::read_dir(config_dir).await.map_err(|e| {
            ModelSrvError::IoError(format!("Failed to read config directory: {}", e))
        })?;

        while let Some(entry) = dir_entries
            .next_entry()
            .await
            .map_err(|e| ModelSrvError::IoError(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "yaml" || extension == "yml" || extension == "json" {
                        if let Some(path_str) = path.to_str() {
                            match self.load_from_config_file(path_str).await {
                                Ok(mut instances) => {
                                    all_instances.append(&mut instances);
                                }
                                Err(e) => {
                                    error!("Failed to load config file {}: {}", path_str, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Auto-discovery completed. Created {} instances total",
            all_instances.len()
        );
        Ok(all_instances)
    }

    /// 删除设备实例
    pub async fn remove_instance(&self, instance_id: &str) -> Result<()> {
        info!("Removing device instance: {}", instance_id);

        // 取消数据流订阅
        if let Err(e) = self
            .data_flow_processor
            .unsubscribe_instance(instance_id)
            .await
        {
            warn!(
                "Failed to unsubscribe data flow for instance {}: {}",
                instance_id, e
            );
        }

        // 删除Redis中的实例数据
        let keys_to_delete = vec![
            format!("{}:instance:{}:config", self.config_key_prefix, instance_id),
            format!(
                "{}:instance:{}:mappings",
                self.config_key_prefix, instance_id
            ),
        ];

        for key in keys_to_delete {
            if let Err(e) = self.redis_client.del(&key).await {
                warn!("Failed to delete Redis key {}: {}", key, e);
            }
        }

        // 从实例管理器删除
        // TODO: 实现 instance_manager.remove_instance 方法

        info!("Successfully removed device instance: {}", instance_id);
        Ok(())
    }

    /// 获取所有自动配置的实例
    pub async fn list_auto_configured_instances(&self) -> Result<Vec<String>> {
        let pattern = format!("{}:instance:*:config", self.config_key_prefix);
        let keys = self.redis_client.scan_keys(&pattern).await?;

        let mut instances = Vec::new();
        for key in keys {
            // 提取实例ID: prefix:instance:INSTANCE_ID:config
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 3 {
                instances.push(parts[2].to_string());
            }
        }

        Ok(instances)
    }
}

/// 配置验证器
pub struct ConfigValidator;

impl ConfigValidator {
    /// 验证设备实例配置
    pub fn validate_instance_config(config: &DeviceInstanceConfig) -> Result<()> {
        // 验证实例ID
        if config.instance_id.is_empty() {
            return Err(ModelSrvError::ConfigError(
                "Instance ID cannot be empty".to_string(),
            ));
        }

        // 验证模型ID
        if config.model_id.is_empty() {
            return Err(ModelSrvError::ConfigError(
                "Model ID cannot be empty".to_string(),
            ));
        }

        // 验证通道ID
        if config.channel_config.channel_id == 0 {
            return Err(ModelSrvError::ConfigError(
                "Channel ID cannot be zero".to_string(),
            ));
        }

        // 验证点位映射
        for (telemetry_name, redis_key) in &config.point_mappings {
            if telemetry_name.is_empty() {
                return Err(ModelSrvError::ConfigError(
                    "Telemetry name cannot be empty".to_string(),
                ));
            }

            if redis_key.is_empty() {
                return Err(ModelSrvError::ConfigError(
                    "Redis key cannot be empty".to_string(),
                ));
            }

            // 验证Redis键格式
            let parts: Vec<&str> = redis_key.split(':').collect();
            if parts.len() != 3 {
                return Err(ModelSrvError::ConfigError(format!(
                    "Invalid Redis key format: {}. Expected format: channelID:type:pointID",
                    redis_key
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_redis_key_format() {
        let manager = AutoConfigManager::new(
            Arc::new(RedisHandler::new()),
            Arc::new(InstanceManager::new(Arc::new(ModelRegistry::new()))),
            Arc::new(
                DataFlowProcessor::new(
                    Arc::new(RedisHandler::new()),
                    Arc::new(InstanceManager::new(Arc::new(ModelRegistry::new()))),
                    Arc::new(CalculationEngine::new()),
                )
                .0,
            ),
            "templates".to_string(),
            "modsrv".to_string(),
        );

        // 有效格式
        assert!(manager.validate_redis_key_format("1001:m:10001"));
        assert!(manager.validate_redis_key_format("1001:s:10002"));
        assert!(manager.validate_redis_key_format("1001:c:30001"));
        assert!(manager.validate_redis_key_format("1001:a:30002"));

        // 无效格式
        assert!(!manager.validate_redis_key_format("1001:m"));
        assert!(!manager.validate_redis_key_format("1001:m:10001:extra"));
        assert!(!manager.validate_redis_key_format("abc:m:10001"));
        assert!(!manager.validate_redis_key_format("1001:x:10001"));
        assert!(!manager.validate_redis_key_format("1001:m:abc"));
    }

    #[test]
    fn test_config_validation() {
        let config = DeviceInstanceConfig {
            instance_id: "test_instance".to_string(),
            instance_name: "Test Instance".to_string(),
            model_id: "test_model".to_string(),
            enabled: true,
            point_mappings: [
                ("voltage".to_string(), "1001:m:10001".to_string()),
                ("current".to_string(), "1001:m:10002".to_string()),
            ]
            .into_iter()
            .collect(),
            properties: HashMap::new(),
            channel_config: ChannelConfig {
                channel_id: 1001,
                protocol: "modbus_tcp".to_string(),
                description: "Test Channel".to_string(),
                point_mappings: HashMap::new(),
            },
        };

        assert!(ConfigValidator::validate_instance_config(&config).is_ok());
    }
}
