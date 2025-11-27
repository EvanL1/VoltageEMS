//! Communication Link Error Types
//!
//! Core error types for communication protocols.

use thiserror::Error;

/// Result type for voltage-comlink operations
pub type Result<T> = std::result::Result<T, ComLinkError>;

/// Communication link errors
#[derive(Debug, Error, Clone)]
pub enum ComLinkError {
    /// Protocol-level errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),

    /// Not connected
    #[error("Not connected")]
    NotConnected,

    /// IO errors
    #[error("IO error: {0}")]
    Io(String),

    /// Timeout errors
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Invalid data
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Data conversion errors
    #[error("Data conversion error: {0}")]
    DataConversion(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Channel not found
    #[error("Channel not found: {0}")]
    ChannelNotFound(u16),

    /// Point not found
    #[error("Point not found: {0}")]
    PointNotFound(String),

    /// Not supported
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),

    /// Modbus specific errors
    #[error("Modbus error: {0}")]
    Modbus(String),

    /// CAN specific errors
    #[error("CAN error: {0}")]
    Can(String),
}

impl From<std::io::Error> for ComLinkError {
    fn from(err: std::io::Error) -> Self {
        ComLinkError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for ComLinkError {
    fn from(err: serde_json::Error) -> Self {
        ComLinkError::InvalidData(format!("JSON error: {}", err))
    }
}

impl From<voltage_modbus::ModbusError> for ComLinkError {
    fn from(err: voltage_modbus::ModbusError) -> Self {
        ComLinkError::Modbus(err.to_string())
    }
}

// Helper methods for creating errors
impl ComLinkError {
    pub fn protocol(msg: impl Into<String>) -> Self {
        ComLinkError::Protocol(msg.into())
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        ComLinkError::Connection(msg.into())
    }

    pub fn io(msg: impl Into<String>) -> Self {
        ComLinkError::Io(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        ComLinkError::Timeout(msg.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        ComLinkError::InvalidData(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        ComLinkError::Config(msg.into())
    }

    pub fn modbus(msg: impl Into<String>) -> Self {
        ComLinkError::Modbus(msg.into())
    }

    pub fn can(msg: impl Into<String>) -> Self {
        ComLinkError::Can(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        ComLinkError::Internal(msg.into())
    }

    /// Check if this error indicates a need for reconnection
    pub fn needs_reconnect(&self) -> bool {
        match self {
            ComLinkError::Io(msg) => {
                msg.contains("Broken pipe")
                    || msg.contains("Connection reset")
                    || msg.contains("Connection refused")
                    || msg.contains("Connection aborted")
                    || msg.contains("Network is unreachable")
            },
            ComLinkError::Connection(_) => true,
            ComLinkError::NotConnected => true,
            _ => false,
        }
    }
}
