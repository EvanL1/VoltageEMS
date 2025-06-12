use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
#[serde(tag = "type", rename_all = "lowercase")]
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

/// Unified cloud MQTT configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudMqttConfig {
    pub name: String,
    pub enabled: bool,
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

impl CloudMqttConfig {
    /// Create AWS IoT configuration template
    pub fn aws_iot_template() -> Self {
        Self {
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
            reconnect_delay_ms: 5000,
            max_reconnect_attempts: 5,
            custom_properties: None,
        }
    }

    /// Create Aliyun IoT configuration template
    pub fn aliyun_iot_template() -> Self {
        Self {
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
            reconnect_delay_ms: 5000,
            max_reconnect_attempts: 5,
            custom_properties: None,
        }
    }

    /// Create Azure IoT Hub configuration template
    pub fn azure_iot_template() -> Self {
        Self {
            name: "Azure IoT Hub".to_string(),
            enabled: false,
            cloud_provider: CloudProvider::Azure,
            endpoint: "your-hub.azure-devices.net".to_string(),
            port: 8883,
            client_id: "your-device-id".to_string(),
            auth_config: AuthConfig::SasToken {
                token: "SharedAccessSignature sr=...".to_string(),
                expiry: None,
            },
            topic_config: TopicConfig {
                publish_topic: "devices/{device_id}/messages/events/".to_string(),
                subscribe_topics: Some(vec![
                    "devices/{device_id}/messages/devicebound/#".to_string()
                ]),
                ..Default::default()
            },
            tls_config: TlsConfig::default(),
            keep_alive_secs: 30,
            connection_timeout_ms: 10000,
            reconnect_delay_ms: 5000,
            max_reconnect_attempts: 5,
            custom_properties: None,
        }
    }

    /// Create Tencent IoT Hub configuration template
    pub fn tencent_iot_template() -> Self {
        Self {
            name: "Tencent IoT Hub".to_string(),
            enabled: false,
            cloud_provider: CloudProvider::Tencent,
            endpoint: "your-product.iotcloud.tencentdevices.com".to_string(),
            port: 8883,
            client_id: "your-product-id-your-device-name".to_string(),
            auth_config: AuthConfig::Certificate {
                cert_path: "/path/to/device-cert.crt".to_string(),
                key_path: "/path/to/device-key.key".to_string(),
                ca_path: "/path/to/root-ca.crt".to_string(),
            },
            topic_config: TopicConfig {
                publish_topic: "your-product-id/{device_name}/data".to_string(),
                subscribe_topics: Some(vec![
                    "your-product-id/{device_name}/control".to_string()
                ]),
                ..Default::default()
            },
            tls_config: TlsConfig::default(),
            keep_alive_secs: 240,
            connection_timeout_ms: 10000,
            reconnect_delay_ms: 5000,
            max_reconnect_attempts: 5,
            custom_properties: None,
        }
    }

    /// Validate configuration based on cloud provider requirements
    pub fn validate(&self) -> Result<(), String> {
        match self.cloud_provider {
            CloudProvider::Aws => self.validate_aws(),
            CloudProvider::Aliyun => self.validate_aliyun(),
            CloudProvider::Azure => self.validate_azure(),
            CloudProvider::Tencent => self.validate_tencent(),
            CloudProvider::Huawei => self.validate_huawei(),
            CloudProvider::Custom => Ok(()), // Custom configs are flexible
        }
    }

    fn validate_aws(&self) -> Result<(), String> {
        if !matches!(self.auth_config, AuthConfig::Certificate { .. }) {
            return Err("AWS IoT requires certificate authentication".to_string());
        }
        if self.port != 8883 && self.port != 443 {
            return Err("AWS IoT typically uses port 8883 or 443".to_string());
        }
        Ok(())
    }

    fn validate_aliyun(&self) -> Result<(), String> {
        if !matches!(self.auth_config, AuthConfig::DeviceSecret { .. }) {
            return Err("Aliyun IoT requires device secret authentication".to_string());
        }
        Ok(())
    }

    fn validate_azure(&self) -> Result<(), String> {
        if !matches!(self.auth_config, AuthConfig::SasToken { .. }) {
            return Err("Azure IoT typically uses SAS token authentication".to_string());
        }
        Ok(())
    }

    fn validate_tencent(&self) -> Result<(), String> {
        if !matches!(self.auth_config, AuthConfig::Certificate { .. }) {
            return Err("Tencent IoT requires certificate authentication".to_string());
        }
        Ok(())
    }

    fn validate_huawei(&self) -> Result<(), String> {
        // Huawei IoT supports both certificate and device secret
        match self.auth_config {
            AuthConfig::Certificate { .. } | AuthConfig::DeviceSecret { .. } => Ok(()),
            _ => Err("Huawei IoT requires certificate or device secret authentication".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_iot_template() {
        let config = CloudMqttConfig::aws_iot_template();
        assert_eq!(config.cloud_provider, CloudProvider::Aws);
        assert_eq!(config.port, 8883);
        assert!(matches!(config.auth_config, AuthConfig::Certificate { .. }));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_aliyun_iot_template() {
        let config = CloudMqttConfig::aliyun_iot_template();
        assert_eq!(config.cloud_provider, CloudProvider::Aliyun);
        assert!(matches!(config.auth_config, AuthConfig::DeviceSecret { .. }));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_azure_iot_template() {
        let config = CloudMqttConfig::azure_iot_template();
        assert_eq!(config.cloud_provider, CloudProvider::Azure);
        assert!(matches!(config.auth_config, AuthConfig::SasToken { .. }));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tencent_iot_template() {
        let config = CloudMqttConfig::tencent_iot_template();
        assert_eq!(config.cloud_provider, CloudProvider::Tencent);
        assert!(matches!(config.auth_config, AuthConfig::Certificate { .. }));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_aws_config() {
        let mut config = CloudMqttConfig::aws_iot_template();
        config.auth_config = AuthConfig::UsernamePassword {
            username: "test".to_string(),
            password: "test".to_string(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_topic_config_default() {
        let topic_config = TopicConfig::default();
        assert_eq!(topic_config.publish_topic, "ems/data");
        assert_eq!(topic_config.qos, 1);
        assert!(!topic_config.retain);
    }

    #[test]
    fn test_tls_config_default() {
        let tls_config = TlsConfig::default();
        assert!(tls_config.enabled);
        assert!(tls_config.verify_cert);
        assert!(tls_config.verify_hostname);
        assert_eq!(tls_config.alpn_protocols, Some(vec!["mqtt".to_string()]));
    }
} 