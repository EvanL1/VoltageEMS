use std::fmt;
use thiserror::Error;
use crate::utils::error::ComSrvError;

/// Base communication error types that all protocols can extend
/// 
/// This is a wrapper around ComSrvError that provides protocol-specific
/// error handling utilities while leveraging the comprehensive error
/// classification from the main error system.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BaseCommError {
    /// Core communication service error
    #[error(transparent)]
    Core(#[from] ComSrvError),
    
    /// Protocol-specific error with custom type and context
    #[error("Protocol-specific error ({error_type}): {message}")]
    ProtocolSpecific { error_type: String, message: String },
}

impl BaseCommError {
    /// Create a connection error
    pub fn connection<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::ConnectionError(message.into()))
    }
    
    /// Create a timeout error with specific timeout value
    pub fn timeout(timeout_ms: u64) -> Self {
        Self::Core(ComSrvError::TimeoutError(format!("operation timed out after {}ms", timeout_ms)))
    }
    
    /// Create a protocol error
    pub fn protocol<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::ProtocolError(message.into()))
    }
    
    /// Create a data conversion error
    pub fn data_conversion<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::ParsingError(message.into()))
    }
    
    /// Create a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::ConfigError(message.into()))
    }
    
    /// Create an I/O error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::IoError(message.into()))
    }
    
    /// Create a network error
    pub fn network<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::NetworkError(message.into()))
    }
    
    /// Create an authentication error
    pub fn authentication<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::PermissionDenied(message.into()))
    }
    
    /// Create a resource unavailable error
    pub fn resource_unavailable<S: Into<String>>(resource: S) -> Self {
        Self::Core(ComSrvError::NotFound(resource.into()))
    }
    
    /// Create an invalid state error
    pub fn invalid_state<S: Into<String>>(message: S) -> Self {
        Self::Core(ComSrvError::InvalidOperation(message.into()))
    }
    
    /// Create a protocol-specific error
    pub fn protocol_specific<S: Into<String>, T: Into<String>>(error_type: S, message: T) -> Self {
        Self::ProtocolSpecific { 
            error_type: error_type.into(), 
            message: message.into() 
        }
    }
    
    /// Get error category for metrics/logging
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Core(err) => match err {
                ComSrvError::ConnectionError(_) => ErrorCategory::Connection,
                ComSrvError::TimeoutError(_) => ErrorCategory::Timeout,
                ComSrvError::ProtocolError(_) => ErrorCategory::Protocol,
                ComSrvError::ParsingError(_) | ComSrvError::SerializationError(_) => ErrorCategory::DataConversion,
                ComSrvError::ConfigError(_) | ComSrvError::ConfigurationError(_) => ErrorCategory::Configuration,
                ComSrvError::IoError(_) => ErrorCategory::Io,
                ComSrvError::NetworkError(_) => ErrorCategory::Network,
                ComSrvError::PermissionDenied(_) => ErrorCategory::Authentication,
                ComSrvError::NotFound(_) => ErrorCategory::Resource,
                ComSrvError::InvalidOperation(_) | ComSrvError::StateError(_) => ErrorCategory::State,
                _ => ErrorCategory::Protocol,
            },
            Self::ProtocolSpecific { .. } => ErrorCategory::ProtocolSpecific,
        }
    }
    
    /// Check if error is retriable
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Core(err) => match err {
                ComSrvError::ConnectionError(_) | 
                ComSrvError::TimeoutError(_) | 
                ComSrvError::NetworkError(_) | 
                ComSrvError::IoError(_) => true,
                ComSrvError::ConfigError(_) | 
                ComSrvError::ConfigurationError(_) |
                ComSrvError::ParsingError(_) |
                ComSrvError::SerializationError(_) |
                ComSrvError::PermissionDenied(_) => false,
                _ => false,
            },
            Self::ProtocolSpecific { .. } => false, // Protocol-dependent
        }
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Core(err) => match err {
                ComSrvError::ConfigError(_) | 
                ComSrvError::ConfigurationError(_) |
                ComSrvError::PermissionDenied(_) => ErrorSeverity::Critical,
                ComSrvError::ConnectionError(_) | 
                ComSrvError::NetworkError(_) | 
                ComSrvError::NotFound(_) => ErrorSeverity::High,
                ComSrvError::ProtocolError(_) | 
                ComSrvError::ParsingError(_) |
                ComSrvError::SerializationError(_) |
                ComSrvError::InvalidOperation(_) |
                ComSrvError::StateError(_) => ErrorSeverity::Medium,
                ComSrvError::TimeoutError(_) | 
                ComSrvError::IoError(_) => ErrorSeverity::Low,
                _ => ErrorSeverity::Medium,
            },
            Self::ProtocolSpecific { .. } => ErrorSeverity::Low,
        }
    }

    /// Get the underlying ComSrvError if available
    pub fn as_core_error(&self) -> Option<&ComSrvError> {
        match self {
            Self::Core(err) => Some(err),
            Self::ProtocolSpecific { .. } => None,
        }
    }

    /// Convert to ComSrvError
    pub fn into_core_error(self) -> ComSrvError {
        match self {
            Self::Core(err) => err,
            Self::ProtocolSpecific { error_type, message } => {
                ComSrvError::ProtocolError(format!("Protocol-specific error ({}): {}", error_type, message))
            }
        }
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Connection,
    Timeout,
    Protocol,
    DataConversion,
    Configuration,
    Io,
    Network,
    Authentication,
    Resource,
    State,
    ProtocolSpecific,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection => write!(f, "connection"),
            Self::Timeout => write!(f, "timeout"),
            Self::Protocol => write!(f, "protocol"),
            Self::DataConversion => write!(f, "data_conversion"),
            Self::Configuration => write!(f, "configuration"),
            Self::Io => write!(f, "io"),
            Self::Network => write!(f, "network"),
            Self::Authentication => write!(f, "authentication"),
            Self::Resource => write!(f, "resource"),
            Self::State => write!(f, "state"),
            Self::ProtocolSpecific => write!(f, "protocol_specific"),
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Convert from standard I/O errors
impl From<std::io::Error> for BaseCommError {
    fn from(err: std::io::Error) -> Self {
        Self::Core(ComSrvError::IoError(err.to_string()))
    }
}

/// Result type using BaseCommError
pub type BaseCommResult<T> = Result<T, BaseCommError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let conn_err = BaseCommError::connection("Failed to connect to device");
        assert_eq!(conn_err.category(), ErrorCategory::Connection);
        assert!(conn_err.is_retriable());
        assert_eq!(conn_err.severity(), ErrorSeverity::High);
        
        let timeout_err = BaseCommError::timeout(5000);
        assert_eq!(timeout_err.category(), ErrorCategory::Timeout);
        assert!(timeout_err.is_retriable());
        assert_eq!(timeout_err.severity(), ErrorSeverity::Low);
        
        let config_err = BaseCommError::configuration("Invalid port number");
        assert_eq!(config_err.category(), ErrorCategory::Configuration);
        assert!(!config_err.is_retriable());
        assert_eq!(config_err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_protocol_specific_error() {
        let modbus_err = BaseCommError::protocol_specific("modbus_exception", "Illegal function code");
        assert_eq!(modbus_err.category(), ErrorCategory::ProtocolSpecific);
        assert!(!modbus_err.is_retriable());
        
        if let BaseCommError::ProtocolSpecific { error_type, message } = modbus_err {
            assert_eq!(error_type, "modbus_exception");
            assert_eq!(message, "Illegal function code");
        } else {
            panic!("Expected ProtocolSpecific error");
        }
    }

    #[test]
    fn test_error_display() {
        let err = BaseCommError::connection("Connection refused");
        assert!(err.to_string().contains("Connection refused"));
        
        let timeout_err = BaseCommError::timeout(1000);
        assert!(timeout_err.to_string().contains("1000ms"));
    }

    #[test]
    fn test_error_category_display() {
        assert_eq!(ErrorCategory::Connection.to_string(), "connection");
        assert_eq!(ErrorCategory::Protocol.to_string(), "protocol");
        assert_eq!(ErrorCategory::ProtocolSpecific.to_string(), "protocol_specific");
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Low < ErrorSeverity::Medium);
        assert!(ErrorSeverity::Medium < ErrorSeverity::High);
        assert!(ErrorSeverity::High < ErrorSeverity::Critical);
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let base_err: BaseCommError = io_err.into();
        
        assert_eq!(base_err.category(), ErrorCategory::Io);
        assert!(base_err.is_retriable());
    }

    #[test]
    fn test_core_error_conversion() {
        let base_err = BaseCommError::connection("Test connection error");
        
        // Test as_core_error
        assert!(base_err.as_core_error().is_some());
        
        // Test into_core_error
        let core_err = base_err.into_core_error();
        assert!(matches!(core_err, ComSrvError::ConnectionError(_)));
    }

    #[test]
    fn test_protocol_specific_conversion() {
        let base_err = BaseCommError::protocol_specific("test", "Test protocol error");
        
        // Test as_core_error (should be None for protocol-specific)
        assert!(base_err.as_core_error().is_none());
        
        // Test into_core_error
        let core_err = base_err.into_core_error();
        assert!(matches!(core_err, ComSrvError::ProtocolError(_)));
    }
} 