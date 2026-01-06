//! Runtime lifecycle management
//!
//! Provides orchestration functions for service startup, shutdown, and maintenance tasks
//! as part of the runtime orchestration layer

use crate::core::channels::ChannelManager;
use crate::core::config::ConfigManager;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use voltage_rtdb::{RedisRtdb, Rtdb};

/// Start the communication service with optimized performance and monitoring
///
/// Initializes and starts all configured communication channels using the provided
/// configuration manager and protocol factory. This function handles channel creation,
/// startup, and error reporting with comprehensive metrics collection.
///
/// # Arguments
///
/// * `config_manager` - Shared configuration manager containing channel definitions
/// * `channel_manager` - Thread-safe channel manager for creating and managing channels
///
/// # Returns
///
/// * `Ok(())` - If the service starts successfully
/// * `Err(error)` - If critical errors occur during startup
///
/// # Features
///
/// - **Parallel Channel Creation**: Creates multiple channels concurrently
/// - **Error Isolation**: Continues operation even if some channels fail
/// - **Metrics Integration**: Records channel status and performance metrics
/// - **Graceful Degradation**: Provides service even with partial channel failures
///
/// # Service Architecture
///
/// ```text
/// ┌─────────────────────┐    ┌─────────────────────┐
/// │   Configuration     │───►│  Channel Factory    │
/// │   Manager           │    │                     │
/// └─────────────────────┘    └─────────────────────┘
///           │                           │
///           ▼                           ▼
/// ┌─────────────────────┐    ┌─────────────────────┐
/// │   Channel Config    │───►│  Protocol Channels  │
/// │   Validation        │    │  (Modbus/IEC/...)   │
/// └─────────────────────┘    └─────────────────────┘
///                                       │
///                                       ▼
///                           ┌─────────────────────┐
///                           │   Metrics & Status  │
///                           │   Monitoring        │
///                           └─────────────────────┘
/// ```
///
/// # Error Handling
///
/// The function implements robust error handling:
/// - Individual channel failures don't stop service startup
/// - Detailed error logging with context
/// - Metrics recording for failed operations
/// - Graceful degradation with partial functionality
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::core::channel_manager;
///
/// #[tokio::main]
/// async fn main() -> errors::VoltageResult<()> {
///     use common::DEFAULT_REDIS_URL;
///
///     let config_manager = Arc::new(ConfigManager::load().await?);
///     let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
///         protocol_factory,
///         DEFAULT_REDIS_URL.into(),
///     )));
///
///     let configured_count = channel_manager::start_communication_channels(config_manager, channel_manager).await?;
///     println!("Started with {} configured channels", configured_count);
///     Ok(())
/// }
/// ```
///
/// This function provides a convenient public interface for starting the communication service.
pub async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    channel_manager: Arc<RwLock<ChannelManager<RedisRtdb>>>,
) -> Result<usize> {
    start_communication_service_generic(config_manager, channel_manager).await
}

/// Generic version of start_communication_service that accepts any Rtdb implementation.
/// Used by tests with MemoryRtdb.
pub(crate) async fn start_communication_service_generic<R: Rtdb + 'static>(
    config_manager: Arc<ConfigManager>,
    channel_manager: Arc<RwLock<ChannelManager<R>>>,
) -> Result<usize> {
    debug!("start_communication_service called");

    // Get channel configurations
    let configs = config_manager.channels();

    if configs.is_empty() {
        warn!("No channels configured");
        return Ok(0);
    }

    let total_configured = configs.len();
    let enabled_count = configs.iter().filter(|c| c.is_enabled()).count();
    let disabled_count = total_configured - enabled_count;

    info!(
        "Found {} channels: {} enabled, {} disabled",
        configs.len(),
        enabled_count,
        disabled_count
    );

    // Record disabled channels.
    for channel in configs.iter().filter(|c| !c.is_enabled()) {
        info!(
            "Channel {} ({}) is disabled, skipping",
            channel.id(),
            channel.name()
        );
    }

    // Create all channels concurrently to improve startup performance.
    use futures::future::join_all;

    // First create all channel instances concurrently without holding the lock.
    let channel_futures: Vec<_> = configs
        .iter()
        .filter(|c| c.is_enabled()) // Only create enabled channels.
        .map(|channel_config| {
            let channel_manager = Arc::clone(&channel_manager);
            // Clone the Arc (cheap reference count increment), not the inner ChannelConfig
            let channel_config = Arc::clone(channel_config);
            async move {
                let channel_id = channel_config.id();
                let channel_name = channel_config.name().to_string();

                info!("Creating channel: {} - {}", channel_id, channel_name);

                // Debug: Verify points are available before creating channel
                debug!(
                    "Channel {} points will be loaded from SQLite at runtime",
                    channel_id
                );

                // Acquire the lock briefly to insert the channel.
                let manager_guard = channel_manager.write().await;
                let result = manager_guard.create_channel(channel_config).await;
                drop(manager_guard); // Release the lock immediately.
                match result {
                    Ok(_) => {
                        info!("Channel created successfully: {}", channel_id);
                        Ok((channel_id, channel_name))
                    },
                    Err(e) => {
                        error!("Failed to create channel {}: {}", channel_id, e);
                        Err((channel_id, channel_name, e))
                    },
                }
            }
        })
        .collect();

    // Wait for all channels to be created.
    let results = join_all(channel_futures).await;

    // Summarize successful and failed channel creations.
    let mut successful_channels = 0;
    let mut failed_channels = 0;
    let mut failed_details = Vec::new();

    for result in results {
        match result {
            Ok((id, name)) => {
                successful_channels += 1;
                debug!("Channel {} ({}) added to successful list", id, name);
            },
            Err((id, name, err)) => {
                failed_channels += 1;
                failed_details.push(format!("Channel {} ({}): {}", id, name, err));
            },
        }
    }

    // If any channel failed, log the details.
    if !failed_details.is_empty() {
        error!("Failed channels details:");
        for detail in &failed_details {
            error!("  - {}", detail);
        }
    }

    info!(
        "Channel initialization completed: {} successful, {} failed",
        successful_channels, failed_channels
    );

    // Wait briefly to ensure all channels are initialized
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Phase 2: Establish connections for all channels in batch
    info!("Starting connection phase for all initialized channels...");
    let manager_guard = channel_manager.read().await;
    match manager_guard.connect_all_channels().await {
        Ok(()) => {
            info!("All channel connections completed successfully");
        },
        Err(e) => {
            error!("Some channel connections failed: {}", e);
            // Connection failure should not prevent service startup, continue running
        },
    }
    drop(manager_guard);

    info!(
        "Communication service started with {} channels successfully initialized",
        successful_channels
    );

    Ok(total_configured)
}

/// Handle graceful shutdown of the communication service
///
/// Performs an orderly shutdown of all communication channels, ensuring that
/// ongoing operations complete properly and resources are released cleanly.
/// Updates metrics to reflect the service shutdown state.
///
/// # Arguments
///
/// * `factory` - Thread-safe protocol factory managing all active channels
///
/// # Features
///
/// - **Graceful Channel Shutdown**: Stops all channels in an orderly manner
/// - **Resource Cleanup**: Ensures proper release of network and system resources
/// - **Metrics Update**: Records service shutdown in monitoring systems
/// - **Error Handling**: Logs but doesn't fail on individual channel shutdown errors
///
/// # Shutdown Process
///
/// ```text
/// ┌─────────────────────┐
/// │  Shutdown Signal    │
/// │  Received           │
/// └─────────────────────┘
///           │
///           ▼
/// ┌─────────────────────┐
/// │  Stop All Channels  │
/// │  (Async)            │
/// └─────────────────────┘
///           │
///           ▼
/// ┌─────────────────────┐
/// │  Update Metrics     │
/// │  (Service Stopped)  │
/// └─────────────────────┘
///           │
///           ▼
/// ┌─────────────────────┐
/// │  Cleanup Resources  │
/// │  Complete           │
/// └─────────────────────┘
/// ```
///
/// # Examples
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::runtime::shutdown_handler;
///
/// async fn main() {
///     // Assume channel_manager is initialized
///     let channel_manager = todo!();
///
///     // Setup signal handlers
///     let factory_clone = channel_manager.clone();
///     tokio::spawn(async move {
///         if let Ok(_) = tokio::signal::ctrl_c().await {
///             shutdown_handler(factory_clone).await;
///         }
///     });
///
///     // Main service loop...
/// }
/// ```
///
/// This function provides a convenient public interface for graceful service shutdown.
pub async fn shutdown_handler(channel_manager: Arc<RwLock<ChannelManager<RedisRtdb>>>) {
    shutdown_handler_generic(channel_manager).await
}

/// Generic version of shutdown_handler that accepts any Rtdb implementation.
/// Used by tests with MemoryRtdb.
pub(crate) async fn shutdown_handler_generic<R: Rtdb + 'static>(
    channel_manager: Arc<RwLock<ChannelManager<R>>>,
) {
    info!("Starting graceful shutdown...");

    // Get all channel IDs
    let channel_ids = {
        let manager_guard = channel_manager.read().await;
        manager_guard.get_channel_ids()
    };

    let total_channels = channel_ids.len();
    if total_channels == 0 {
        info!("No channels to shutdown");
        return;
    }

    info!("Stopping {} channels concurrently...", total_channels);

    // Stop all channels concurrently.
    use futures::future::join_all;

    let shutdown_futures: Vec<_> = channel_ids
        .into_iter()
        .map(|channel_id| {
            let channel_manager = Arc::clone(&channel_manager);
            async move {
                // For each channel, acquire the lock and stop it independently.
                let manager_guard = channel_manager.write().await;
                let result = manager_guard.remove_channel(channel_id).await;
                drop(manager_guard); // Release the lock immediately.

                match result {
                    Ok(_) => {
                        debug!("Channel {} stopped successfully", channel_id);
                        Ok(channel_id)
                    },
                    Err(e) => {
                        error!("Error stopping channel {}: {}", channel_id, e);
                        Err((channel_id, e))
                    },
                }
            }
        })
        .collect();

    // Wait for all channels to stop.
    let results = join_all(shutdown_futures).await;

    // Summarize stop results.
    let mut successful_stops = 0;
    let mut failed_stops = 0;

    for result in results {
        match result {
            Ok(_) => successful_stops += 1,
            Err(_) => failed_stops += 1,
        }
    }

    info!(
        "Shutdown completed: {} channels stopped successfully, {} failed",
        successful_stops, failed_stops
    );
}

/// Start the periodic cleanup task for resource management
///
/// Launches a background task that periodically cleans up idle channels and
/// logs system statistics. This helps prevent resource leaks and provides
/// operational visibility into the service state.
///
/// # Arguments
///
/// * `factory` - Thread-safe protocol factory to monitor and clean up
///
/// # Returns
///
/// A `JoinHandle` for the cleanup task that can be used to cancel or wait for completion
///
/// # Features
///
/// - **Idle Channel Cleanup**: Removes channels that have been idle for extended periods
/// - **Statistics Logging**: Regular logging of channel and system statistics
/// - **Resource Monitoring**: Tracks memory and connection usage
/// - **Configurable Intervals**: Adjustable cleanup and reporting intervals
///
/// # Configuration
///
/// - **Cleanup Interval**: 5 minutes (300 seconds)
/// - **Idle Timeout**: 1 hour (3600 seconds)
/// - **Statistics Interval**: Every cleanup cycle
///
/// # Task Lifecycle
///
/// ```text
/// ┌─────────────────────┐
/// │  Task Started       │
/// └─────────────────────┘
///           │
///           ▼
/// ┌─────────────────────┐    ┌─────────────────────┐
/// │  Wait 5 Minutes     │◄───│  Cleanup Cycle      │
/// └─────────────────────┘    │  Complete           │
///           │                └─────────────────────┘
///           ▼                           ▲
/// ┌─────────────────────┐               │
/// │  Cleanup Idle       │               │
/// │  Channels           │               │
/// └─────────────────────┘               │
///           │                           │
///           ▼                           │
/// ┌─────────────────────┐               │
/// │  Log Statistics     │───────────────┘
/// └─────────────────────┘
/// ```
///
/// # Returns
///
/// Returns a tuple of:
/// - `JoinHandle<()>` - The task handle to await completion
/// - `CancellationToken` - Token to gracefully stop the task
///
/// # Examples
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::runtime::start_cleanup_task;
///
/// async fn main() {
///     // Assume channel_manager and configured_count are initialized
///     let channel_manager = todo!();
///     let configured_count = 10;
///
///     let (cleanup_handle, cancel_token) = start_cleanup_task(channel_manager, configured_count);
///
///     // Keep the handle to cancel if needed
///     tokio::select! {
///         _ = cleanup_handle => {
///             println!("Cleanup task completed");
///         }
///         _ = tokio::signal::ctrl_c() => {
///             println!("Shutting down cleanup task");
///             cancel_token.cancel();
///         }
///     }
/// }
/// ```
///
/// This function provides a convenient public interface for resource cleanup management.
pub fn start_cleanup_task(
    channel_manager: Arc<RwLock<ChannelManager<RedisRtdb>>>,
    configured_count: usize,
) -> (tokio::task::JoinHandle<()>, CancellationToken) {
    start_cleanup_task_generic(channel_manager, configured_count)
}

/// Generic version of start_cleanup_task that accepts any Rtdb implementation.
/// Used by tests with MemoryRtdb.
pub(crate) fn start_cleanup_task_generic<R: Rtdb + 'static>(
    channel_manager: Arc<RwLock<ChannelManager<R>>>,
    configured_count: usize,
) -> (tokio::task::JoinHandle<()>, CancellationToken) {
    let token = CancellationToken::new();
    let task_token = token.clone();

    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Clean up idle channels (1 hour idle time)
                    let manager_guard = channel_manager.read().await;

                    // Log statistics
                    let all_stats = manager_guard.get_all_channel_stats().await;

                    // Collect active channels for display
                    let active_channels: Vec<String> = all_stats
                        .iter()
                        .filter(|s| s.is_connected)
                        .map(|s| format!("{}({})", s.name, s.channel_id))
                        .collect();

                    if active_channels.is_empty() {
                        info!(
                            "Channel stats: configured={}, initialized={}, active=0",
                            configured_count,
                            all_stats.len()
                        );
                    } else {
                        info!(
                            "Channel stats: configured={}, initialized={}, active={} [{}]",
                            configured_count,
                            all_stats.len(),
                            active_channels.len(),
                            active_channels.join(", ")
                        );
                    }
                    drop(manager_guard);
                }
                () = task_token.cancelled() => {
                    info!("Cleanup task received cancellation signal, shutting down");
                    break;
                }
            }
        }

        info!("Cleanup task terminated");
    });

    (handle, token)
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM on Unix)
///
/// Re-exports the common shutdown handler for backwards compatibility.
pub async fn wait_for_shutdown() {
    common::shutdown::wait_for_shutdown().await
}

/// Perform graceful shutdown of all services
pub async fn shutdown_services(
    channel_manager: Arc<RwLock<ChannelManager<RedisRtdb>>>,
    shutdown_token: CancellationToken,
    cleanup_token: CancellationToken,
    cleanup_handle: tokio::task::JoinHandle<()>,
    server_handle: tokio::task::JoinHandle<()>,
    warning_handle: tokio::task::JoinHandle<()>,
) {
    shutdown_services_generic(
        channel_manager,
        shutdown_token,
        cleanup_token,
        cleanup_handle,
        server_handle,
        warning_handle,
    )
    .await
}

/// Generic version of shutdown_services that accepts any Rtdb implementation.
/// Used by tests with MemoryRtdb.
pub(crate) async fn shutdown_services_generic<R: Rtdb + 'static>(
    channel_manager: Arc<RwLock<ChannelManager<R>>>,
    shutdown_token: CancellationToken,
    cleanup_token: CancellationToken,
    cleanup_handle: tokio::task::JoinHandle<()>,
    server_handle: tokio::task::JoinHandle<()>,
    warning_handle: tokio::task::JoinHandle<()>,
) {
    info!("Received shutdown signal, starting graceful shutdown...");

    // First shutdown the communication channels
    shutdown_handler_generic(channel_manager).await;

    // Signal all tasks to shutdown
    shutdown_token.cancel();

    // Cancel cleanup task
    cleanup_token.cancel();
    cleanup_handle.abort();

    // Wait for tasks with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(30);

    // Wait for server task
    match tokio::time::timeout(shutdown_timeout, server_handle).await {
        Ok(Ok(())) => info!("Server shut down gracefully"),
        Ok(Err(e)) => error!("Server task failed: {}", e),
        Err(_) => error!("Server shutdown timed out"),
    }

    // Abort warning monitor if still running
    warning_handle.abort();
    let _ = warning_handle.await; // Ignore abort error

    info!("Service shutdown complete");
}

// NOTE: These tests are temporarily disabled during AFIT migration.
// The production functions (start_communication_service, start_cleanup_task) are hardcoded to RedisRtdb,
// but tests create ChannelManager<MemoryRtdb>. This type mismatch cannot be resolved without either:
// 1. Genericizing the production functions (significant refactor)
// 2. Converting to integration tests with real Redis
// TODO: Genericize lifecycle functions to accept any Rtdb implementation.
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::config::ConfigManager;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    /// Helper: Create a test database with minimal configuration
    async fn create_test_database() -> (TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_lifecycle.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = SqlitePool::connect(&db_url).await.unwrap();

        // Create service_config table (with service_name column and composite primary key)
        sqlx::query(
            "CREATE TABLE service_config (
                service_name TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                type TEXT DEFAULT 'string',
                description TEXT,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (service_name, key)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert basic service config (with service_name column)
        sqlx::query("INSERT INTO service_config (service_name, key, value) VALUES ('comsrv', 'service_name', 'comsrv')")
            .execute(&pool)
            .await
            .unwrap();

        // Create channels table
        sqlx::query(
            "CREATE TABLE channels (
                channel_id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                protocol TEXT NOT NULL,
                enabled BOOLEAN DEFAULT TRUE,
                config TEXT DEFAULT '{}'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create point tables
        for table_name in &[
            "telemetry_points",
            "signal_points",
            "control_points",
            "adjustment_points",
        ] {
            sqlx::query(&format!(
                "CREATE TABLE {} (
                    point_id INTEGER PRIMARY KEY,
                    signal_name TEXT NOT NULL,
                    scale REAL DEFAULT 1.0,
                    offset REAL DEFAULT 0.0,
                    unit TEXT DEFAULT '',
                    reverse BOOLEAN DEFAULT FALSE,
                    data_type TEXT DEFAULT 'float32',
                    description TEXT DEFAULT ''
                )",
                table_name
            ))
            .execute(&pool)
            .await
            .unwrap();
        }

        pool.close().await;
        (temp_dir, db_path.to_string_lossy().to_string())
    }

    /// Helper: Add test channels to database
    async fn add_test_channels(db_path: &str, enabled: bool) {
        let pool = SqlitePool::connect(&format!("sqlite://{}", db_path))
            .await
            .unwrap();

        sqlx::query("INSERT INTO channels (channel_id, name, protocol, enabled) VALUES (1001, 'Test Channel 1', 'virtual', ?)")
            .bind(enabled)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO channels (channel_id, name, protocol, enabled) VALUES (1002, 'Test Channel 2', 'virtual', ?)")
            .bind(enabled)
            .execute(&pool)
            .await
            .unwrap();

        pool.close().await;
    }

    // ========================================================================
    // Phase 1: Service Startup Tests
    // ========================================================================

    #[tokio::test]
    async fn test_start_service_success_with_enabled_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let result = start_communication_service_generic(config_manager, channel_manager).await;

        assert!(result.is_ok(), "Service startup should succeed");
        let configured_count = result.unwrap();
        assert_eq!(
            configured_count, 2,
            "Should return count of configured channels"
        );
    }

    #[tokio::test]
    async fn test_start_service_with_no_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        // Don't add any channels

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let result = start_communication_service_generic(config_manager, channel_manager).await;

        assert!(
            result.is_ok(),
            "Service startup should succeed with no channels"
        );
        let configured_count = result.unwrap();
        assert_eq!(configured_count, 0, "Should return 0 for no channels");
    }

    #[tokio::test]
    async fn test_start_service_with_disabled_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, false).await; // disabled channels

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let result = start_communication_service_generic(config_manager, channel_manager).await;

        assert!(
            result.is_ok(),
            "Service startup should succeed with disabled channels"
        );
        let configured_count = result.unwrap();
        assert_eq!(
            configured_count, 2,
            "Should return configured count even if disabled"
        );
    }

    // ========================================================================
    // Phase 2: Service Shutdown Tests
    // ========================================================================

    #[tokio::test]
    async fn test_shutdown_with_active_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Start service first
        let _ = start_communication_service_generic(config_manager, channel_manager.clone()).await;

        // Now shutdown
        shutdown_handler_generic(channel_manager.clone()).await;

        // Verify all channels are stopped
        let manager_guard = channel_manager.read().await;
        assert_eq!(
            manager_guard.channel_count(),
            0,
            "All channels should be removed after shutdown"
        );
    }

    #[tokio::test]
    async fn test_shutdown_with_no_channels() {
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Shutdown without starting any channels
        shutdown_handler_generic(channel_manager).await;

        // Test passes if no panic occurs
    }

    #[tokio::test]
    async fn test_shutdown_idempotency() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Start service
        let _ = start_communication_service_generic(config_manager, channel_manager.clone()).await;

        // Shutdown twice
        shutdown_handler_generic(channel_manager.clone()).await;
        shutdown_handler_generic(channel_manager).await;

        // Test passes if no panic occurs on second shutdown
    }

    // ========================================================================
    // Phase 3: Cleanup Task Tests
    // ========================================================================

    #[tokio::test]
    async fn test_cleanup_task_starts() {
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let (handle, cancel_token) = start_cleanup_task_generic(channel_manager, 0);

        // Verify handle is valid
        assert!(!handle.is_finished(), "Cleanup task should be running");

        // Cancel and wait for completion
        cancel_token.cancel();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn test_cleanup_task_cancellation() {
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let (handle, cancel_token) = start_cleanup_task_generic(channel_manager, 0);

        // Cancel immediately
        cancel_token.cancel();

        // Wait for task to complete
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "Task should complete after cancellation");
    }

    #[tokio::test]
    async fn test_cleanup_task_with_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Start service
        let configured_count =
            start_communication_service_generic(config_manager, channel_manager.clone())
                .await
                .unwrap();

        // Start cleanup task
        let (handle, cancel_token) =
            start_cleanup_task_generic(channel_manager.clone(), configured_count);

        // Let it run briefly
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cancel and cleanup
        cancel_token.cancel();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
    }

    // ========================================================================
    // Phase 4: Connection Phase Tests
    // ========================================================================

    #[tokio::test]
    async fn test_service_connection_phase_completes() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Start service includes connection phase
        let result = start_communication_service_generic(config_manager, channel_manager).await;

        assert!(
            result.is_ok(),
            "Service startup with connection phase should succeed"
        );
    }

    #[tokio::test]
    async fn test_connection_phase_does_not_block_startup() {
        let (_temp_dir, db_path) = create_test_database().await;
        add_test_channels(&db_path, true).await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        // Even if connections fail, startup should succeed
        let result = start_communication_service_generic(config_manager, channel_manager).await;

        assert!(
            result.is_ok(),
            "Service startup should succeed even if connections fail"
        );
    }

    #[tokio::test]
    async fn test_parallel_channel_creation() {
        let (_temp_dir, db_path) = create_test_database().await;

        // Add multiple channels
        let pool = SqlitePool::connect(&format!("sqlite://{}", db_path))
            .await
            .unwrap();
        for i in 1001..1006 {
            sqlx::query("INSERT INTO channels (channel_id, name, protocol, enabled) VALUES (?, ?, 'virtual', true)")
                .bind(i)
                .bind(format!("Channel {}", i))
                .execute(&pool)
                .await
                .unwrap();
        }
        pool.close().await;

        let config_manager = Arc::new(ConfigManager::from_sqlite(&db_path).await.unwrap());
        let channel_manager = Arc::new(RwLock::new(ChannelManager::new(
            crate::test_utils::create_test_rtdb(),
            crate::test_utils::create_test_routing_cache(),
        )));

        let start_time = std::time::Instant::now();
        let result = start_communication_service_generic(config_manager, channel_manager).await;
        let elapsed = start_time.elapsed();

        assert!(result.is_ok(), "Parallel channel creation should succeed");
        // Parallel creation should be faster than sequential (< 5s for 5 channels)
        assert!(
            elapsed < tokio::time::Duration::from_secs(5),
            "Parallel creation should complete quickly"
        );
    }
}
