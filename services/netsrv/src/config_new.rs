use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use voltage_config::prelude::*;

/// Network service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// Network configurations
    #[serde(default)]
    pub networks: Vec<NetworkConfig>,
    
    /// Data processing configuration
    pub data: DataConfig,
}

/// Data processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Redis data key pattern
    #[serde(default = "default_data_key_pattern")]
    pub redis_data_key: String,
    
    /// Redis polling interval in seconds
    #[serde(default = "default_polling_interval")]
    pub redis_polling_interval_secs: u64,
    
    /// Enable data buffering
    #[serde(default = "default_true")]
    pub enable_buffering: bool,
    
    /// Buffer size
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkConfig {
    /// Legacy MQTT configuration
    LegacyMqtt(LegacyMqttConfig),
    
    /// HTTP REST API configuration
    Http(HttpConfig),
    
    /// Cloud MQTT configuration
    CloudMqtt(CloudMqttConfig),
}

/// Legacy MQTT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyMqttConfig {
    /// Network name
    pub name: String,
    
    /// Broker address
    pub broker: String,
    
    /// Client ID
    pub client_id: String,
    
    /// Authentication
    #[serde(default)]
    pub auth: Option<BasicAuth>,
    
    /// Topics configuration
    pub topics: TopicConfig,
    
    /// Data format
    #[serde(default)]
    pub format_type: FormatType,
    
    /// Connection settings
    #[serde(default)]
    pub connection: ConnectionSettings,
}

/// HTTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Network name
    pub name: String,
    
    /// Base URL
    pub url: String,
    
    /// HTTP method
    #[serde(default = "default_http_method")]
    pub method: String,
    
    /// Authentication
    #[serde(default)]
    pub auth: Option<HttpAuth>,
    
    /// Headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    
    /// Timeout in seconds
    #[serde(default = "default_http_timeout")]
    pub timeout_secs: u64,
    
    /// Data format
    #[serde(default)]
    pub format_type: FormatType,
}

/// Cloud MQTT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMqttConfig {
    /// Network name
    pub name: String,
    
    /// Cloud provider
    pub provider: CloudProvider,
    
    /// Provider-specific configuration
    pub provider_config: ProviderConfig,
    
    /// Authentication configuration
    pub auth: AuthConfig,
    
    /// Topics configuration
    pub topics: CloudTopicConfig,
    
    /// Data format
    #[serde(default)]
    pub format_type: FormatType,
    
    /// TLS configuration
    #[serde(default)]
    pub tls: TlsConfig,
    
    /// AWS-specific features
    #[serde(default)]
    pub aws_features: AwsIotFeatures,
}

/// Cloud provider enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    Aws,
    Aliyun,
    Azure,
    Tencent,
    Huawei,
    Custom,
}

/// Provider-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderConfig {
    /// AWS IoT configuration
    Aws {
        endpoint: String,
        port: u16,
        thing_name: String,
    },
    
    /// Aliyun IoT configuration
    Aliyun {
        endpoint: String,
        port: u16,
        product_key: String,
        device_name: String,
    },
    
    /// Azure IoT Hub configuration
    Azure {
        hostname: String,
        device_id: String,
    },
    
    /// Tencent IoT configuration
    Tencent {
        endpoint: String,
        port: u16,
        product_id: String,
        device_name: String,
    },
    
    /// Huawei IoT configuration
    Huawei {
        endpoint: String,
        port: u16,
        device_id: String,
    },
    
    /// Custom provider configuration
    Custom {
        broker: String,
        port: u16,
        client_id: String,
    },
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// Certificate-based authentication
    Certificate {
        cert_path: String,
        key_path: String,
        #[serde(default)]
        ca_path: Option<String>,
    },
    
    /// Device secret authentication (Aliyun)
    DeviceSecret {
        product_key: String,
        device_name: String,
        device_secret: String,
    },
    
    /// SAS token authentication (Azure)
    SasToken {
        token: String,
        #[serde(default)]
        expires_at: Option<u64>,
    },
    
    /// Username/password authentication
    UsernamePassword {
        username: String,
        password: String,
    },
    
    /// Custom authentication
    Custom {
        #[serde(flatten)]
        params: HashMap<String, String>,
    },
}

/// Basic authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicAuth {
    pub username: String,
    pub password: String,
}

/// HTTP authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpAuth {
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    Custom {
        #[serde(flatten)]
        headers: HashMap<String, String>,
    },
}

/// Topic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicConfig {
    /// Publish topic
    pub publish: String,
    
    /// Subscribe topic
    #[serde(default)]
    pub subscribe: Option<String>,
    
    /// QoS level
    #[serde(default)]
    pub qos: u8,
    
    /// Retain flag
    #[serde(default)]
    pub retain: bool,
}

/// Cloud topic configuration with templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTopicConfig {
    /// Publish topic template
    pub publish_template: String,
    
    /// Subscribe topic template
    #[serde(default)]
    pub subscribe_template: Option<String>,
    
    /// Topic variables
    #[serde(default)]
    pub variables: HashMap<String, String>,
    
    /// QoS level
    #[serde(default)]
    pub qos: u8,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    /// Enable TLS
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Verify server certificate
    #[serde(default = "default_true")]
    pub verify_cert: bool,
    
    /// Verify hostname
    #[serde(default = "default_true")]
    pub verify_hostname: bool,
    
    /// ALPN protocols
    #[serde(default)]
    pub alpn_protocols: Vec<String>,
}

/// Connection settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionSettings {
    /// Keep alive interval in seconds
    #[serde(default = "default_keep_alive")]
    pub keep_alive_secs: u64,
    
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub timeout_secs: u64,
    
    /// Reconnect interval in seconds
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval_secs: u64,
    
    /// Maximum reconnect attempts
    #[serde(default = "default_max_reconnects")]
    pub max_reconnect_attempts: u32,
}

/// AWS IoT specific features configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AwsIotFeatures {
    /// Enable AWS IoT Jobs support
    #[serde(default)]
    pub jobs_enabled: bool,
    
    /// Enable Device Shadow support
    #[serde(default)]
    pub device_shadow_enabled: bool,
    
    /// Enable Fleet Provisioning support  
    #[serde(default)]
    pub fleet_provisioning_enabled: bool,
    
    /// AWS IoT Jobs topic prefix
    #[serde(default = "default_jobs_topic")]
    pub jobs_topic_prefix: String,
    
    /// Device Shadow topic prefix
    #[serde(default = "default_shadow_topic")]
    pub shadow_topic_prefix: String,
    
    /// Fleet Provisioning template name
    #[serde(default)]
    pub provisioning_template: Option<String>,
    
    /// Auto-respond to AWS IoT Jobs
    #[serde(default = "default_true")]
    pub auto_respond_jobs: bool,
    
    /// Maximum concurrent jobs
    #[serde(default = "default_max_jobs")]
    pub max_concurrent_jobs: u32,
}

/// Data format type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    Json,
    Ascii,
    Binary,
    Protobuf,
}

impl Default for FormatType {
    fn default() -> Self {
        FormatType::Json
    }
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_data_key_pattern() -> String {
    "voltage:data:*".to_string()
}

fn default_polling_interval() -> u64 {
    1
}

fn default_buffer_size() -> usize {
    1000
}

fn default_http_method() -> String {
    "POST".to_string()
}

fn default_http_timeout() -> u64 {
    30
}

fn default_keep_alive() -> u64 {
    60
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_reconnect_interval() -> u64 {
    5
}

fn default_max_reconnects() -> u32 {
    0 // 0 means infinite
}

fn default_jobs_topic() -> String {
    "$aws/things/{thing_name}/jobs".to_string()
}

fn default_shadow_topic() -> String {
    "$aws/things/{thing_name}/shadow".to_string()
}

fn default_max_jobs() -> u32 {
    5
}

impl Configurable for NetServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate data configuration
        if self.data.redis_polling_interval_secs == 0 {
            return Err(ConfigError::Validation(
                "Redis polling interval cannot be 0".into()
            ));
        }
        
        if self.data.enable_buffering && self.data.buffer_size == 0 {
            return Err(ConfigError::Validation(
                "Buffer size cannot be 0 when buffering is enabled".into()
            ));
        }
        
        // Validate each network configuration
        for (idx, network) in self.networks.iter().enumerate() {
            match network {
                NetworkConfig::CloudMqtt(config) => {
                    // Validate provider-specific requirements
                    match (&config.provider, &config.auth) {
                        (CloudProvider::Aws, AuthConfig::Certificate { .. }) => {},
                        (CloudProvider::Aws, _) => {
                            return Err(ConfigError::Validation(format!(
                                "Network {}: AWS IoT requires certificate authentication",
                                idx
                            )));
                        }
                        
                        (CloudProvider::Aliyun, AuthConfig::DeviceSecret { .. }) => {},
                        (CloudProvider::Aliyun, _) => {
                            return Err(ConfigError::Validation(format!(
                                "Network {}: Aliyun IoT requires device secret authentication",
                                idx
                            )));
                        }
                        
                        (CloudProvider::Azure, AuthConfig::SasToken { .. }) => {},
                        (CloudProvider::Azure, AuthConfig::UsernamePassword { .. }) => {},
                        (CloudProvider::Azure, _) => {
                            return Err(ConfigError::Validation(format!(
                                "Network {}: Azure IoT Hub requires SAS token or username/password authentication",
                                idx
                            )));
                        }
                        
                        _ => {} // Other providers are more flexible
                    }
                }
                
                NetworkConfig::Http(config) => {
                    // Validate HTTP method
                    let valid_methods = ["GET", "POST", "PUT", "DELETE", "PATCH"];
                    if !valid_methods.contains(&config.method.as_str()) {
                        return Err(ConfigError::Validation(format!(
                            "Network {}: Invalid HTTP method: {}",
                            idx, config.method
                        )));
                    }
                }
                
                _ => {} // Legacy MQTT has fewer restrictions
            }
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for NetServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}

impl Default for NetServiceConfig {
    fn default() -> Self {
        Self {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "netsrv".to_string(),
                    version: "0.1.0".to_string(),
                    description: "Network Forwarding Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:net:".to_string(),
                    pool_size: 20,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: None,
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9095,
                    health_check_enabled: true,
                    health_check_port: 8096,
                    health_check_interval: 30,
                },
            },
            data: DataConfig {
                redis_data_key: "voltage:data:*".to_string(),
                redis_polling_interval_secs: 1,
                enable_buffering: true,
                buffer_size: 1000,
            },
            networks: vec![],
        }
    }
}

impl NetServiceConfig {
    /// Load configuration using the unified framework
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("netsrv.yml")
            .environment(Environment::from_env())
            .env_prefix("NET")
            .defaults(serde_json::json!({
                "service": {
                    "name": "netsrv",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Network Forwarding Service"
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "prefix": "voltage:net:",
                    "pool_size": 20
                },
                "logging": {
                    "level": "info",
                    "console": true,
                    "file": {
                        "path": "logs/netsrv.log",
                        "rotation": "daily",
                        "max_size": "100MB",
                        "max_files": 7
                    }
                },
                "monitoring": {
                    "metrics_enabled": true,
                    "metrics_port": 9095,
                    "health_check_enabled": true,
                    "health_check_port": 8096
                },
                "data": {
                    "redis_data_key": "voltage:data:*",
                    "redis_polling_interval_secs": 1,
                    "enable_buffering": true,
                    "buffer_size": 1000
                },
                "networks": [
                    {
                        "type": "cloud_mqtt",
                        "name": "aws_iot_default",
                        "provider": "aws",
                        "provider_config": {
                            "endpoint": "your-endpoint.iot.us-east-1.amazonaws.com",
                            "port": 8883,
                            "thing_name": "voltage_device_01"
                        },
                        "auth": {
                            "type": "certificate",
                            "cert_path": "certs/aws/device.crt",
                            "key_path": "certs/aws/device.key",
                            "ca_path": "certs/aws/AmazonRootCA1.pem"
                        },
                        "topics": {
                            "publish_template": "voltage/devices/{thing_name}/telemetry",
                            "subscribe_template": "voltage/devices/{thing_name}/commands",
                            "variables": {
                                "thing_name": "voltage_device_01"
                            },
                            "qos": 1
                        },
                        "format_type": "json",
                        "tls": {
                            "enabled": true,
                            "verify_cert": true,
                            "verify_hostname": true,
                            "alpn_protocols": ["x-amzn-mqtt-ca"]
                        }
                    }
                ]
            }))?
            .build()?;
        
        let config: NetServiceConfig = loader.load_async().await
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        
        // Validate complete configuration
        config.validate_all()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;
        
        Ok(config)
    }
    
    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = NetServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "netsrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Network Forwarding Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:net:".to_string(),
                    pool_size: 20,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: Some(voltage_config::base::LogFileConfig {
                        path: "logs/netsrv.log".to_string(),
                        rotation: "daily".to_string(),
                        max_size: "100MB".to_string(),
                        max_files: 7,
                    }),
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9095,
                    health_check_enabled: true,
                    health_check_port: 8096,
                    health_check_interval: 30,
                },
            },
            data: DataConfig {
                redis_data_key: "voltage:data:*".to_string(),
                redis_polling_interval_secs: 1,
                enable_buffering: true,
                buffer_size: 1000,
            },
            networks: vec![
                // Example AWS IoT configuration
                NetworkConfig::CloudMqtt(CloudMqttConfig {
                    name: "aws_iot_example".to_string(),
                    provider: CloudProvider::Aws,
                    provider_config: ProviderConfig::Aws {
                        endpoint: "xxxxx.iot.us-east-1.amazonaws.com".to_string(),
                        port: 8883,
                        thing_name: "voltage_device_01".to_string(),
                    },
                    auth: AuthConfig::Certificate {
                        cert_path: "certs/aws/device.crt".to_string(),
                        key_path: "certs/aws/device.key".to_string(),
                        ca_path: Some("certs/aws/AmazonRootCA1.pem".to_string()),
                    },
                    topics: CloudTopicConfig {
                        publish_template: "voltage/devices/{thing_name}/telemetry".to_string(),
                        subscribe_template: Some("voltage/devices/{thing_name}/commands".to_string()),
                        variables: [("thing_name".to_string(), "voltage_device_01".to_string())]
                            .into_iter().collect(),
                        qos: 1,
                    },
                    format_type: FormatType::Json,
                    tls: TlsConfig::default(),
                    aws_features: AwsIotFeatures {
                        jobs_enabled: true,
                        device_shadow_enabled: true,
                        fleet_provisioning_enabled: false,
                        jobs_topic_prefix: "$aws/things/voltage_device_01/jobs".to_string(),
                        shadow_topic_prefix: "$aws/things/voltage_device_01/shadow".to_string(),
                        provisioning_template: None,
                        auto_respond_jobs: true,
                        max_concurrent_jobs: 5,
                    },
                }),
                
                // Example traditional MQTT configuration
                NetworkConfig::LegacyMqtt(LegacyMqttConfig {
                    name: "local_mqtt".to_string(),
                    broker: "mqtt://localhost:1883".to_string(),
                    client_id: "voltage_netsrv".to_string(),
                    auth: Some(BasicAuth {
                        username: "voltage".to_string(),
                        password: "password".to_string(),
                    }),
                    topics: TopicConfig {
                        publish: "voltage/data".to_string(),
                        subscribe: Some("voltage/commands".to_string()),
                        qos: 1,
                        retain: false,
                    },
                    format_type: FormatType::Json,
                    connection: ConnectionSettings::default(),
                }),
            ],
        };
        
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cloud_provider_validation() {
        let mut config = NetServiceConfig {
            base: Default::default(),
            data: DataConfig {
                redis_data_key: "voltage:data:*".to_string(),
                redis_polling_interval_secs: 1,
                enable_buffering: true,
                buffer_size: 1000,
            },
            networks: vec![
                NetworkConfig::CloudMqtt(CloudMqttConfig {
                    name: "aws_test".to_string(),
                    provider: CloudProvider::Aws,
                    provider_config: ProviderConfig::Aws {
                        endpoint: "test.iot.amazonaws.com".to_string(),
                        port: 8883,
                        thing_name: "test_thing".to_string(),
                    },
                    auth: AuthConfig::Certificate {
                        cert_path: "cert.pem".to_string(),
                        key_path: "key.pem".to_string(),
                        ca_path: None,
                    },
                    topics: CloudTopicConfig {
                        publish_template: "test/topic".to_string(),
                        subscribe_template: None,
                        variables: HashMap::new(),
                        qos: 1,
                    },
                    format_type: FormatType::Json,
                    tls: TlsConfig::default(),
                }),
            ],
        };
        
        // Valid AWS configuration should pass
        assert!(config.validate().is_ok());
        
        // AWS with wrong auth type should fail
        if let NetworkConfig::CloudMqtt(ref mut cloud_config) = config.networks[0] {
            cloud_config.auth = AuthConfig::UsernamePassword {
                username: "user".to_string(),
                password: "pass".to_string(),
            };
        }
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_http_method_validation() {
        let config = NetServiceConfig {
            base: Default::default(),
            data: DataConfig {
                redis_data_key: "voltage:data:*".to_string(),
                redis_polling_interval_secs: 1,
                enable_buffering: true,
                buffer_size: 1000,
            },
            networks: vec![
                NetworkConfig::Http(HttpConfig {
                    name: "http_test".to_string(),
                    url: "https://api.example.com/data".to_string(),
                    method: "INVALID".to_string(),
                    auth: None,
                    headers: HashMap::new(),
                    timeout_secs: 30,
                    format_type: FormatType::Json,
                }),
            ],
        };
        
        // Invalid HTTP method should fail
        assert!(config.validate().is_err());
    }
}