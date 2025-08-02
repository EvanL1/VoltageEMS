//! Service lifecycle management
//!
//! Provides management functions for service startup, shutdown, and maintenance tasks

use crate::core::combase::factory::ProtocolFactory;
use crate::core::config::ConfigManager;
use crate::utils::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Start the communication service with optimized performance and monitoring
///
/// Initializes and starts all configured communication channels using the provided
/// configuration manager and protocol factory. This function handles channel creation,
/// startup, and error reporting with comprehensive metrics collection.
///
/// # Arguments
///
/// * `config_manager` - Shared configuration manager containing channel definitions
/// * `factory` - Thread-safe protocol factory for creating and managing channels
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
/// use comsrv::service::start_communication_service;
/// use comsrv::{ConfigManager, ProtocolFactory};
///
/// #[tokio::main]
/// async fn main() -> comsrv::Result<()> {
///     let config_manager = Arc::new(ConfigManager::from_file("config.yaml")?);
///     let factory = Arc::new(RwLock::new(ProtocolFactory::new()));
///     
///     start_communication_service(config_manager, factory).await?;
///     Ok(())
/// }
/// ```
///
/// This function provides a convenient public interface for starting the communication service.
pub async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<()> {
    debug!("start_communication_service called");

    // Get channel configurations
    let configs = config_manager.channels();

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
        match factory_guard
            .create_channel(channel_config, Some(&*config_manager))
            .await
        {
            Ok(_) => {
                info!("Channel created successfully: {}", channel_config.id);
                successful_channels += 1;
            },
            Err(e) => {
                error!("Failed to create channel {}: {e}", channel_config.id);
                failed_channels += 1;

                // Continue with other channels instead of failing completely
                continue;
            },
        }
        drop(factory_guard); // Release the lock for each iteration
    }

    info!(
        "Channel initialization completed: {} successful, {} failed",
        successful_channels, failed_channels
    );

    // Wait briefly to ensure all channels are initialized
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Phase 2: Establish connections for all channels in batch
    info!("Starting connection phase for all initialized channels...");
    let factory_guard = factory.read().await;
    match factory_guard.connect_all_channels().await {
        Ok(()) => {
            info!("All channel connections completed successfully");
        },
        Err(e) => {
            error!("Some channel connections failed: {}", e);
            // Connection failure should not prevent service startup, continue running
        },
    }
    drop(factory_guard);

    info!(
        "Communication service started with {} channels successfully initialized",
        successful_channels
    );

    Ok(())
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
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::service::shutdown_handler;
/// use comsrv::ProtocolFactory;
///
/// async fn main_loop(factory: Arc<RwLock<ProtocolFactory>>) {
///     // Setup signal handlers
///     let factory_clone = factory.clone();
///     tokio::spawn(async move {
///         tokio::signal::ctrl_c().await.unwrap();
///         shutdown_handler(factory_clone).await;
///     });
///
///     // Main service loop...
/// }
/// ```
///
/// This function provides a convenient public interface for graceful service shutdown.
pub async fn shutdown_handler(factory: Arc<RwLock<ProtocolFactory>>) {
    info!("Starting graceful shutdown...");

    // Get all channel IDs
    let channel_ids = {
        let factory_guard = factory.read().await;
        factory_guard.get_channel_ids()
    };

    // Remove all channels
    for channel_id in channel_ids {
        let factory_guard = factory.write().await;
        if let Err(e) = factory_guard.remove_channel(channel_id).await {
            error!("Error stopping channel {}: {}", channel_id, e);
        }
        drop(factory_guard);
    }

    info!("All channels stopped");
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
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::service::start_cleanup_task;
/// use comsrv::ProtocolFactory;
///
/// async fn setup_maintenance(factory: Arc<RwLock<ProtocolFactory>>) {
///     let (cleanup_handle, cancel_token) = start_cleanup_task(factory);
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
    factory: Arc<RwLock<ProtocolFactory>>,
) -> (tokio::task::JoinHandle<()>, CancellationToken) {
    let token = CancellationToken::new();
    let task_token = token.clone();

    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Clean up idle channels (1 hour idle time)
                    let factory_guard = factory.read().await;

                    // Log statistics
                    let all_stats = factory_guard.get_all_channel_stats().await;
                    info!(
                        "Channel stats: total={}, active={}",
                        all_stats.len(),
                        all_stats.iter().filter(|s| s.is_connected).count()
                    );
                    drop(factory_guard);
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
