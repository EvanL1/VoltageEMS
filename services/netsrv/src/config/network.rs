use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified network configuration that supports both legacy and cloud protocols
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NetworkConfig {
    /// Legacy MQTT configuration
    Mqtt {
        name: String,
        enabled: bool,
        format_type: FormatType,
        broker_url: String,
        port: u16,
        client_id: String,
        username: Option<String>,
        password: Option<String>,
        topic: String,
        qos: u8,
        data_filter: Option<Vec<String>>,
    },
    /// HTTP REST API configuration
    Http {
        name: String,
        enabled: bool,
        format_type: FormatType,
        url: String,
        method: String,
        headers: Option<HashMap<String, String>>,
        auth_type: Option<String>,
        username: Option<String>,
        password: Option<String>,
        token: Option<String>,
        timeout_ms: u64,
        data_filter: Option<Vec<String>>,
    },
    /// Cloud MQTT configuration (AWS, Aliyun, Azure, etc.)
    Cloud {
        name: String,
        enabled: bool,
        cloud_provider: CloudProvider,
        endpoint: String,
        port: u16,
        client_id: String,
        auth_config: AuthConfig,
        topic_config: TopicConfig,
        tls_config: TlsConfig,
        keep_alive_secs: u64,
        connection_timeout_ms: u64,
    },
}

/// Data format types
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    Json,
    Ascii,
}

impl From<FormatType> for crate::formatter::FormatType {
    fn from(format_type: FormatType) -> Self {
        match format_type {
            FormatType::Json => crate::formatter::FormatType::Json,
            FormatType::Ascii => crate::formatter::FormatType::Ascii,
        }
    }
}

/// Supported cloud providers
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    Aws,
    Aliyun,
    Tencent,
    Huawei,
    Azure,
    Custom,
}

impl std::fmt::Display for CloudProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloudProvider::Aws => write!(f, "AWS"),
            CloudProvider::Aliyun => write!(f, "Aliyun"),
            CloudProvider::Azure => write!(f, "Azure"),
            CloudProvider::Tencent => write!(f, "Tencent"),
            CloudProvider::Huawei => write!(f, "Huawei"),
            CloudProvider::Custom => write!(f, "Custom"),
        }
    }
}

/// Authentication configuration for different cloud providers
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "auth_type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// X.509 Certificate authentication (AWS IoT, etc.)
    Certificate {
        cert_path: String,
        key_path: String,
        ca_path: String,
    },
    /// Device secret authentication (Aliyun IoT, etc.)
    DeviceSecret {
        product_key: String,
        device_name: String,
        device_secret: String,
    },
    /// SAS Token authentication (Azure IoT, etc.)
    SasToken {
        token: String,
        expiry: Option<i64>,
    },
    /// Username/Password authentication
    UsernamePassword {
        username: String,
        password: String,
    },
    /// Custom authentication with flexible parameters
    Custom {
        params: HashMap<String, String>,
    },
}

/// TLS configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TlsConfig {
    pub enabled: bool,
    pub verify_cert: bool,
    pub verify_hostname: bool,
    pub ca_path: Option<String>,
    pub alpn_protocols: Option<Vec<String>>,
}

/// Topic configuration for cloud providers
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopicConfig {
    /// Base topic for publishing data
    pub publish_topic: String,
    /// Topic template variables (e.g., {device_id}, {timestamp})
    pub topic_variables: Option<HashMap<String, String>>,
    /// Subscribe topics for receiving commands
    pub subscribe_topics: Option<Vec<String>>,
    /// Quality of Service level
    pub qos: u8,
    /// Retain message flag
    pub retain: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            verify_cert: true,
            verify_hostname: true,
            ca_path: None,
            alpn_protocols: Some(vec!["mqtt".to_string()]),
        }
    }
}

impl Default for TopicConfig {
    fn default() -> Self {
        Self {
            publish_topic: "ems/data".to_string(),
            topic_variables: None,
            subscribe_topics: None,
            qos: 1,
            retain: false,
        }
    }
}

impl NetworkConfig {
    pub fn name(&self) -> &str {
        match self {
            NetworkConfig::Mqtt { name, .. } => name,
            NetworkConfig::Http { name, .. } => name,
            NetworkConfig::Cloud { name, .. } => name,
        }
    }

    pub fn is_enabled(&self) -> bool {
        match self {
            NetworkConfig::Mqtt { enabled, .. } => *enabled,
            NetworkConfig::Http { enabled, .. } => *enabled,
            NetworkConfig::Cloud { enabled, .. } => *enabled,
        }
    }

    pub fn format_type(&self) -> Option<&FormatType> {
        match self {
            NetworkConfig::Mqtt { format_type, .. } => Some(format_type),
            NetworkConfig::Http { format_type, .. } => Some(format_type),
            NetworkConfig::Cloud { .. } => None, // Cloud networks use JSON by default
        }
    }

    /// Create default MQTT configuration
    pub fn default_mqtt() -> Self {
        NetworkConfig::Mqtt {
            name: "Default MQTT".to_string(),
            enabled: false,
            format_type: FormatType::Json,
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "netsrv-client".to_string(),
            username: None,
            password: None,
            topic: "ems/data".to_string(),
            qos: 0,
            data_filter: None,
        }
    }

    /// Create default HTTP configuration
    pub fn default_http() -> Self {
        NetworkConfig::Http {
            name: "Default HTTP".to_string(),
            enabled: false,
            format_type: FormatType::Json,
            url: "http://localhost:8080/api/data".to_string(),
            method: "POST".to_string(),
            headers: Some(HashMap::new()),
            auth_type: None,
            username: None,
            password: None,
            token: None,
            timeout_ms: 5000,
            data_filter: None,
        }
    }

    /// Create AWS IoT configuration template
    pub fn aws_iot_template() -> Self {
        NetworkConfig::Cloud {
            name: "AWS IoT Core".to_string(),
            enabled: false,
            cloud_provider: CloudProvider::Aws,
            endpoint: "your-endpoint.iot.region.amazonaws.com".to_string(),
            port: 8883,
            client_id: "ems-device-{device_id}".to_string(),
            auth_config: AuthConfig::Certificate {
                cert_path: "/path/to/device-cert.pem".to_string(),
                key_path: "/path/to/device-key.pem".to_string(),
                ca_path: "/path/to/root-ca.pem".to_string(),
            },
            topic_config: TopicConfig {
                publish_topic: "ems/{device_id}/data".to_string(),
                subscribe_topics: Some(vec!["ems/{device_id}/commands".to_string()]),
                ..Default::default()
            },
            tls_config: TlsConfig::default(),
            keep_alive_secs: 30,
            connection_timeout_ms: 10000,
        }
    }

    /// Create Aliyun IoT configuration template
    pub fn aliyun_iot_template() -> Self {
        NetworkConfig::Cloud {
            name: "Aliyun IoT Platform".to_string(),
            enabled: false,
            cloud_provider: CloudProvider::Aliyun,
            endpoint: "your-product.iot-as-mqtt.region.aliyuncs.com".to_string(),
            port: 443,
            client_id: "your-device-client-id".to_string(),
            auth_config: AuthConfig::DeviceSecret {
                product_key: "your-product-key".to_string(),
                device_name: "your-device-name".to_string(),
                device_secret: "your-device-secret".to_string(),
            },
            topic_config: TopicConfig {
                publish_topic: "/sys/{product_key}/{device_name}/thing/event/property/post".to_string(),
                subscribe_topics: Some(vec![
                    "/sys/{product_key}/{device_name}/thing/service/property/set".to_string()
                ]),
                ..Default::default()
            },
            tls_config: TlsConfig::default(),
            keep_alive_secs: 60,
            connection_timeout_ms: 10000,
        }
    }

    /// Validate cloud configuration based on provider requirements
    pub fn validate(&self) -> Result<(), String> {
        if let NetworkConfig::Cloud { cloud_provider, auth_config, port, .. } = self {
            match cloud_provider {
                CloudProvider::Aws => {
                    if !matches!(auth_config, AuthConfig::Certificate { .. }) {
                        return Err("AWS IoT requires certificate authentication".to_string());
                    }
                    if *port != 8883 && *port != 443 {
                        return Err("AWS IoT typically uses port 8883 or 443".to_string());
                    }
                }
                CloudProvider::Aliyun => {
                    if !matches!(auth_config, AuthConfig::DeviceSecret { .. }) {
                        return Err("Aliyun IoT requires device secret authentication".to_string());
                    }
                }
                CloudProvider::Azure => {
                    if !matches!(auth_config, AuthConfig::SasToken { .. }) {
                        return Err("Azure IoT typically uses SAS token authentication".to_string());
                    }
                }
                CloudProvider::Tencent => {
                    if !matches!(auth_config, AuthConfig::Certificate { .. }) {
                        return Err("Tencent IoT requires certificate authentication".to_string());
                    }
                }
                CloudProvider::Huawei => {
                    match auth_config {
                        AuthConfig::Certificate { .. } | AuthConfig::DeviceSecret { .. } => {}
                        _ => return Err("Huawei IoT requires certificate or device secret authentication".to_string()),
                    }
                }
                CloudProvider::Custom => {} // Custom configs are flexible
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_iot_template() {
        let config = NetworkConfig::aws_iot_template();
        if let NetworkConfig::Cloud { cloud_provider, auth_config, port, .. } = config {
            assert_eq!(cloud_provider, CloudProvider::Aws);
            assert_eq!(port, 8883);
            assert!(matches!(auth_config, AuthConfig::Certificate { .. }));
        } else {
            panic!("Expected Cloud configuration");
        }
    }

    #[test]
    fn test_aliyun_iot_template() {
        let config = NetworkConfig::aliyun_iot_template();
        if let NetworkConfig::Cloud { cloud_provider, auth_config, .. } = config {
            assert_eq!(cloud_provider, CloudProvider::Aliyun);
            assert!(matches!(auth_config, AuthConfig::DeviceSecret { .. }));
        } else {
            panic!("Expected Cloud configuration");
        }
    }

    #[test]
    fn test_validation() {
        let config = NetworkConfig::aws_iot_template();
        assert!(config.validate().is_ok());

        let config = NetworkConfig::aliyun_iot_template();
        assert!(config.validate().is_ok());
    }
} 