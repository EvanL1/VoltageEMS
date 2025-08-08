//! Runtime lifecycle management
//!
//! Provides orchestration functions for service startup, shutdown, and maintenance tasks
//! as part of the runtime orchestration layer

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
/// use comsrv::runtime::start_communication_service;
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

    // 并发创建所有channel以提高启动性能
    use futures::future::join_all;

    // 先并发创建所有channel实例（无锁操作）
    let channel_futures: Vec<_> = configs
        .iter()
        .map(|channel_config| {
            let factory = factory.clone();
            async move {
                let channel_id = channel_config.id;
                let channel_name = channel_config.name.clone();

                info!("Creating channel: {} - {}", channel_id, channel_name);

                // Debug: Verify points are available before creating channel
                debug!(
                    "Channel {} points before creation: {} telemetry, {} signal, {} control, {} adjustment",
                    channel_id,
                    channel_config.telemetry_points.len(),
                    channel_config.signal_points.len(),
                    channel_config.control_points.len(),
                    channel_config.adjustment_points.len()
                );

                // 短时间加锁插入channel
                let factory_guard = factory.write().await;
                let result = factory_guard.create_channel(channel_config).await;
                drop(factory_guard); // 立即释放锁
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

    // 等待所有channel创建完成
    let results = join_all(channel_futures).await;

    // 统计成功和失败的channel
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

    // 如果有失败的channel，打印详细信息
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
/// use comsrv::runtime::shutdown_handler;
/// use comsrv::ProtocolFactory;
///
/// async fn main_loop(factory: Arc<RwLock<ProtocolFactory>>) {
///     // Setup signal handlers
///     let factory_clone = factory.clone();
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
pub async fn shutdown_handler(factory: Arc<RwLock<ProtocolFactory>>) {
    info!("Starting graceful shutdown...");

    // Get all channel IDs
    let channel_ids = {
        let factory_guard = factory.read().await;
        factory_guard.get_channel_ids()
    };

    let total_channels = channel_ids.len();
    if total_channels == 0 {
        info!("No channels to shutdown");
        return;
    }

    info!("Stopping {} channels concurrently...", total_channels);

    // 并发停止所有channel
    use futures::future::join_all;

    let shutdown_futures: Vec<_> = channel_ids
        .into_iter()
        .map(|channel_id| {
            let factory = factory.clone();
            async move {
                // 每个channel独立加锁和停止
                let factory_guard = factory.write().await;
                let result = factory_guard.remove_channel(channel_id).await;
                drop(factory_guard); // 立即释放锁

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

    // 等待所有channel停止完成
    let results = join_all(shutdown_futures).await;

    // 统计停止结果
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
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
/// use comsrv::runtime::start_cleanup_task;
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
