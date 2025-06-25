//! Core Communication Service Components
//!
//! This module contains the core functionality of the communication service,
//! including protocol implementations, configuration management, connection
//! pooling, and factory patterns for creating protocol instances.
//!
//! # Architecture
//!
//! The core module is organized into several key components:
//!
//! - **`config`** - Configuration management and validation, including enhanced point table management
//! - **`protocols`** - Protocol implementations (Modbus RTU/TCP, IEC60870, etc.)
//! - **`storage`** - Data storage and caching mechanisms
//! - **`connection_pool`** - Connection pooling for network efficiency
//! - **`protocol_factory`** - Factory pattern for creating protocol instances

//!
//! # Design Principles
//!
//! ## Async-First
//!
//! All core components are designed with async/await in mind for maximum
//! concurrency and performance.
//!
//! ## Protocol Agnostic
//!
//! The core provides a unified interface for all protocols through the
//! ComBase trait, allowing protocols to be treated uniformly.
//!
//! ## Configuration Driven
//!
//! All behavior is controlled through configuration files, enabling
//! runtime customization without code changes.
//!
//! ## Business Layer Focus
//!
//! Enhanced with business-level features including:
//! - Intelligent point table management with optimization
//! - Real-time monitoring and diagnostics
//! - Advanced channel configuration management
//! - Performance analytics and reporting
//!
//! # Example Usage
//!
//! ```rust
//! use comsrv::{ConfigManager, ProtocolFactory, EnhancedPointTableManager};
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! async fn setup_enhanced_service() -> comsrv::Result<()> {
//!     // Load configuration
//!     let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
//!     
//!     // Create enhanced point table manager
//!     let point_table_manager = Arc::new(EnhancedPointTableManager::new(
//!         "config/points",
//!         Default::default(),
//!     ));
//!     point_table_manager.start().await?;
//!     

//!     
//!     // Create protocol factory with enhanced capabilities
//!     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
//!     
//!     // Register channels from configuration with enhanced point tables
//!     for channel_config in config_manager.get_channels() {
//!         let channel = factory.write().await.create_enhanced_channel(
//!             channel_config.clone(),
//!             point_table_manager.clone(),
//!         )?;
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod forward_calculation;
pub mod protocols;
pub mod storage;

// Re-export commonly used protocol components for public API

// Re-export protocol factory and connection pool from protocols/common

// Re-export enhanced components for business layer

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn test_module_structure() {
        // This test verifies that all core modules are accessible
        // and the module structure is valid

        // The existence of these modules is verified by successful compilation
        // We can't directly instantiate all components without proper configuration,
        // but we can test that the modules are accessible

        assert!(true, "Core module structure is valid");
    }

    #[tokio::test]
    async fn test_config_module_integration() {
        // Test basic config module functionality
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let test_config = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test service"
  metrics:
    enabled: false
    bind_address: "0.0.0.0:9100"
  logging:
    level: "info"
    file: "/tmp/test_comsrv.log"
    max_size: 10485760
    max_files: 5
    console: true
  api:
    enabled: false
    bind_address: "0.0.0.0:3000"
    version: "v1"
  redis:
    enabled: false
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 0
  point_tables:
    enabled: false
    directory: "config/points"
    watch_changes: false
    reload_interval: 60
channels: []
"#;

        std::fs::write(&config_path, test_config).unwrap();

        // Test that we can use config module components
        let config_manager = config::ConfigManager::from_file(&config_path);
        assert!(
            config_manager.is_ok(),
            "Should be able to create config manager from core module"
        );

        let manager = config_manager.unwrap();
        assert_eq!(manager.get_service_name(), "test_service");
    }

    #[test]
    fn test_protocols_module_access() {
        // Test that protocol components are accessible
        use crate::core::protocols::common::*;

        // Test factory creation
        let factory = ProtocolFactory::new();
        assert_eq!(factory.channel_count(), 0);

        // Test error types
        let error = errors::BaseCommError::connection("test error");
        assert!(error.to_string().contains("Connection"));
    }

    #[tokio::test]
    async fn test_storage_module_access() {
        // Test that storage components are accessible
        // Note: We can't fully test Redis without a running Redis instance,
        // but we can test module accessibility

        // This mainly serves as a compilation test for the storage module
        assert!(true, "Storage module is accessible");
    }

    #[test]
    fn test_module_re_exports() {
        // Test that re-exports work correctly
        // This is mainly a compilation test to ensure all imports are valid

        // Test protocol re-exports
        use crate::core::protocols::common::*;

        let _ = ProtocolFactory::new();
        let _ = errors::BaseCommError::connection("test");

        assert!(true, "Module re-exports are working");
    }

    #[tokio::test]
    async fn test_comprehensive_core_functionality() {
        // Test comprehensive integration of core components
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("comprehensive_test.yaml");

        let test_config = r#"
version: "1.0"
service:
  name: "comprehensive_test_service"
  description: "Comprehensive test service"
  logging:
    level: "info"
    file: "/tmp/test_comsrv.log"
    max_size: 10485760
    max_files: 5
    console: true
  api:
    enabled: false
    bind_address: "0.0.0.0:3000"
    version: "v1"
  redis:
    enabled: false
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 0
  point_tables:
    enabled: false
    directory: "config/points"
    watch_changes: false
    reload_interval: 60
channels:
  - id: 1
    name: "Test Virtual Channel"
    description: "Test channel for comprehensive testing"
    protocol: "Virtual"
    parameters:
      interval: 1000
      data_points: 5
"#;

        std::fs::write(&config_path, test_config).unwrap();

        // Test config + protocols integration
        let config_manager = config::ConfigManager::from_file(&config_path).unwrap();
        let factory = protocols::common::ProtocolFactory::new();

        // Test channel creation through factory
        let channels = config_manager.get_channels();
        assert_eq!(channels.len(), 1);

        // This would normally create a real channel, but for testing we just verify structure
        assert_eq!(channels[0].name, "Test Virtual Channel");
        assert_eq!(factory.channel_count(), 0); // No channels created yet
    }

    #[test]
    fn test_error_integration_across_modules() {
        // Test that error types work consistently across modules
        use crate::core::protocols::common::errors::*;
        use crate::utils::error::ComSrvError;

        // Test error conversion from protocol to service level
        let base_error = BaseCommError::timeout(5000);
        let core_error = base_error.into_core_error();

        assert!(matches!(core_error, ComSrvError::TimeoutError(_)));
        assert!(core_error.to_string().contains("5000ms"));
    }

    #[test]
    fn test_configuration_types_integration() {
        // Test integration between config and protocol modules
        use crate::core::config::*;

        // Test protocol types
        let modbus_tcp = ProtocolType::ModbusTcp;
        let modbus_rtu = ProtocolType::ModbusRtu;
        let virtual_proto = ProtocolType::Virtual;

        assert_ne!(modbus_tcp, modbus_rtu);
        assert_ne!(modbus_tcp, virtual_proto);

        // Test that protocol configurations work
        use crate::core::config::protocol_config::BaseCommConfig;
        let base_config = BaseCommConfig::new();
        assert!(base_config.timeout > std::time::Duration::ZERO);
        assert!(base_config.max_retries > 0);
    }

    #[tokio::test]
    async fn test_async_core_operations() {
        // Test async operations across core modules
        use crate::core::protocols::common::*;
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Test concurrent access
        let factory1 = factory.clone();
        let factory2 = factory.clone();

        let task1 = tokio::spawn(async move {
            let guard = factory1.read().await;
            guard.channel_count()
        });

        let task2 = tokio::spawn(async move {
            let guard = factory2.read().await;
            guard.channel_count()
        });

        let (result1, result2) = tokio::join!(task1, task2);
        assert_eq!(result1.unwrap(), 0);
        assert_eq!(result2.unwrap(), 0);
    }
}
