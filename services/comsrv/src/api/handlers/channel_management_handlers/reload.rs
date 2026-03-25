//! Channel and routing reload handlers

use super::helpers::*;
use crate::api::routes::AppState;
use crate::core::channels::ChannelManager;
use crate::core::config::{ChannelCore, ChannelLoggingConfig};
use crate::dto::{AppError, SuccessResponse};
use axum::{extract::State, response::Json};
use std::sync::Arc;
use voltage_rtdb::Rtdb;

/// Reload all channels from SQLite configuration
///
/// @route POST /api/channels/reload
/// @side-effects Synchronizes runtime with SQLite configuration
#[utoipa::path(
    post,
    path = "/api/channels/reload",
    responses(
        (status = 200, description = "Configuration reloaded", body = crate::dto::ReloadConfigResult)
    ),
    tag = "comsrv"
)]
pub async fn reload_configuration_handler<R: Rtdb>(
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::ReloadConfigResult>>, AppError> {
    use crate::core::config::ChannelConfig;

    tracing::debug!("Reloading config");

    // 1. Load all channels from SQLite
    let db_channels: Vec<(i64, String, String, bool)> =
        sqlx::query_as("SELECT channel_id, name, protocol, enabled FROM channels")
            .fetch_all(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Load channels: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    // 2. Get runtime channel IDs
    let runtime_ids: std::collections::HashSet<u32> = {
        // Direct access without RwLock (lock-free)
        let manager = &state.channel_manager;
        manager.get_channel_ids().into_iter().collect()
    };

    let db_ids: std::collections::HashSet<u32> =
        db_channels.iter().map(|(id, _, _, _)| *id as u32).collect();

    // 3. Determine changes
    let to_add: Vec<u32> = db_ids.difference(&runtime_ids).copied().collect();
    let to_remove: Vec<u32> = runtime_ids.difference(&db_ids).copied().collect();
    let to_update: Vec<u32> = db_ids.intersection(&runtime_ids).copied().collect();

    let mut channels_added = Vec::new();
    let mut channels_updated = Vec::new();
    let mut channels_removed = Vec::new();
    let mut errors = Vec::new();

    // 4. Remove channels that are no longer in SQLite
    {
        // Direct access without RwLock (lock-free)
        let manager = &state.channel_manager;
        for id in &to_remove {
            match manager.remove_channel(*id).await {
                Ok(_) => {
                    channels_removed.push(*id);
                    tracing::debug!("Ch{} removed (not in DB)", id);
                },
                Err(e) => {
                    errors.push(format!("Failed to remove channel {}: {}", id, e));
                },
            }
        }
    }

    // 5-6. Add new and update existing channels
    let combined = to_add
        .iter()
        .map(|id| (*id, false))
        .chain(to_update.iter().map(|id| (*id, true)));

    let manager = &state.channel_manager;
    for (id, is_update) in combined {
        let label = if is_update { "update" } else { "add" };
        let Some((_, name, protocol, enabled)) =
            db_channels.iter().find(|(cid, _, _, _)| *cid as u32 == id)
        else {
            continue;
        };

        let config_str: Option<String> =
            sqlx::query_scalar("SELECT config FROM channels WHERE channel_id = ?")
                .bind(id as i64)
                .fetch_optional(&state.sqlite_pool)
                .await
                .map_err(|e| {
                    tracing::error!("Load channel {} config: {}", id, e);
                    AppError::internal_error("Database operation failed")
                })?
                .flatten();

        let (description, parameters, _logging) = parse_channel_config(id, config_str)?;

        if is_update {
            if let Err(e) = manager.remove_channel(id).await {
                tracing::debug!("Ch{} not in runtime: {}", id, e);
            }
        }

        let result_vec = if is_update {
            &mut channels_updated
        } else {
            &mut channels_added
        };

        if *enabled {
            let channel_config = ChannelConfig {
                core: ChannelCore {
                    id,
                    name: name.clone(),
                    description,
                    protocol: protocol.clone(),
                    enabled: *enabled,
                },
                parameters,
                logging: ChannelLoggingConfig::default(),
            };

            match manager.create_channel(Arc::new(channel_config)).await {
                Ok(entry) => {
                    if let Err(e) = entry.connect().await {
                        tracing::warn!("Ch{} connect: {}", id, e);
                    }
                    result_vec.push(id);
                    tracing::debug!("Ch{} {}d", id, label);
                },
                Err(e) => {
                    errors.push(format!("Failed to {} channel {}: {}", label, id, e));
                },
            }
        } else {
            result_vec.push(id);
            tracing::debug!("Ch{} {}d (disabled)", id, label);
        }
    }

    let result = crate::dto::ReloadConfigResult {
        total_channels: db_channels.len(),
        channels_added,
        channels_updated,
        channels_removed,
        errors,
    };

    tracing::info!(
        "Reload: +{} ~{} -{} err:{}",
        result.channels_added.len(),
        result.channels_updated.len(),
        result.channels_removed.len(),
        result.errors.len()
    );

    Ok(Json(SuccessResponse::new(result)))
}

/// Reload routing cache from SQLite configuration
///
/// @route POST /api/routing/reload
/// @side-effects Updates in-memory routing cache with latest data from SQLite
#[utoipa::path(
    post,
    path = "/api/routing/reload",
    responses(
        (status = 200, description = "Routing cache reloaded successfully", body = crate::dto::RoutingReloadResult),
        (status = 500, description = "Internal server error")
    ),
    tag = "comsrv"
)]
pub async fn reload_routing_handler<R: Rtdb>(
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::RoutingReloadResult>>, AppError> {
    tracing::debug!("Reloading routing");

    let start_time = std::time::Instant::now();
    let mut errors = Vec::new();

    // Get routing_cache reference from channel_manager
    let (c2m_count, m2c_count, c2c_count) = {
        // Direct access without RwLock (lock-free)
        let manager = &state.channel_manager;

        // Call the public reload_routing_cache method
        match ChannelManager::<voltage_rtdb::RedisRtdb>::reload_routing_cache(
            &state.sqlite_pool,
            &manager.routing_cache,
        )
        .await
        {
            Ok(counts) => {
                // SHM layout is based on channel points, not routing.
                // No SHM rebuild needed for routing changes.
                counts
            },
            Err(e) => {
                let error_msg = format!("Failed to reload routing cache: {}", e);
                tracing::error!("{}", error_msg);
                errors.push(error_msg);
                (0, 0, 0)
            },
        }
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let result = crate::dto::RoutingReloadResult {
        c2m_count,
        m2c_count,
        c2c_count,
        errors,
        duration_ms,
    };

    if result.errors.is_empty() {
        tracing::info!(
            "Routing: {} C2M, {} M2C, {} C2C ({}ms)",
            c2m_count,
            m2c_count,
            c2c_count,
            duration_ms
        );
    } else {
        tracing::warn!(
            "Routing: {} errors ({}ms)",
            result.errors.len(),
            duration_ms
        );
    }

    Ok(Json(SuccessResponse::new(result)))
}
