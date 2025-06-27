// Service implementation shared between library and binary
// Contains concrete functions for starting, shutting down, and cleaning up the
// communication service.

use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::config::ConfigManager;
use crate::core::protocols::common::ProtocolFactory;

use crate::utils::error::Result;

/// Convert new ChannelConfig to legacy types::ChannelConfig for compatibility
fn convert_channel_config(config: &crate::core::config::config_manager::ChannelConfig) -> crate::core::config::types::ChannelConfig {
    let protocol_type = match config.protocol.as_str() {
        "modbus_tcp" => crate::core::config::types::ProtocolType::ModbusTcp,
        "modbus_rtu" => crate::core::config::types::ProtocolType::ModbusRtu,
        "can" => crate::core::config::types::ProtocolType::Can,
        "iec104" => crate::core::config::types::ProtocolType::Iec104,
        "virtual" => crate::core::config::types::ProtocolType::Virtual,
        _ => crate::core::config::types::ProtocolType::Virtual, // default fallback
    };

    // Convert parameters to Generic HashMap (simplified)
    let param_map: std::collections::HashMap<String, serde_yaml::Value> = config.parameters.iter()
        .map(|(k, v)| (k.clone(), serde_yaml::Value::String(format!("{:?}", v))))
        .collect();

    crate::core::config::types::ChannelConfig {
        id: config.id,
        name: config.name.clone(),
        description: config.description.clone(),
        protocol: protocol_type,
        parameters: crate::core::config::types::ChannelParameters::Generic(param_map),
    }
}

/// Start the communication service with optimized performance and monitoring.
pub async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<()> {
    // Check if Redis is enabled and initialize Redis storage
    let redis_config = config_manager.get_redis_config();
    if redis_config.enabled {
        info!("Redis is enabled, initializing Redis storage...");
        
        match crate::core::storage::redis_storage::RedisStore::from_config(redis_config).await {
            Ok(Some(redis_store)) => {
                info!("Redis storage initialized successfully");
                
                // Enable Redis storage for the protocol factory
                {
                    let mut factory_guard = factory.write().await;
                    if let Err(e) = factory_guard.enable_redis_storage(redis_store.clone()) {
                        warn!("Failed to enable Redis storage for ProtocolFactory: {}", e);
                    } else {
                        info!("Redis storage enabled for ProtocolFactory");
                    }
                }
                
                // Note: ConfigManager Redis storage would be enabled separately if needed
                // This would require making config_manager mutable, which we avoid here
                // to maintain the current API compatibility
            }
            Ok(None) => {
                info!("Redis storage is disabled in configuration");
            }
            Err(e) => {
                warn!("Failed to initialize Redis storage: {}. Continuing with in-memory storage only.", e);
            }
        }
    } else {
        info!("Redis storage is disabled, using in-memory storage only");
    }

    // Get channel configurations
    let configs = config_manager.get_channels().clone();

    if configs.is_empty() {
        warn!("No channels configured");
        return Ok(());
    }

    info!("Creating {} channels...", configs.len());

    // Create channels with improved error handling and metrics
    let mut successful_channels = 0;
    let mut failed_channels = 0;

    for channel_config in configs {
        info!(
            "Creating channel: {} - {}",
            channel_config.id, channel_config.name
        );

        let factory_guard = factory.write().await;
        let converted_config = convert_channel_config(&channel_config);
        match factory_guard
            .create_channel_with_config_manager(converted_config, Some(&*config_manager))
            .await
        {
            Ok(_) => {
                info!("Channel created successfully: {}", channel_config.id);
                successful_channels += 1;
            }
            Err(e) => {
                error!("Failed to create channel {}: {}", channel_config.id, e);
                failed_channels += 1;

                // Continue with other channels instead of failing completely
                continue;
            }
        }
        drop(factory_guard); // Release the lock for each iteration
    }

    info!(
        "Channel creation completed: {} successful, {} failed",
        successful_channels, failed_channels
    );

    // Start all channels with improved performance
    let factory_guard = factory.read().await;
    if let Err(e) = factory_guard.start_all_channels().await {
        error!("Failed to start some channels: {}", e);
        // Log but don't fail - some channels might have started successfully
    }

    let stats = factory_guard.get_channel_stats().await;
    info!(
        "Communication service started with {} channels (Protocol distribution: {:?}){}",
        stats.total_channels, 
        stats.protocol_counts,
        if factory_guard.is_redis_enabled() { " [Redis storage enabled]" } else { " [Memory storage only]" }
    );
    
    drop(factory_guard);
    
    // Sync channel metadata if Redis is enabled
    let factory_guard = factory.read().await;
    if factory_guard.is_redis_enabled() {
        drop(factory_guard);
        let mut factory_guard = factory.write().await;
        if let Err(e) = factory_guard.sync_channel_metadata().await {
            warn!("Failed to sync channel metadata to Redis: {}", e);
        } else {
            info!("Channel metadata synchronized to Redis");
        }
    }

    Ok(())
}

/// Handle graceful shutdown of the communication service.
pub async fn shutdown_handler(factory: Arc<RwLock<ProtocolFactory>>) {
    info!("Starting graceful shutdown...");

    let factory_guard = factory.read().await;
    if let Err(e) = factory_guard.stop_all_channels().await {
        error!("Error during channel shutdown: {}", e);
    }
    drop(factory_guard);

    info!("All channels stopped");
}

/// Start the periodic cleanup task for resource management.
pub fn start_cleanup_task(factory: Arc<RwLock<ProtocolFactory>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            interval.tick().await;

            // Clean up idle channels (1 hour idle time)
            let factory_guard = factory.read().await;
            factory_guard
                .cleanup_channels(std::time::Duration::from_secs(3600))
                .await;

            // Log statistics
            let stats = factory_guard.get_channel_stats().await;
            info!(
                "Channel stats: total={}, running={}",
                stats.total_channels, stats.running_channels
            );
            drop(factory_guard);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::timeout;

    /// Create a test configuration manager with minimal valid configuration
    fn create_test_config_manager() -> Arc<ConfigManager> {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let test_config = r#"
version: "1.0"
service:
  name: "test_service"
  description: "Test Communication Service"
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
        Arc::new(ConfigManager::from_file(&config_path).unwrap())
    }

    /// Create a test configuration manager with sample channels
    fn create_test_config_manager_with_channels() -> Arc<ConfigManager> {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let test_config = r#"
version: "1.0"
service:
  name: "test_service_with_channels"
  description: "Test Communication Service with Channels"
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
channels:
  - id: 1
    name: "Test Virtual Channel"
    description: "Test virtual channel for unit testing"
    protocol: "Virtual"
    parameters:
      interval: 1000
      data_points: 10
"#;

        std::fs::write(&config_path, test_config).unwrap();
        Arc::new(ConfigManager::from_file(&config_path).unwrap())
    }

    #[tokio::test]
    async fn test_start_communication_service_empty_channels() {
        let config_manager = create_test_config_manager();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        let result = start_communication_service(config_manager, factory.clone()).await;

        assert!(result.is_ok(), "Should succeed with empty channels");

        // Verify no channels were created
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 0);
    }

    #[tokio::test]
    async fn test_start_communication_service_with_virtual_channels() {
        let config_manager = create_test_config_manager_with_channels();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        let result = start_communication_service(config_manager, factory.clone()).await;

        assert!(result.is_ok(), "Should succeed with virtual channels");

        // Verify channel was created
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_shutdown_handler() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // This should not panic or error, even with empty factory
        shutdown_handler(factory.clone()).await;

        // Add a channel and test shutdown
        let config_manager = create_test_config_manager_with_channels();
        let _result = start_communication_service(config_manager, factory.clone()).await;

        // Shutdown should work with channels
        shutdown_handler(factory).await;
    }

    #[tokio::test]
    async fn test_start_cleanup_task() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Start cleanup task
        let cleanup_handle = start_cleanup_task(factory.clone());

        // Let it run for a short time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cancel the task
        cleanup_handle.abort();

        // Verify task was running (doesn't panic when aborted)
        let result = cleanup_handle.await;
        assert!(result.is_err()); // Should be cancelled
    }

    #[tokio::test]
    async fn test_cleanup_task_with_channels() {
        let config_manager = create_test_config_manager_with_channels();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Create channels first
        let _result = start_communication_service(config_manager, factory.clone()).await;

        // Start cleanup task
        let cleanup_handle = start_cleanup_task(factory.clone());

        // Let it run for a short time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cancel the task
        cleanup_handle.abort();

        // Verify the factory still has channels
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_service_lifecycle() {
        let config_manager = create_test_config_manager_with_channels();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Test complete lifecycle
        // 1. Start service
        let start_result = start_communication_service(config_manager, factory.clone()).await;
        assert!(start_result.is_ok());

        // 2. Verify channels created
        {
            let factory_guard = factory.read().await;
            assert_eq!(factory_guard.channel_count(), 1);
        }

        // 3. Start cleanup task
        let cleanup_handle = start_cleanup_task(factory.clone());

        // 4. Let it run briefly
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 5. Shutdown service
        shutdown_handler(factory.clone()).await;

        // 6. Stop cleanup task
        cleanup_handle.abort();
        let _cleanup_result = cleanup_handle.await;

        // 7. Verify final state
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 1); // Channels still exist but stopped
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let config_manager = create_test_config_manager_with_channels();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Start multiple operations concurrently
        let factory1 = factory.clone();
        let config1 = config_manager.clone();
        let start_handle =
            tokio::spawn(async move { start_communication_service(config1, factory1).await });

        let factory2 = factory.clone();
        let cleanup_handle = start_cleanup_task(factory2);

        // Wait for start to complete
        let start_result = timeout(Duration::from_secs(5), start_handle).await;
        assert!(start_result.is_ok());
        assert!(start_result.unwrap().is_ok());

        // Let cleanup run briefly
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Shutdown concurrently
        let factory3 = factory.clone();
        let shutdown_handle = tokio::spawn(async move { shutdown_handler(factory3).await });

        // Wait for shutdown
        let shutdown_result = timeout(Duration::from_secs(5), shutdown_handle).await;
        assert!(shutdown_result.is_ok());

        // Stop cleanup
        cleanup_handle.abort();
    }

    #[tokio::test]
    async fn test_error_handling_in_service_start() {
        let config_manager = create_test_config_manager();
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Should handle empty configuration gracefully
        let result = start_communication_service(config_manager, factory).await;
        assert!(result.is_ok(), "Empty config should not cause failure");
    }

    #[tokio::test]
    async fn test_cleanup_task_creation() {
        let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

        // Should be able to create cleanup task without panic
        let cleanup_handle = start_cleanup_task(factory);

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Immediately abort to clean up
        cleanup_handle.abort();

        // Wait for abort to complete
        let _ = cleanup_handle.await;
    }
}
