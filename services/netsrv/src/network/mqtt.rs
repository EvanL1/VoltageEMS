use crate::config::network::{AuthConfig, TopicConfig, TlsConfig, CloudProvider};
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use log::{debug, error, info, warn};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use chrono::Utc;

type HmacSha256 = Hmac<Sha256>;

/// Legacy MQTT configuration
#[derive(Debug, Clone)]
pub struct LegacyMqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub topic: String,
    pub qos: u8,
    pub use_ssl: bool,
    pub ca_cert_path: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
}

/// Cloud MQTT configuration
#[derive(Debug, Clone)]
pub struct CloudMqttConfig {
    pub cloud_provider: CloudProvider,
    pub endpoint: String,
    pub port: u16,
    pub client_id: String,
    pub auth_config: AuthConfig,
    pub topic_config: TopicConfig,
    pub tls_config: TlsConfig,
    pub keep_alive_secs: u64,
    pub connection_timeout_ms: u64,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
    pub custom_properties: Option<HashMap<String, String>>,
}

impl CloudMqttConfig {
    /// Validate cloud configuration
    pub fn validate(&self) -> std::result::Result<(), String> {
        match self.cloud_provider {
            CloudProvider::Aws => {
                if !matches!(self.auth_config, AuthConfig::Certificate { .. }) {
                    return Err("AWS IoT requires certificate authentication".to_string());
                }
                if self.port != 8883 && self.port != 443 {
                    return Err("AWS IoT typically uses port 8883 or 443".to_string());
                }
            }
            CloudProvider::Aliyun => {
                if !matches!(self.auth_config, AuthConfig::DeviceSecret { .. }) {
                    return Err("Aliyun IoT requires device secret authentication".to_string());
                }
            }
            CloudProvider::Azure => {
                if !matches!(self.auth_config, AuthConfig::SasToken { .. }) {
                    return Err("Azure IoT typically uses SAS token authentication".to_string());
                }
            }
            CloudProvider::Tencent => {
                if !matches!(self.auth_config, AuthConfig::Certificate { .. }) {
                    return Err("Tencent IoT requires certificate authentication".to_string());
                }
            }
            CloudProvider::Huawei => {
                match self.auth_config {
                    AuthConfig::Certificate { .. } | AuthConfig::DeviceSecret { .. } => {}
                    _ => return Err("Huawei IoT requires certificate or device secret authentication".to_string()),
                }
            }
            CloudProvider::Custom => {} // Custom configs are flexible
        }
        Ok(())
    }
}

/// MQTT client configuration type
#[derive(Debug, Clone)]
pub enum MqttClientConfig {
    /// Legacy MQTT configuration
    Legacy(LegacyMqttConfig),
    /// Cloud MQTT configuration
    Cloud(CloudMqttConfig),
}

/// MQTT client that supports both legacy and cloud configurations
pub struct MqttClient {
    config: MqttClientConfig,
    client: Option<AsyncClient>,
    formatter: Box<dyn DataFormatter>,
    connected: Arc<Mutex<bool>>,
}

impl MqttClient {
    /// Create a new MQTT client with legacy configuration
    pub fn new_legacy(config: LegacyMqttConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        Ok(Self {
            config: MqttClientConfig::Legacy(config),
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        })
    }

    /// Create a new MQTT client with cloud configuration
    pub fn new_cloud(config: CloudMqttConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        // Validate configuration
        config.validate().map_err(NetSrvError::Config)?;
        
        Ok(Self {
            config: MqttClientConfig::Cloud(config),
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        })
    }

    /// Build MQTT options based on configuration type
    fn build_mqtt_options(&self) -> Result<MqttOptions> {
        match &self.config {
            MqttClientConfig::Legacy(config) => self.build_legacy_mqtt_options(config),
            MqttClientConfig::Cloud(config) => self.build_cloud_mqtt_options(config),
        }
    }

    /// Build MQTT options for legacy configuration
    fn build_legacy_mqtt_options(&self, config: &LegacyMqttConfig) -> Result<MqttOptions> {
        let mut mqtt_options = MqttOptions::new(
            &config.client_id,
            &config.broker_url,
            config.port,
        );

        // Set authentication information
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            mqtt_options.set_credentials(username, password);
        }

        // Set connection parameters
        mqtt_options.set_keep_alive(Duration::from_secs(30));
        mqtt_options.set_clean_session(true);

        Ok(mqtt_options)
    }

    /// Build MQTT options for cloud configuration
    fn build_cloud_mqtt_options(&self, config: &CloudMqttConfig) -> Result<MqttOptions> {
        let mut mqtt_options = MqttOptions::new(
            &config.client_id,
            &config.endpoint,
            config.port,
        );

        // Set basic options
        mqtt_options.set_keep_alive(Duration::from_secs(config.keep_alive_secs));

        // Configure authentication based on cloud provider
        match &config.auth_config {
            AuthConfig::DeviceSecret { product_key, device_name, device_secret } => {
                self.configure_device_secret_auth(&mut mqtt_options, config.cloud_provider.clone(), product_key, device_name, device_secret)?;
            }
            AuthConfig::UsernamePassword { username, password } => {
                mqtt_options.set_credentials(username, password);
            }
            AuthConfig::Custom { params } => {
                self.configure_custom_auth(&mut mqtt_options, params)?;
            }
            _ => {
                warn!("Certificate and SAS token authentication not fully implemented yet");
            }
        }

        Ok(mqtt_options)
    }

    /// Configure device secret authentication (Aliyun IoT, Huawei IoT)
    fn configure_device_secret_auth(
        &self,
        mqtt_options: &mut MqttOptions,
        cloud_provider: CloudProvider,
        product_key: &str,
        device_name: &str,
        device_secret: &str,
    ) -> Result<()> {
        match cloud_provider {
            CloudProvider::Aliyun => {
                let (username, password) = self.generate_aliyun_credentials(product_key, device_name, device_secret)?;
                mqtt_options.set_credentials(username, password);
            }
            CloudProvider::Huawei => {
                // Huawei IoT device secret format
                let username = format!("{}_{}", product_key, device_name);
                mqtt_options.set_credentials(username, device_secret);
            }
            _ => {
                return Err(NetSrvError::Config("Device secret auth not supported for this provider".to_string()));
            }
        }
        Ok(())
    }

    /// Generate Aliyun IoT MQTT credentials
    fn generate_aliyun_credentials(
        &self,
        product_key: &str,
        device_name: &str,
        device_secret: &str,
    ) -> Result<(String, String)> {
        let timestamp = Utc::now().timestamp_millis();
        
        // Generate username
        let username = format!("{}&{}", device_name, product_key);
        
        // Generate password using HMAC-SHA256
        let content = format!("clientId{}deviceName{}productKey{}timestamp{}", 
                             device_name, device_name, product_key, timestamp);
        
        let mut mac = HmacSha256::new_from_slice(device_secret.as_bytes())
            .map_err(|e| NetSrvError::Config(format!("HMAC key error: {}", e)))?;
        mac.update(content.as_bytes());
        let password = hex::encode(mac.finalize().into_bytes());
        
        Ok((username, password))
    }

    /// Configure custom authentication
    fn configure_custom_auth(&self, mqtt_options: &mut MqttOptions, params: &HashMap<String, String>) -> Result<()> {
        if let (Some(username), Some(password)) = (params.get("username"), params.get("password")) {
            mqtt_options.set_credentials(username, password);
        }
        Ok(())
    }

    /// Process topic template variables (for cloud configurations)
    fn process_topic_sync(&self, topic_template: &str) -> String {
        let mut topic = topic_template.to_string();
        
        // Replace common variables
        if let MqttClientConfig::Cloud(config) = &self.config {
            topic = topic.replace("{device_id}", &config.client_id);
        }
        topic = topic.replace("{timestamp}", &Utc::now().timestamp().to_string());
        
        // Replace topic-specific variables
        if let MqttClientConfig::Cloud(config) = &self.config {
            if let Some(topic_vars) = &config.topic_config.topic_variables {
                for (key, value) in topic_vars {
                    topic = topic.replace(&format!("{{{}}}", key), value);
                }
            }
        }
        
        topic
    }

    /// Get QoS level
    fn get_qos(&self) -> QoS {
        match &self.config {
            MqttClientConfig::Legacy(config) => {
                match config.qos {
                    0 => QoS::AtMostOnce,
                    1 => QoS::AtLeastOnce,
                    2 => QoS::ExactlyOnce,
                    _ => QoS::AtMostOnce,
                }
            }
            MqttClientConfig::Cloud(config) => {
                match config.topic_config.qos {
                    0 => QoS::AtMostOnce,
                    1 => QoS::AtLeastOnce,
                    2 => QoS::ExactlyOnce,
                    _ => QoS::AtMostOnce,
                }
            }
        }
    }

    /// Format data using the configured formatter
    pub fn format_data(&self, data: &Value) -> Result<String> {
        self.formatter.format(data)
    }
}

#[async_trait]
impl NetworkClient for MqttClient {
    async fn connect(&mut self) -> Result<()> {
        let config_name = match &self.config {
            MqttClientConfig::Legacy(config) => config.client_id.clone(),
            MqttClientConfig::Cloud(config) => config.client_id.clone(),
        };
        
        info!("Connecting to MQTT: {}", config_name);
        
        let mqtt_options = self.build_mqtt_options()?;
        let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);
        
        // Start background event loop
        let connected = Arc::clone(&self.connected);
        let config_name_for_task = config_name.clone();
        
        tokio::spawn(async move {
            loop {
                match event_loop.poll().await {
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                        info!("MQTT connected successfully: {}", config_name_for_task);
                        *connected.lock().await = true;
                    }
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Disconnect)) => {
                        warn!("MQTT disconnected: {}", config_name_for_task);
                        *connected.lock().await = false;
                    }
                    Ok(_) => {
                        // Other events, continue
                    }
                    Err(e) => {
                        error!("MQTT event loop error for {}: {}", config_name_for_task, e);
                        *connected.lock().await = false;
                        sleep(Duration::from_millis(1000)).await;
                    }
                }
            }
        });
        
        self.client = Some(client);
        
        // Wait for connection
        let timeout_duration = match &self.config {
            MqttClientConfig::Legacy(_) => Duration::from_secs(10),
            MqttClientConfig::Cloud(config) => Duration::from_millis(config.connection_timeout_ms),
        };
        
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < timeout_duration {
            if *self.connected.lock().await {
                info!("Successfully connected to MQTT: {}", config_name);
                
                // Subscribe to topics if configured (cloud only)
                if let MqttClientConfig::Cloud(config) = &self.config {
                    if let Some(subscribe_topics) = &config.topic_config.subscribe_topics {
                        for topic_template in subscribe_topics {
                            let topic = self.process_topic_sync(topic_template);
                            if let Some(client) = &self.client {
                                client.subscribe(topic.clone(), self.get_qos()).await
                                    .map_err(|e| NetSrvError::Mqtt(e.to_string()))?;
                                info!("Subscribed to topic: {}", topic);
                            }
                        }
                    }
                }
                
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        Err(NetSrvError::Connection("Connection timeout".to_string()))
    }

    async fn disconnect(&mut self) -> Result<()> {
        let config_name = match &self.config {
            MqttClientConfig::Legacy(config) => &config.client_id,
            MqttClientConfig::Cloud(config) => &config.client_id,
        };
        
        info!("Disconnecting from MQTT: {}", config_name);
        
        if let Some(client) = &self.client {
            client.disconnect().await
                .map_err(|e| NetSrvError::Mqtt(e.to_string()))?;
        }
        
        *self.connected.lock().await = false;
        self.client = None;
        
        info!("Disconnected from MQTT: {}", config_name);
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::Connection("Not connected".to_string()));
        }
        
        let client = self.client.as_ref()
            .ok_or_else(|| NetSrvError::Connection("Client not initialized".to_string()))?;
        
        match &self.config {
            MqttClientConfig::Legacy(config) => {
                client.publish(&config.topic, self.get_qos(), false, data)
                    .await
                    .map_err(|e| NetSrvError::Mqtt(e.to_string()))?;
                debug!("Published data to legacy topic: {}", config.topic);
            }
            MqttClientConfig::Cloud(config) => {
                let topic = self.process_topic_sync(&config.topic_config.publish_topic);
                client.publish(topic.clone(), self.get_qos(), config.topic_config.retain, data)
                    .await
                    .map_err(|e| NetSrvError::Mqtt(e.to_string()))?;
                debug!("Published data to cloud topic: {}", topic);
            }
        }
        
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Use try_lock to avoid blocking
        if let Ok(connected) = self.connected.try_lock() {
            *connected
        } else {
            false
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::network::{CloudProvider, AuthConfig, TopicConfig, TlsConfig};
    use crate::formatter::JsonFormatter;

    fn create_legacy_config() -> LegacyMqttConfig {
        LegacyMqttConfig {
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "test-legacy".to_string(),
            username: None,
            password: None,
            topic: "test/data".to_string(),
            qos: 0,
            use_ssl: false,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }
    }

    fn create_cloud_config() -> CloudMqttConfig {
        CloudMqttConfig {
            cloud_provider: CloudProvider::Custom,
            endpoint: "localhost".to_string(),
            port: 1883,
            client_id: "test-client".to_string(),
            auth_config: AuthConfig::UsernamePassword {
                username: "test".to_string(),
                password: "test".to_string(),
            },
            topic_config: TopicConfig {
                publish_topic: "test/data".to_string(),
                topic_variables: None,
                subscribe_topics: Some(vec!["test/commands".to_string()]),
                qos: 1,
                retain: false,
            },
            tls_config: TlsConfig {
                enabled: false,
                verify_cert: false,
                verify_hostname: false,
                ca_path: None,
                alpn_protocols: None,
            },
            keep_alive_secs: 30,
            connection_timeout_ms: 5000,
            reconnect_delay_ms: 1000,
            max_reconnect_attempts: 3,
            custom_properties: None,
        }
    }

    #[test]
    fn test_new_legacy_mqtt_client() {
        let config = create_legacy_config();
        let formatter = Box::new(JsonFormatter::new());
        let client = MqttClient::new_legacy(config, formatter).unwrap();
        assert!(matches!(client.config, MqttClientConfig::Legacy(_)));
    }

    #[test]
    fn test_new_cloud_mqtt_client() {
        let config = create_cloud_config();
        let formatter = Box::new(JsonFormatter::new());
        let client = MqttClient::new_cloud(config, formatter);
        assert!(client.is_ok());
        assert!(matches!(client.unwrap().config, MqttClientConfig::Cloud(_)));
    }

    #[test]
    fn test_generate_aliyun_credentials() {
        let config = create_cloud_config();
        let formatter = Box::new(JsonFormatter::new());
        let client = MqttClient::new_cloud(config, formatter).unwrap();
        
        let result = client.generate_aliyun_credentials(
            "test_product",
            "test_device",
            "test_secret"
        );
        assert!(result.is_ok());
        
        let (username, password) = result.unwrap();
        assert_eq!(username, "test_device&test_product");
        assert!(!password.is_empty());
    }
} 