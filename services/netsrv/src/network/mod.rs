pub mod mqtt;
pub mod http;

use crate::config::network::NetworkConfig;
use crate::formatter::DataFormatter;
use crate::error::Result;
use async_trait::async_trait;
use std::any::Any;

pub use mqtt::MqttClient;
pub use http::HttpClient;

/// Trait for network clients
#[async_trait]
pub trait NetworkClient: Send + Sync {
    /// Connect to the network service
    async fn connect(&mut self) -> Result<()>;
    
    /// Disconnect from the network service
    async fn disconnect(&mut self) -> Result<()>;
    
    /// Check if connected to the network service
    fn is_connected(&self) -> bool;
    
    /// Send data to the network service
    async fn send(&self, data: &str) -> Result<()>;
    
    /// Get reference to self as Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Create a network client based on configuration
pub fn create_client(config: &NetworkConfig, formatter: Box<dyn DataFormatter>) -> Result<Box<dyn NetworkClient>> {
    match config {
        NetworkConfig::Mqtt { 
            broker_url, port, client_id, username, password, topic, qos, ..
        } => {
            let mqtt_config = mqtt::LegacyMqttConfig {
                broker_url: broker_url.clone(),
                port: *port,
                client_id: client_id.clone(),
                username: username.clone(),
                password: password.clone(),
                topic: topic.clone(),
                qos: *qos,
            };
            
            let client = MqttClient::new_legacy(mqtt_config, formatter)?;
            Ok(Box::new(client))
        }
        
        NetworkConfig::Http { 
            url, method, headers, auth_type, username, password, token, timeout_ms, ..
        } => {
            let http_config = http::HttpConfig {
                url: url.clone(),
                method: method.clone(),
                headers: headers.clone(),
                auth_type: auth_type.clone(),
                username: username.clone(),
                password: password.clone(),
                token: token.clone(),
                timeout_ms: *timeout_ms,
            };
            
            let client = HttpClient::new(http_config, formatter)?;
            Ok(Box::new(client))
        }
        
        NetworkConfig::Cloud { 
            cloud_provider, endpoint, port, client_id, auth_config, 
            topic_config, tls_config, keep_alive_secs, connection_timeout_ms, ..
        } => {
            let cloud_config = mqtt::CloudMqttConfig {
                cloud_provider: cloud_provider.clone(),
                endpoint: endpoint.clone(),
                port: *port,
                client_id: client_id.clone(),
                auth_config: auth_config.clone(),
                topic_config: topic_config.clone(),
                tls_config: tls_config.clone(),
                keep_alive_secs: *keep_alive_secs,
                connection_timeout_ms: *connection_timeout_ms,
            };
            
            let client = MqttClient::new_cloud(cloud_config, formatter)?;
            Ok(Box::new(client))
        }
    }
} 