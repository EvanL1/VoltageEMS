use std::fmt;

/// Network service errors
#[derive(Debug)]
#[allow(dead_code)]
pub enum NetSrvError {
    /// Connection errors
    Connection(String),
    /// Data formatting errors
    Format(String),
    /// Configuration errors
    Config(String),
    /// Redis errors
    Redis(String),
    /// MQTT errors
    Mqtt(String),
    /// HTTP errors
    Http(String),
    /// I/O errors
    Io(String),
    /// Data errors
    Data(String),
    /// Network errors
    Network(String),
    /// Runtime errors
    Runtime(String),
}

impl fmt::Display for NetSrvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetSrvError::Connection(msg) => write!(f, "Connection error: {}", msg),
            NetSrvError::Format(msg) => write!(f, "Format error: {}", msg),
            NetSrvError::Config(msg) => write!(f, "Configuration error: {}", msg),
            NetSrvError::Redis(msg) => write!(f, "Redis error: {}", msg),
            NetSrvError::Mqtt(msg) => write!(f, "MQTT error: {}", msg),
            NetSrvError::Http(msg) => write!(f, "HTTP error: {}", msg),
            NetSrvError::Io(msg) => write!(f, "I/O error: {}", msg),
            NetSrvError::Data(msg) => write!(f, "Data error: {}", msg),
            NetSrvError::Network(msg) => write!(f, "Network error: {}", msg),
            NetSrvError::Runtime(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::error::Error for NetSrvError {}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, NetSrvError>;

// Redis error conversion removed - using voltage-common

// Convert from Config error
impl From<config::ConfigError> for NetSrvError {
    fn from(err: config::ConfigError) -> Self {
        NetSrvError::Config(err.to_string())
    }
}

// Convert from serde_json error
impl From<serde_json::Error> for NetSrvError {
    fn from(err: serde_json::Error) -> Self {
        NetSrvError::Format(err.to_string())
    }
}

// Convert from Reqwest error
impl From<reqwest::Error> for NetSrvError {
    fn from(err: reqwest::Error) -> Self {
        NetSrvError::Http(err.to_string())
    }
}

// Convert from rumqttc error
impl From<rumqttc::ClientError> for NetSrvError {
    fn from(err: rumqttc::ClientError) -> Self {
        NetSrvError::Mqtt(err.to_string())
    }
}

// Convert from IO error
impl From<std::io::Error> for NetSrvError {
    fn from(err: std::io::Error) -> Self {
        NetSrvError::Io(err.to_string())
    }
}
