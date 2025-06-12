mod mqtt;
mod http;

use crate::config::network_config::{NetworkConfig, NetworkType};
use crate::config::cloud_config::CloudMqttConfig;
use crate::error::Result;
use crate::formatter::DataFormatter;
use async_trait::async_trait;
use std::any::Any;

pub use mqtt::MqttClient;
pub use http::HttpClient;

/// Network client trait for sending data
#[async_trait]
pub trait NetworkClient: Send + Sync {
    /// Connect to the network service
    async fn connect(&mut self) -> Result<()>;
    /// Send data to the network service
    async fn send(&self, data: &str) -> Result<()>;
    /// Check if connected
    fn is_connected(&self) -> bool;
    /// Disconnect from the network service
    async fn disconnect(&mut self) -> Result<()>;
    /// Get reference to Any for dynamic casting
    fn as_any(&self) -> &dyn Any;
}

/// Factory function to create network clients for legacy protocols
pub fn create_client(
    config: &NetworkConfig,
    formatter: Box<dyn DataFormatter>,
) -> Result<Box<dyn NetworkClient>> {
    match config.network_type {
        NetworkType::Mqtt => {
            if let Some(mqtt_config) = &config.mqtt_config {
                Ok(Box::new(MqttClient::new(mqtt_config.clone(), formatter)))
            } else {
                Err(crate::error::NetSrvError::ConfigError(
                    "MQTT configuration is missing".to_string(),
                ))
            }
        }
        NetworkType::Http => {
            if let Some(http_config) = &config.http_config {
                Ok(Box::new(HttpClient::new(http_config.clone(), formatter)))
            } else {
                Err(crate::error::NetSrvError::ConfigError(
                    "HTTP configuration is missing".to_string(),
                ))
            }
        }
    }
}

/// Factory function to create cloud clients
pub fn create_cloud_client(
    config: &CloudMqttConfig,
    formatter: Box<dyn DataFormatter>,
) -> Result<Box<dyn NetworkClient>> {
    match MqttClient::new_cloud(config.clone(), formatter) {
        Ok(client) => Ok(Box::new(client)),
        Err(e) => Err(e),
    }
} 