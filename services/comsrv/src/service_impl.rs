// Service implementation shared between library and binary
// Contains concrete functions for starting, shutting down, and cleaning up the
// communication service.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::core::config::ConfigManager;
use crate::core::protocols::common::ProtocolFactory;
use crate::core::metrics::get_metrics;
use crate::utils::error::Result;

/// Start the communication service with optimized performance and monitoring.
pub async fn start_communication_service(
    config_manager: Arc<ConfigManager>,
    factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<()> {
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
        info!("Creating channel: {} - {}", channel_config.id, channel_config.name);

        let factory_guard = factory.write().await;
        match factory_guard.create_channel(channel_config.clone()) {
            Ok(_) => {
                info!("Channel created successfully: {}", channel_config.id);
                successful_channels += 1;

                // Record metrics if available
                if let Some(metrics) = get_metrics() {
                    metrics.update_channel_status(
                        &channel_config.id.to_string(),
                        false, // Not connected yet
                        &config_manager.get_service_name(),
                    );
                }
            }
            Err(e) => {
                error!("Failed to create channel {}: {}", channel_config.id, e);
                failed_channels += 1;

                // Record error metrics if available
                if let Some(metrics) = get_metrics() {
                    metrics.record_channel_error(
                        &channel_config.id.to_string(),
                        "creation_failed",
                        &config_manager.get_service_name(),
                    );
                }

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

    let stats = factory_guard.get_channel_stats();
    info!(
        "Communication service started with {} channels (Protocol distribution: {:?})",
        stats.total_channels, stats.protocol_counts
    );
    drop(factory_guard);

    // Update service metrics
    if let Some(metrics) = get_metrics() {
        metrics.update_service_status(true);
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

    // Update service metrics
    if let Some(metrics) = get_metrics() {
        metrics.update_service_status(false);
    }

    info!("All channels stopped");
}

/// Start the periodic cleanup task for resource management.
pub fn start_cleanup_task(
    factory: Arc<RwLock<ProtocolFactory>>, 
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            interval.tick().await;

            // Clean up idle channels (1 hour idle time)
            let factory_guard = factory.read().await;
            factory_guard.cleanup_channels(std::time::Duration::from_secs(3600)).await;

            // Log statistics
            let stats = factory_guard.get_channel_stats();
            info!(
                "Channel stats: total={}, running={}",
                stats.total_channels, stats.running_channels
            );
            drop(factory_guard);
        }
    })
}

