//! Utility Functions and Common Components
//!
//! This module provides essential utilities, error handling, and shared
//! components used throughout the communication service library.
//!
//! # Modules
//!
//! - [`error`] - Comprehensive error types and error handling utilities
//!
//! # Key Components
//!
//! ## Error Handling
//!
//! The [`ComSrvError`] enum provides comprehensive error classification for all
//! possible error conditions in the system. The [`error::ErrorExt`] trait adds convenient
//! error conversion methods to `Result` types.
//!
//! # Examples
//!
//! ```rust
//! use comsrv::utils::{ComSrvError, Result};
//! use comsrv::utils::error::ErrorExt;
//!
//! // Error handling
//! fn example_function() -> Result<String> {
//!     std::fs::read_to_string("config.yaml")
//!         .config_error("Failed to read configuration file")
//! }
//! ```

pub mod error;

// Re-export commonly used items for convenience
pub use error::{ComSrvError, Result};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_module_exports() {
        // Test that we can use the re-exported types
        let error = ComSrvError::ConfigError("test error".to_string());
        assert!(error.to_string().contains("test error"));

        // Test Result type alias
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let error_result: Result<i32> = Err(ComSrvError::IoError("test io error".to_string()));
        assert!(error_result.is_err());
    }

    #[test]
    fn test_error_types() {
        // Test basic error creation and handling
        let config_error = ComSrvError::ConfigError("test config error".to_string());
        assert!(config_error.to_string().contains("config"));

        let io_error = ComSrvError::IoError("test io error".to_string());
        assert!(io_error.to_string().contains("io"));
    }

    #[test]
    fn test_error_conversion_integration() {
        // Test that we can convert between error types
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let comsrv_error: ComSrvError = io_error.into();
        assert!(matches!(comsrv_error, ComSrvError::IoError(_)));
    }

    #[test]
    fn test_error_context_integration() {
        use crate::utils::error::ErrorExt;

        // Test that ErrorExt trait is available through the module
        let io_result: std::result::Result<String, io::Error> = Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "access denied",
        ));

        let contextualized = io_result.context("Failed to access resource");
        assert!(contextualized.is_err());

        let error = contextualized.unwrap_err();
        assert!(error.to_string().contains("Failed to access resource"));
        assert!(error.to_string().contains("access denied"));
    }

    #[test]
    fn test_module_structure() {
        // Verify that the expected modules are accessible

        // Test error module availability
        let _error_type = ComSrvError::ConfigError("test".to_string());

        // We can't directly test logger and pool modules here without importing them,
        // but we can verify the module exists by checking compilation

        // This test mainly serves as a compilation check for module structure
        assert!(true, "Module structure is valid if this compiles");
    }

    #[test]
    fn test_comprehensive_error_types() {
        // Test various error types that should be available
        let errors = vec![
            ComSrvError::ConfigError("config".to_string()),
            ComSrvError::IoError("io".to_string()),
            ComSrvError::ProtocolError("protocol".to_string()),
            ComSrvError::ConnectionError("connection".to_string()),
            ComSrvError::TimeoutError("timeout".to_string()),
            ComSrvError::NetworkError("network".to_string()),
            ComSrvError::UnknownError("unknown".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn test_error_types_comprehensive() {
        // Test various ComSrvError types
        let errors = vec![
            ComSrvError::ConfigError("config error".to_string()),
            ComSrvError::ConnectionError("connection error".to_string()),
            ComSrvError::TimeoutError("timeout error".to_string()),
            ComSrvError::ProtocolError("protocol error".to_string()),
            ComSrvError::NetworkError("network error".to_string()),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[tokio::test]
    async fn test_async_error_handling() {
        // Test error handling in async context
        async fn failing_async_operation() -> Result<String> {
            Err(ComSrvError::TimeoutError("async timeout".to_string()))
        }

        let result = failing_async_operation().await;
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ComSrvError::TimeoutError(_)));
        }
    }
}
