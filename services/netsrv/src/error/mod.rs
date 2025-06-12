use std::fmt;

/// Network service errors
#[derive(Debug)]
pub enum NetSrvError {
    /// Connection errors
    ConnectionError(String),
    /// Network operation errors
    NetworkError(String),
    /// Data formatting errors
    FormatError(String),
    /// Configuration errors
    ConfigError(String),
    /// Redis errors
    RedisError(String),
    /// MQTT errors
    MqttError(String),
    /// HTTP errors
    HttpError(String),
    /// I/O errors
    IoError(String),
    /// Data errors
    DataError(String),
}

impl fmt::Display for NetSrvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetSrvError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            NetSrvError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            NetSrvError::FormatError(msg) => write!(f, "Format error: {}", msg),
            NetSrvError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            NetSrvError::RedisError(msg) => write!(f, "Redis error: {}", msg),
            NetSrvError::MqttError(msg) => write!(f, "MQTT error: {}", msg),
            NetSrvError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            NetSrvError::IoError(msg) => write!(f, "I/O error: {}", msg),
            NetSrvError::DataError(msg) => write!(f, "Data error: {}", msg),
        }
    }
}

impl std::error::Error for NetSrvError {}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, NetSrvError>;

// Convert from Redis error
impl From<redis::RedisError> for NetSrvError {
    fn from(err: redis::RedisError) -> Self {
        NetSrvError::RedisError(err.to_string())
    }
}

// Convert from Config error
impl From<config::ConfigError> for NetSrvError {
    fn from(err: config::ConfigError) -> Self {
        NetSrvError::ConfigError(err.to_string())
    }
}

// Convert from serde_json error
impl From<serde_json::Error> for NetSrvError {
    fn from(err: serde_json::Error) -> Self {
        NetSrvError::FormatError(err.to_string())
    }
}

// Convert from Reqwest error
impl From<reqwest::Error> for NetSrvError {
    fn from(err: reqwest::Error) -> Self {
        NetSrvError::HttpError(err.to_string())
    }
}

// Convert from rumqttc error
impl From<rumqttc::ClientError> for NetSrvError {
    fn from(err: rumqttc::ClientError) -> Self {
        NetSrvError::MqttError(err.to_string())
    }
}

// Convert from IO error
impl From<std::io::Error> for NetSrvError {
    fn from(err: std::io::Error) -> Self {
        NetSrvError::IoError(err.to_string())
    }
} 