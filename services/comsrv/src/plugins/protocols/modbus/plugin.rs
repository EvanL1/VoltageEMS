//! Modbus Protocol Plugin Implementation
//!
//! 精简的 Modbus 插件实现，适配现有的插件接口

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::core::combase::ComBase;
use crate::core::config::types::ChannelConfig;
use crate::plugins::traits::{ConfigTemplate, ProtocolMetadata, ProtocolPlugin, ValidationRule};
use crate::utils::error::{ComSrvError as Error, Result};

use super::core::ModbusProtocol;
use super::transport::create_transport;
use super::types::ModbusPollingConfig;

/// Modbus TCP Protocol Plugin
#[derive(Debug, Default)]
pub struct ModbusTcpPlugin;

/// Modbus RTU Protocol Plugin  
#[derive(Debug, Default)]
pub struct ModbusRtuPlugin;

#[async_trait]
impl ProtocolPlugin for ModbusTcpPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        ProtocolMetadata {
            id: "modbus_tcp".to_string(),
            name: "Modbus TCP".to_string(),
            version: "2.0.0".to_string(),
            description: "精简的 Modbus TCP 协议实现，包含轮询和批量优化".to_string(),
            author: "VoltageEMS Team".to_string(),
            license: "MIT".to_string(),
            features: vec!["polling".to_string(), "batch_read".to_string()],
            dependencies: HashMap::new(),
        }
    }

    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![ConfigTemplate {
            name: "polling".to_string(),
            description: "Modbus 轮询和批量处理配置".to_string(),
            param_type: "object".to_string(),
            required: false,
            default_value: Some(json!({
                "enabled": true,
                "default_interval_ms": 1000,
                "connection_timeout_ms": 5000,
                "read_timeout_ms": 3000,
                "max_retries": 3,
                "retry_interval_ms": 1000,
                "batch_config": {
                    "enabled": true,
                    "max_batch_size": 100,
                    "max_gap": 5,
                    "merge_function_codes": false
                }
            })),
            validation: Some(ValidationRule {
                min: Some(100.0),
                max: Some(60000.0),
                pattern: None,
                allowed_values: None,
            }),
        }]
    }

    fn validate_config(&self, _config: &HashMap<String, Value>) -> Result<()> {
        // 基本验证已在 config_template 中定义
        Ok(())
    }

    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        info!("创建 Modbus TCP 实例，通道 {}", channel_config.id);

        // 提取轮询配置
        let polling_config = extract_polling_config(&channel_config.parameters);

        // 创建传输层
        let transport = create_transport(&channel_config).await?;

        // 创建 Modbus 协议实例
        let protocol = ModbusProtocol::new(
            channel_config,
            Arc::new(tokio::sync::Mutex::new(transport)),
            polling_config,
        )?;

        Ok(Box::new(protocol))
    }
}

#[async_trait]
impl ProtocolPlugin for ModbusRtuPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        ProtocolMetadata {
            id: "modbus_rtu".to_string(),
            name: "Modbus RTU".to_string(),
            version: "2.0.0".to_string(),
            description: "精简的 Modbus RTU 协议实现，包含轮询和批量优化".to_string(),
            author: "VoltageEMS Team".to_string(),
            license: "MIT".to_string(),
            features: vec!["polling".to_string(), "batch_read".to_string()],
            dependencies: HashMap::new(),
        }
    }

    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![ConfigTemplate {
            name: "polling".to_string(),
            description: "Modbus 轮询和批量处理配置".to_string(),
            param_type: "object".to_string(),
            required: false,
            default_value: Some(json!({
                "enabled": true,
                "default_interval_ms": 1000,
                "connection_timeout_ms": 5000,
                "read_timeout_ms": 3000,
                "max_retries": 3,
                "retry_interval_ms": 1000,
                "batch_config": {
                    "enabled": true,
                    "max_batch_size": 100,
                    "max_gap": 5,
                    "merge_function_codes": false
                }
            })),
            validation: Some(ValidationRule {
                min: Some(100.0),
                max: Some(60000.0),
                pattern: None,
                allowed_values: None,
            }),
        }]
    }

    fn validate_config(&self, _config: &HashMap<String, Value>) -> Result<()> {
        Ok(())
    }

    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        info!("创建 Modbus RTU 实例，通道 {}", channel_config.id);

        // 提取轮询配置
        let polling_config = extract_polling_config(&channel_config.parameters);

        // 创建传输层
        let transport = create_transport(&channel_config).await?;

        // 创建 Modbus 协议实例
        let protocol = ModbusProtocol::new(
            channel_config,
            Arc::new(tokio::sync::Mutex::new(transport)),
            polling_config,
        )?;

        Ok(Box::new(protocol))
    }
}

// 辅助函数：提取轮询配置
fn extract_polling_config(parameters: &HashMap<String, serde_yaml::Value>) -> ModbusPollingConfig {
    if let Some(polling_value) = parameters.get("polling") {
        if let Ok(config) = serde_yaml::from_value::<ModbusPollingConfig>(polling_value.clone()) {
            return config;
        }
    }
    ModbusPollingConfig::default()
}
