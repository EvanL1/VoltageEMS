//! Communication Service Library
//!
//! Industrial communication service providing unified interface for various protocols

// Module declarations
pub mod api;
pub mod cleanup_provider;
pub mod common;
pub mod core;
pub mod dto;
pub mod protocols;
pub mod runtime;
pub mod storage;
pub mod utils;

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
pub mod test_utils;

// Re-export commonly used types
pub use common::{telemetry_type_to_redis, PluginPointUpdate};
pub use utils::error::ComSrvError;

// Re-export core functionality
pub use core::bootstrap::ServiceArgs;
pub use core::combase::ChannelManager;
pub use core::config::ConfigManager;

use tokio_util::sync::CancellationToken;
use tracing::error;

// ============================================================================
// Crate-wide macros
// ============================================================================

/// Get maximum allowed Modbus TCP MBAP length (Unit ID + PDU)
/// Expands to `1 + pdu::MAX_PDU_SIZE` so it stays consistent with spec.
#[macro_export]
macro_rules! modbus_tcp_max_length {
    () => {
        1 + $crate::protocols::modbus::pdu::MAX_PDU_SIZE
    };
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM on Unix)
pub async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let term_signal = match signal(SignalKind::terminate()) {
            Ok(sig) => Some(sig),
            Err(e) => {
                error!(
                    "Failed to install SIGTERM handler: {}. Service will only respond to Ctrl+C",
                    e
                );
                None
            },
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(mut sig) = term_signal {
                    sig.recv().await;
                } else {
                    // If SIGTERM handler failed, wait forever (only Ctrl+C will work)
                    std::future::pending::<()>().await
                }
            } => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Perform graceful shutdown of all services
pub async fn shutdown_services(
    channel_manager: std::sync::Arc<
        tokio::sync::RwLock<crate::core::combase::channel_manager::ChannelManager>,
    >,
    shutdown_token: CancellationToken,
    cleanup_token: CancellationToken,
    cleanup_handle: tokio::task::JoinHandle<()>,
    server_handle: tokio::task::JoinHandle<()>,
    warning_handle: tokio::task::JoinHandle<()>,
) {
    use tracing::info;

    info!("Received shutdown signal, starting graceful shutdown...");

    // First shutdown the communication channels
    crate::runtime::shutdown_handler(channel_manager).await;

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
