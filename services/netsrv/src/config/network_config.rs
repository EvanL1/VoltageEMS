use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    Mqtt,
    Http,
    AwsIot,
    AliyunIot,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FormatType {
    Json,
    Ascii,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub name: String,
    pub enabled: bool,
    pub network_type: NetworkType,
    pub format_type: FormatType,
    pub mqtt_config: Option<MqttConfig>,
    pub http_config: Option<HttpConfig>,
    pub aws_iot_config: Option<AwsIotConfig>,
    pub aliyun_iot_config: Option<AliyunIotConfig>,
    pub data_filter: Option<Vec<String>>,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsIotConfig {
    pub endpoint: String,
    pub region: String,
    pub topic: String,
    pub thing_name: String,
    pub client_id: String,
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: String,
    pub qos: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AliyunIotConfig {
    pub product_key: String,
    pub device_name: String,
    pub device_secret: String,
    pub region_id: String,
    pub topic: String,
    pub qos: u8,
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
            aws_iot_config: None,
            aliyun_iot_config: None,
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
            aws_iot_config: None,
            aliyun_iot_config: None,
            data_filter: None,
        }
    }
} 