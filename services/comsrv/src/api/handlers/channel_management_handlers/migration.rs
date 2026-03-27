//! Channel ID migration logic
//!
//! Handles migrating a channel to a new ID, including all related tables.

use super::helpers::*;
use crate::api::routes::AppState;
use crate::core::channels::ChannelManager;
use crate::core::config::ChannelCore;
use crate::dto::{AppError, SuccessResponse};
use axum::response::Json;
use std::sync::Arc;
use voltage_rtdb::Rtdb;

/// Migrate channel ID in database (all related tables)
///
/// Updates the channel_id in all related tables within a single transaction.
async fn migrate_channel_id_in_db(
    old_id: u32,
    new_id: u32,
    pool: &sqlx::SqlitePool,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to begin transaction: {}", e)))?;

    // Disable foreign key constraints temporarily
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to disable FK: {}", e)))?;

    // Update all related tables (order matters for potential future FK constraints)
    for table in &RELATED_TABLES {
        let query = format!("UPDATE {} SET channel_id = ? WHERE channel_id = ?", table);
        let result = sqlx::query(&query)
            .bind(new_id as i64)
            .bind(old_id as i64)
            .execute(&mut *tx)
            .await;

        match result {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    tracing::debug!(
                        "Ch{} -> Ch{}: {} rows in {}",
                        old_id,
                        new_id,
                        r.rows_affected(),
                        table
                    );
                }
            },
            Err(e) => {
                // Table might not exist, log and continue
                tracing::debug!("Table {} update skipped: {}", table, e);
            },
        }
    }

    // Update the main channels table
    sqlx::query("UPDATE channels SET channel_id = ? WHERE channel_id = ?")
        .bind(new_id as i64)
        .bind(old_id as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to update channels: {}", e)))?;

    // Re-enable foreign key constraints
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to enable FK: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to commit transaction: {}", e)))?;

    tracing::info!("Ch{} -> Ch{}: database migration complete", old_id, new_id);
    Ok(())
}

/// Change channel ID with full migration
///
/// 1. Validates new_id doesn't exist
/// 2. Stops old channel (runtime)
/// 3. Migrates database (all related tables in transaction)
/// 4. Clears Redis keys for old channel
/// 5. Reloads routing cache
/// 6. Creates and connects new channel
pub(super) async fn change_channel_id<R: Rtdb + 'static>(
    old_id: u32,
    new_id: u32,
    req: crate::dto::ChannelConfigUpdateRequest,
    state: &AppState<R>,
) -> Result<Json<SuccessResponse<crate::dto::ChannelCrudResult>>, AppError> {
    use crate::core::config::ChannelConfig;
    use std::time::Duration;

    // 1-2. Validate new_id doesn't exist in runtime or database
    let manager = &state.channel_manager;
    check_channel_id_available(new_id, &state.sqlite_pool, manager).await?;

    // 3. Load current configuration from database
    let (current_name, current_protocol, enabled, current_config_str) =
        load_channel_from_db(&state.sqlite_pool, old_id).await?;

    // 4. Parse current config
    let (current_description, current_parameters, current_logging) =
        parse_channel_config(old_id, current_config_str)?;

    // Check if any additional updates are requested (besides channel_id)
    let has_other_updates = req.name.is_some()
        || req.protocol.is_some()
        || req.description.is_some()
        || req.parameters.is_some()
        || req.logging.is_some();

    // Apply requested updates (if any)
    let name = req.name.unwrap_or(current_name);
    let protocol = req.protocol.unwrap_or(current_protocol);
    let description = req.description.or(current_description);
    let parameters = if let Some(new_params) = req.parameters {
        let mut merged = current_parameters;
        for (k, v) in new_params {
            merged.insert(k, v);
        }
        merged
    } else {
        current_parameters
    };
    let logging = req.logging.unwrap_or(current_logging);

    // 5. Stop old channel (runtime)
    if manager.get_channel(old_id).is_some() {
        if let Err(e) = manager.remove_channel(old_id).await {
            tracing::warn!("Ch{} remove failed: {}", old_id, e);
        } else {
            tracing::debug!("Ch{} stopped for ID migration", old_id);
        }
    }

    // 6. Migrate database (all related tables in transaction)
    migrate_channel_id_in_db(old_id, new_id, &state.sqlite_pool).await?;

    // 7. Update channel name/protocol/config if changed
    if has_other_updates {
        let config_json = build_channel_config_json(description.as_ref(), &parameters, &logging)
            .map_err(|e| AppError::internal_error(format!("Failed to build config JSON: {}", e)))?;

        sqlx::query("UPDATE channels SET name = ?, protocol = ?, config = ? WHERE channel_id = ?")
            .bind(&name)
            .bind(&protocol)
            .bind(&config_json)
            .bind(new_id as i64)
            .execute(&state.sqlite_pool)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to update channel: {}", e)))?;
    }

    // 8. Redis keys for old channel will be overwritten when new channel starts
    tracing::debug!(
        "Ch{} Redis keys will be overwritten by Ch{}",
        old_id,
        new_id
    );

    // 9. Reload routing cache (in-memory only, no SHM rebuild)
    if let Err(e) = ChannelManager::<voltage_rtdb::RedisRtdb>::reload_routing_cache(
        &state.sqlite_pool,
        &manager.routing_cache,
    )
    .await
    {
        tracing::warn!("Routing cache reload failed: {}", e);
    }
    // 9b. Rebuild SHM for channel structure change (new channel points)
    if let Some(handle) = manager.shm_handle() {
        match voltage_rtdb_shm::ChannelPointCounts::load_from_db(&state.sqlite_pool).await {
            Ok(cp) => {
                if let Err(e) = handle.rebuild(&cp) {
                    tracing::warn!("SHM rebuild after migration failed: {}", e);
                }
            },
            Err(e) => tracing::warn!("Failed to load channel points for SHM rebuild: {}", e),
        }
    }

    // 10. Create and connect new channel (if enabled)
    let runtime_status = if enabled {
        let new_config = ChannelConfig {
            core: ChannelCore {
                id: new_id,
                name: name.clone(),
                description: description.clone(),
                protocol: protocol.clone(),
                enabled,
            },
            parameters: parameters.clone(),
            logging: logging.clone(),
        };

        match manager.create_channel(Arc::new(new_config)).await {
            Ok(entry) => {
                // Wait for connection with timeout (sync instead of fire-and-forget)
                match tokio::time::timeout(Duration::from_secs(5), entry.connect()).await {
                    Ok(Ok(_)) => {
                        tracing::info!("Ch{} connected after ID migration", new_id);
                        "connected".to_string()
                    },
                    Ok(Err(e)) => {
                        tracing::warn!("Ch{} connect failed: {}", new_id, e);
                        "connection_failed".to_string()
                    },
                    Err(_) => {
                        tracing::debug!("Ch{} connect timeout, continuing in background", new_id);
                        "connecting".to_string()
                    },
                }
            },
            Err(e) => {
                tracing::error!("Ch{} create failed: {}", new_id, e);
                "error".to_string()
            },
        }
    } else {
        "stopped".to_string()
    };

    let result = crate::dto::ChannelCrudResult {
        core: ChannelCore {
            id: new_id,
            name,
            description,
            protocol,
            enabled,
        },
        runtime_status,
        message: Some(format!("Channel ID changed from {} to {}", old_id, new_id)),
    };

    tracing::info!(
        "Ch{} -> Ch{}: migration complete. \
         Run `monarch services refresh modsrv` to restore M2C dispatch.",
        old_id,
        new_id
    );
    Ok(Json(SuccessResponse::new(result)))
}
