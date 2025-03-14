use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetSrvError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("MQTT error: {0}")]
    MqttError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("AWS IoT error: {0}")]
    AwsIotError(String),

    #[error("Aliyun IoT error: {0}")]
    AliyunIotError(String),

    #[error("Format error: {0}")]
    FormatError(String),

    #[error("Data error: {0}")]
    DataError(String),
}

pub type Result<T> = std::result::Result<T, NetSrvError>;

// 从 reqwest 错误转换
impl From<reqwest::Error> for NetSrvError {
    fn from(err: reqwest::Error) -> Self {
        NetSrvError::HttpError(err.to_string())
    }
}

// 从 rumqttc 错误转换
impl From<rumqttc::ClientError> for NetSrvError {
    fn from(err: rumqttc::ClientError) -> Self {
        NetSrvError::MqttError(err.to_string())
    }
}

// 从 paho-mqtt 错误转换
impl From<paho_mqtt::Error> for NetSrvError {
    fn from(err: paho_mqtt::Error) -> Self {
        NetSrvError::MqttError(err.to_string())
    }
} 