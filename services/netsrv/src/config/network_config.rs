use serde::{Deserialize, Serialize};

/// Network type enumeration for basic protocols
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    Mqtt,
    Http,
}

/// Data format types (re-exported from formatter module for compatibility)
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

/// Network configuration for legacy MQTT and HTTP protocols
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub name: String,
    pub enabled: bool,
    pub network_type: NetworkType,
    pub format_type: FormatType,
    pub mqtt_config: Option<MqttConfig>,
    pub http_config: Option<HttpConfig>,
    pub data_filter: Option<Vec<String>>,
}

/// Basic MQTT configuration for legacy systems
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MqttConfig {
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

/// HTTP configuration for REST API integration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HttpConfig {
    pub url: String,
    pub method: String,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub auth_type: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub timeout_ms: u64,
}

impl NetworkConfig {
    pub fn default_mqtt() -> Self {
        NetworkConfig {
            name: "Default MQTT".to_string(),
            enabled: false,
            network_type: NetworkType::Mqtt,
            format_type: FormatType::Json,
            mqtt_config: Some(MqttConfig {
                broker_url: "localhost".to_string(),
                port: 1883,
                client_id: "netsrv-client".to_string(),
                username: None,
                password: None,
                topic: "ems/data".to_string(),
                qos: 0,
                use_ssl: false,
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            }),
            http_config: None,
            data_filter: None,
        }
    }

    pub fn default_http() -> Self {
        NetworkConfig {
            name: "Default HTTP".to_string(),
            enabled: false,
            network_type: NetworkType::Http,
            format_type: FormatType::Json,
            mqtt_config: None,
            http_config: Some(HttpConfig {
                url: "http://localhost:8080/api/data".to_string(),
                method: "POST".to_string(),
                headers: Some(std::collections::HashMap::new()),
                auth_type: None,
                username: None,
                password: None,
                token: None,
                timeout_ms: 5000,
            }),
            data_filter: None,
        }
    }
} 