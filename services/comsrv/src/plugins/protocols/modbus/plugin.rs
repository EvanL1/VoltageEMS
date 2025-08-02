//! Modbus Protocol Plugin Implementation
//!
//! Streamlined Modbus plugin implementation, adapted for existing plugin interface

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::info;

use crate::core::combase::ComBase;
use crate::core::config::types::ChannelConfig;
use crate::plugins::traits::{ConfigTemplate, ProtocolMetadata, ProtocolPlugin, ValidationRule};
use crate::utils::error::Result;

use super::core::ModbusProtocol;
use super::transport::create_connection_params;
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
            description: "Streamlined Modbus TCP protocol with polling and batch optimization"
                .to_string(),
            author: "VoltageEMS Team".to_string(),
            license: "MIT".to_string(),
            features: vec!["polling".to_string(), "batch_read".to_string()],
            dependencies: HashMap::new(),
        }
    }

    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![ConfigTemplate {
            name: "polling".to_string(),
            description: "Modbus polling and batch processing configuration".to_string(),
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
                    "max_gap": 5
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
        // Basic validation is already defined in config_template
        Ok(())
    }

    async fn create_instance(&self, channel_config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        info!(
            "Creating Modbus TCP instance for channel {}",
            channel_config.id
        );

        // Extract polling configuration
        let polling_config = extract_polling_config(&channel_config.parameters);

        // Create connection parameters
        let connection_params = create_connection_params(&channel_config)?;

        // Create Modbus protocol instance
        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)?;

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
            description: "Streamlined Modbus RTU protocol with polling and batch optimization"
                .to_string(),
            author: "VoltageEMS Team".to_string(),
            license: "MIT".to_string(),
            features: vec!["polling".to_string(), "batch_read".to_string()],
            dependencies: HashMap::new(),
        }
    }

    fn config_template(&self) -> Vec<ConfigTemplate> {
        vec![ConfigTemplate {
            name: "polling".to_string(),
            description: "Modbus polling and batch processing configuration".to_string(),
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
                    "max_gap": 5
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
        info!(
            "Creating Modbus RTU instance for channel {}",
            channel_config.id
        );

        // Extract polling configuration
        let polling_config = extract_polling_config(&channel_config.parameters);

        // Create connection parameters
        let connection_params = create_connection_params(&channel_config)?;

        // Create Modbus protocol instance
        let protocol = ModbusProtocol::new(channel_config, connection_params, polling_config)?;

        Ok(Box::new(protocol))
    }
}

// Helper function: extract polling configuration
fn extract_polling_config(parameters: &HashMap<String, serde_yaml::Value>) -> ModbusPollingConfig {
    if let Some(polling_value) = parameters.get("polling") {
        if let Ok(mut config) = serde_yaml::from_value::<ModbusPollingConfig>(polling_value.clone())
        {
            // Check for batch_size in the polling config
            if let Some(polling_map) = polling_value.as_mapping() {
                if let Some(batch_size_value) =
                    polling_map.get(serde_yaml::Value::String("batch_size".to_string()))
                {
                    if let Some(batch_size) = batch_size_value.as_u64() {
                        // Set max_batch_size from batch_size, clamping to valid range
                        config.batch_config.max_batch_size = (batch_size as u16).clamp(1, 128);
                    }
                }
            }
            return config;
        }
    }
    ModbusPollingConfig::default()
}
