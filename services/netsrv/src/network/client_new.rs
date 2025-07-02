use crate::config_new::{NetworkConfig, CloudMqttConfig, LegacyMqttConfig, HttpConfig, AwsIotFeatures};
use crate::formatter::DataFormatter;
use crate::error::Result;
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use serde_json::Value;

/// Modern network client trait that works directly with new configuration
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
    
    /// Get network name
    fn name(&self) -> &str;
    
    /// Get reference to self as Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Factory function to create network clients from new configuration
pub fn create_network_client(config: &NetworkConfig, formatter: Box<dyn DataFormatter>) -> Result<Box<dyn NetworkClient>> {
    match config {
        NetworkConfig::LegacyMqtt(mqtt_config) => {
            let client = LegacyMqttClient::new(mqtt_config.clone(), formatter)?;
            Ok(Box::new(client))
        }
        
        NetworkConfig::Http(http_config) => {
            let client = HttpClient::new(http_config.clone(), formatter)?;
            Ok(Box::new(client))
        }
        
        NetworkConfig::CloudMqtt(cloud_config) => {
            let client = CloudMqttClient::new(cloud_config.clone(), formatter)?;
            Ok(Box::new(client))
        }
    }
}

/// Legacy MQTT client implementation
pub struct LegacyMqttClient {
    config: LegacyMqttConfig,
    formatter: Box<dyn DataFormatter>,
    client: Option<rumqttc::AsyncClient>,
    connected: bool,
}

impl LegacyMqttClient {
    pub fn new(config: LegacyMqttConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        Ok(Self {
            config,
            formatter,
            client: None,
            connected: false,
        })
    }
}

#[async_trait]
impl NetworkClient for LegacyMqttClient {
    async fn connect(&mut self) -> Result<()> {
        // Parse broker URL for host and port
        let broker_url = url::Url::parse(&self.config.broker)
            .map_err(|e| crate::error::NetSrvError::Config(format!("Invalid broker URL: {}", e)))?;
            
        let host = broker_url.host_str().unwrap_or("localhost");
        let port = broker_url.port().unwrap_or(1883);
        
        let mut mqttoptions = rumqttc::MqttOptions::new(&self.config.client_id, host, port);
        
        if let Some(ref username) = self.config.auth {
            mqttoptions.set_credentials(&username.username, &username.password);
        }
        
        let (client, mut eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);
        
        // Start event loop in background
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT event loop error: {}", e);
                        break;
                    }
                }
            }
        });
        
        self.client = Some(client);
        self.connected = true;
        info!("Connected to legacy MQTT broker: {}:{}", host, port);
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        if let Some(ref client) = self.client {
            let _ = client.disconnect().await;
        }
        self.client = None;
        self.connected = false;
        info!("Disconnected from legacy MQTT broker");
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn send(&self, data: &str) -> Result<()> {
        if let Some(ref client) = self.client {
            let qos = match self.config.topics.qos {
                0 => rumqttc::QoS::AtMostOnce,
                1 => rumqttc::QoS::AtLeastOnce,
                2 => rumqttc::QoS::ExactlyOnce,
                _ => rumqttc::QoS::AtMostOnce,
            };
            
            client.publish(&self.config.topics.publish, qos, false, data).await
                .map_err(|e| crate::error::NetSrvError::Network(format!("MQTT publish failed: {}", e)))?;
                
            debug!("Published to topic {}: {}", self.config.topics.publish, data);
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// HTTP client implementation
pub struct HttpClient {
    config: HttpConfig,
    formatter: Box<dyn DataFormatter>,
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new(config: HttpConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| crate::error::NetSrvError::Network(format!("Failed to create HTTP client: {}", e)))?;
            
        Ok(Self {
            config,
            formatter,
            client,
        })
    }
}

#[async_trait]
impl NetworkClient for HttpClient {
    async fn connect(&mut self) -> Result<()> {
        // HTTP doesn't require explicit connection
        info!("HTTP client ready for: {}", self.config.url);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        // HTTP doesn't require explicit disconnection
        info!("HTTP client disconnected");
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        true // HTTP is always "connected"
    }
    
    async fn send(&self, data: &str) -> Result<()> {
        let mut request = match self.config.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.config.url),
            "POST" => self.client.post(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            "DELETE" => self.client.delete(&self.config.url),
            "PATCH" => self.client.patch(&self.config.url),
            _ => return Err(crate::error::NetSrvError::Config(format!("Unsupported HTTP method: {}", self.config.method))),
        };
        
        // Add headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }
        
        // Add authentication
        if let Some(ref auth) = self.config.auth {
            match auth {
                crate::config_new::HttpAuth::Basic { username, password } => {
                    request = request.basic_auth(username, Some(password));
                }
                crate::config_new::HttpAuth::Bearer { token } => {
                    request = request.bearer_auth(token);
                }
                crate::config_new::HttpAuth::Custom { headers } => {
                    for (key, value) in headers {
                        request = request.header(key, value);
                    }
                }
            }
        }
        
        // Send with body for POST/PUT/PATCH
        let response = if matches!(self.config.method.to_uppercase().as_str(), "POST" | "PUT" | "PATCH") {
            request.body(data.to_string()).send().await
        } else {
            request.send().await
        };
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!("HTTP request successful: {}", resp.status());
                } else {
                    warn!("HTTP request failed with status: {}", resp.status());
                }
            }
            Err(e) => {
                error!("HTTP request failed: {}", e);
                return Err(crate::error::NetSrvError::Network(format!("HTTP request failed: {}", e)));
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cloud MQTT client with AWS IoT optimization
pub struct CloudMqttClient {
    config: CloudMqttConfig,
    formatter: Box<dyn DataFormatter>,
    client: Option<rumqttc::AsyncClient>,
    connected: bool,
}

impl CloudMqttClient {
    pub fn new(config: CloudMqttConfig, formatter: Box<dyn DataFormatter>) -> Result<Self> {
        Ok(Self {
            config,
            formatter,
            client: None,
            connected: false,
        })
    }
    
    fn setup_aws_iot_options(&self, mqttoptions: &mut rumqttc::MqttOptions) -> Result<()> {
        // AWS IoT specific configuration
        if let crate::config_new::CloudProvider::Aws = self.config.provider {
            // Set ALPN protocols for AWS IoT
            if self.config.tls.alpn_protocols.contains(&"x-amzn-mqtt-ca".to_string()) {
                // AWS IoT requires specific ALPN protocol
                info!("Configuring AWS IoT ALPN protocols");
            }
            
            // Set up certificate authentication
            if let crate::config_new::AuthConfig::Certificate { cert_path, key_path, ca_path } = &self.config.auth {
                // Load certificates for AWS IoT
                info!("Loading AWS IoT certificates from {}", cert_path);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl NetworkClient for CloudMqttClient {
    async fn connect(&mut self) -> Result<()> {
        let endpoint = match &self.config.provider_config {
            crate::config_new::ProviderConfig::Aws { endpoint, port, .. } => (endpoint.clone(), *port),
            crate::config_new::ProviderConfig::Aliyun { endpoint, port, .. } => (endpoint.clone(), *port),
            crate::config_new::ProviderConfig::Azure { hostname, .. } => (hostname.clone(), 8883),
            crate::config_new::ProviderConfig::Custom { broker, port, .. } => (broker.clone(), *port),
            _ => return Err(crate::error::NetSrvError::Config("Unsupported provider configuration".to_string())),
        };
        
        let mut mqttoptions = rumqttc::MqttOptions::new(&self.config.name, &endpoint.0, endpoint.1);
        
        // Configure TLS
        if self.config.tls.enabled {
            mqttoptions.set_transport(rumqttc::Transport::tls_with_config(
                rumqttc::ClientConfig::default()
            ));
        }
        
        // AWS IoT specific setup
        self.setup_aws_iot_options(&mut mqttoptions)?;
        
        let (client, mut eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);
        
        // Start event loop
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Cloud MQTT event loop error: {}", e);
                        break;
                    }
                }
            }
        });
        
        // Subscribe to AWS IoT topics if enabled
        if let crate::config_new::CloudProvider::Aws = self.config.provider {
            if self.config.aws_features.jobs_enabled {
                let jobs_topic = format!("{}/#", self.config.aws_features.jobs_topic_prefix);
                info!("Subscribing to AWS IoT Jobs topic: {}", jobs_topic);
                // TODO: Implement subscription
            }
            
            if self.config.aws_features.device_shadow_enabled {
                let shadow_topic = format!("{}/#", self.config.aws_features.shadow_topic_prefix);
                info!("Subscribing to AWS IoT Shadow topic: {}", shadow_topic);
                // TODO: Implement subscription
            }
        }
        
        self.client = Some(client);
        self.connected = true;
        info!("Connected to cloud MQTT: {}:{}", endpoint.0, endpoint.1);
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        if let Some(ref client) = self.client {
            let _ = client.disconnect().await;
        }
        self.client = None;
        self.connected = false;
        info!("Disconnected from cloud MQTT");
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn send(&self, data: &str) -> Result<()> {
        if let Some(ref client) = self.client {
            let qos = match self.config.topics.qos {
                0 => rumqttc::QoS::AtMostOnce,
                1 => rumqttc::QoS::AtLeastOnce,
                2 => rumqttc::QoS::ExactlyOnce,
                _ => rumqttc::QoS::AtMostOnce,
            };
            
            // Replace variables in topic template
            let mut topic = self.config.topics.publish_template.clone();
            for (key, value) in &self.config.topics.variables {
                topic = topic.replace(&format!("{{{}}}", key), value);
            }
            
            client.publish(&topic, qos, false, data).await
                .map_err(|e| crate::error::NetSrvError::Network(format!("Cloud MQTT publish failed: {}", e)))?;
                
            debug!("Published to cloud topic {}: {}", topic, data);
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}