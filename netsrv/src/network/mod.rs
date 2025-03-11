mod mqtt;
mod http;
mod aws_iot;
mod aliyun_iot;

use crate::config::network_config::{NetworkConfig, NetworkType};
use crate::error::Result;
use crate::formatter::DataFormatter;
use async_trait::async_trait;
use serde_json::Value;

pub use mqtt::MqttClient;
pub use http::HttpClient;
pub use aws_iot::AwsIotClient;
pub use aliyun_iot::AliyunIotClient;

#[async_trait]
pub trait NetworkClient: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send(&self, data: &str) -> Result<()>;
    fn is_connected(&self) -> bool;
}

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
                    "MQTT configuration is missing".into(),
                ))
            }
        }
        NetworkType::Http => {
            if let Some(http_config) = &config.http_config {
                Ok(Box::new(HttpClient::new(http_config.clone(), formatter)))
            } else {
                Err(crate::error::NetSrvError::ConfigError(
                    "HTTP configuration is missing".into(),
                ))
            }
        }
        NetworkType::AwsIot => {
            if let Some(aws_config) = &config.aws_iot_config {
                Ok(Box::new(AwsIotClient::new(aws_config.clone(), formatter)))
            } else {
                Err(crate::error::NetSrvError::ConfigError(
                    "AWS IoT configuration is missing".into(),
                ))
            }
        }
        NetworkType::AliyunIot => {
            if let Some(aliyun_config) = &config.aliyun_iot_config {
                Ok(Box::new(AliyunIotClient::new(aliyun_config.clone(), formatter)))
            } else {
                Err(crate::error::NetSrvError::ConfigError(
                    "Aliyun IoT configuration is missing".into(),
                ))
            }
        }
    }
} 