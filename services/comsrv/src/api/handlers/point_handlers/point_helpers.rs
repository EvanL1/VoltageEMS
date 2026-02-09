#![allow(clippy::disallowed_methods)]

//! Validation and reload utility functions for point handlers

use crate::api::routes::AppState;
use crate::dto::AppError;
use voltage_rtdb::Rtdb;

// ----------------------------------------------------------------------------
// Validation Helper Functions
// ----------------------------------------------------------------------------

/// Validate that a channel exists
pub(super) async fn validate_channel_exists(
    pool: &sqlx::SqlitePool,
    channel_id: u32,
) -> Result<(), AppError> {
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT channel_id FROM channels WHERE channel_id = ?")
            .bind(channel_id as i64)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                tracing::error!("Ch check: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    if exists.is_none() {
        return Err(AppError::not_found(format!(
            "Channel {} not found",
            channel_id
        )));
    }

    Ok(())
}

/// Validate that a point ID is unique within a channel
pub(super) async fn validate_point_uniqueness(
    pool: &sqlx::SqlitePool,
    channel_id: u32,
    table: &str,
    point_id: u32,
) -> Result<(), AppError> {
    let query = format!(
        "SELECT point_id FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );

    let exists: Option<(i64,)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Point uniqueness check: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    if exists.is_some() {
        return Err(AppError::conflict(format!(
            "Point {} already exists in channel {}",
            point_id, channel_id
        )));
    }

    Ok(())
}

// ============================================================================
// Auto-Reload Helper Functions
// ============================================================================

/// Trigger channel reload if auto_reload is enabled
///
/// This function is called after successful CRUD operations on points to ensure
/// changes take effect immediately. It runs asynchronously to avoid blocking the API response.
///
/// ## Parameters
/// - `channel_id`: The channel to reload
/// - `state`: Application state
/// - `auto_reload`: Whether to perform reload (from query parameter)
///
/// ## Behavior
/// - If `auto_reload=true`: Loads config from SQLite and hot-reloads the channel in background
/// - If `auto_reload=false`: No action (user must manually call `/api/channels/reload`)
///
/// ## Implementation
/// Uses `tokio::spawn` for async execution to avoid blocking the API response.
pub async fn trigger_channel_reload_if_needed<R: Rtdb + 'static>(
    channel_id: u32,
    state: &AppState<R>,
    auto_reload: bool,
) {
    if !auto_reload {
        tracing::debug!(
            "Auto-reload disabled for channel {}, skipping hot reload",
            channel_id
        );
        return;
    }

    tracing::debug!("Ch{} auto-reload", channel_id);

    let state_clone = state.clone();
    // Fire-and-forget: channel reload is best-effort and non-critical
    // The task will complete on its own; errors are logged but not propagated
    drop(tokio::spawn(async move {
        if let Err(e) = perform_channel_reload(channel_id, &state_clone).await {
            tracing::error!("Ch{} reload: {}", channel_id, e);
        } else {
            tracing::debug!("Ch{} reloaded", channel_id);
        }
    }));
}

/// Perform channel reload (load config from SQLite and hot-reload)
///
/// This is an internal helper function that performs the actual reload logic.
async fn perform_channel_reload<R: Rtdb>(
    channel_id: u32,
    state: &AppState<R>,
) -> anyhow::Result<()> {
    use crate::core::channels::channel_manager::ChannelManager;

    // 1. Load channel configuration from SQLite
    let config = ChannelManager::<voltage_rtdb::RedisRtdb>::load_channel_from_db(
        &state.sqlite_pool,
        channel_id,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to load channel config: {}", e))?;

    // 2. Remove old channel
    // Direct access without RwLock (lock-free)
    let manager = &state.channel_manager;
    if let Err(e) = manager.remove_channel(channel_id).await {
        tracing::warn!("Ch{} remove: {}", channel_id, e);
    }

    // 3. Create new channel with updated config
    let entry = manager
        .create_channel(std::sync::Arc::new(config))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create channel: {}", e))?;

    // 4. Connect in background (non-blocking, fire-and-forget)
    // Connection is best-effort; errors are logged but don't fail the reload
    drop(tokio::spawn(async move {
        // Use ChannelEntry's direct connect method
        match entry.connect().await {
            Ok(_) => tracing::debug!("Ch{} connected", channel_id),
            Err(e) => tracing::warn!("Ch{} connect: {}", channel_id, e),
        }
    }));

    Ok(())
}
