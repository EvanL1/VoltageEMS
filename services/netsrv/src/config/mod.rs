use serde::{Deserialize, Serialize};

pub mod redis_config;
pub mod network;

use redis_config::RedisConfig;
use network::NetworkConfig;

/// Logging configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub console: bool,
    pub file: Option<String>,
}

/// Main application configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub networks: Vec<NetworkConfig>,
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            redis: RedisConfig::default(),
            networks: vec![
                NetworkConfig::default_mqtt(),
                NetworkConfig::default_http(),
            ],
            logging: LoggingConfig {
                level: "info".to_string(),
                console: true,
                file: None,
            },
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn new(config_path: &str) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(config_path))
            .build()?;

        settings.try_deserialize()
    }

    /// Generate example configuration file content
    pub fn example_config() -> serde_json::Value {
        use serde_json::json;
        
        json!({
            "redis": {
                "host": "127.0.0.1",
                "port": 6379,
                "password": "",
                "socket": "",
                "max_connections": 10,
                "retry_attempts": 3,
                "retry_delay_ms": 1000
            },
            "networks": [
                {
                    "type": "mqtt",
                    "name": "Local MQTT Broker",
                    "enabled": true,
                    "format_type": "json",
                    "broker_url": "localhost",
                    "port": 1883,
                    "client_id": "netsrv-client",
                    "username": null,
                    "password": null,
                    "topic": "ems/data",
                    "qos": 0,
                    "use_ssl": false,
                    "ca_cert_path": null,
                    "client_cert_path": null,
                    "client_key_path": null,
                    "data_filter": null
                },
                {
                    "type": "http",
                    "name": "REST API",
                    "enabled": false,
                    "format_type": "json",
                    "url": "http://localhost:8080/api/data",
                    "method": "POST",
                    "headers": {
                        "Content-Type": "application/json"
                    },
                    "auth_type": null,
                    "username": null,
                    "password": null,
                    "token": null,
                    "timeout_ms": 5000,
                    "data_filter": null
                },
                {
                    "type": "cloud",
                    "name": "AWS IoT Core",
                    "enabled": false,
                    "cloud_provider": "aws",
                    "endpoint": "your-endpoint.iot.region.amazonaws.com",
                    "port": 8883,
                    "client_id": "ems-device-{device_id}",
                    "auth_config": {
                        "auth_type": "certificate",
                        "cert_path": "/path/to/device-cert.pem",
                        "key_path": "/path/to/device-key.pem",
                        "ca_path": "/path/to/root-ca.pem"
                    },
                    "topic_config": {
                        "publish_topic": "ems/{device_id}/data",
                        "subscribe_topics": ["ems/{device_id}/commands"],
                        "qos": 1,
                        "retain": false
                    },
                    "tls_config": {
                        "enabled": true,
                        "verify_cert": true,
                        "verify_hostname": true,
                        "ca_path": null,
                        "alpn_protocols": ["mqtt"]
                    },
                    "keep_alive_secs": 30,
                    "connection_timeout_ms": 10000,
                    "reconnect_delay_ms": 5000,
                    "max_reconnect_attempts": 5,
                    "custom_properties": null
                }
            ],
            "logging": {
                "level": "info",
                "console": true,
                "file": null
            }
        })
    }
} 