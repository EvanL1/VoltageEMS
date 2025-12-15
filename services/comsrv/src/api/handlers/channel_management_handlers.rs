//! Channel management CRUD handlers
//!
//! This module contains handlers for:
//! - Creating new channels with hot startup
//! - Updating channel configurations with hot reload
//! - Enabling/disabling channels
//! - Deleting channels
//! - Reloading all channels from database

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::api::routes::AppState;
use crate::core::config::{ChannelCore, ChannelLoggingConfig};
use crate::dto::{AppError, ParameterChangeType, SuccessResponse};
use axum::{
    extract::{Path, State},
    response::Json,
};
use std::sync::Arc;

/// Parse channel config JSON into description, parameters, and logging
fn parse_channel_config(
    config_str: Option<String>,
) -> (
    Option<String>,
    std::collections::HashMap<String, serde_json::Value>,
    ChannelLoggingConfig,
) {
    let config_obj = config_str
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let description = config_obj
        .get("description")
        .and_then(|d| d.as_str().map(|s| s.to_string()));

    let parameters = config_obj
        .get("parameters")
        .and_then(|p| p.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let logging = config_obj
        .get("logging")
        .and_then(|l| serde_json::from_value(l.clone()).ok())
        .unwrap_or_default();

    (description, parameters, logging)
}

/// Build channel config JSON from description, parameters, and logging
fn build_channel_config_json(
    description: Option<&String>,
    parameters: &std::collections::HashMap<String, serde_json::Value>,
    logging: &ChannelLoggingConfig,
) -> Result<String, serde_json::Error> {
    let mut config_obj = serde_json::Map::new();

    if let Some(desc) = description {
        config_obj.insert(
            "description".to_string(),
            serde_json::Value::String(desc.clone()),
        );
    }

    let params_obj: serde_json::Map<String, serde_json::Value> = parameters
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    config_obj.insert(
        "parameters".to_string(),
        serde_json::Value::Object(params_obj),
    );

    let logging_json = serde_json::to_value(logging)?;
    config_obj.insert("logging".to_string(), logging_json);

    serde_json::to_string(&config_obj)
}

/// Analyze parameter changes to determine if reload is needed
///
/// Returns the highest severity change type:
/// - MetadataOnly: name, description - no reload needed
/// - NonCritical: timeout, retry - may need reload
/// - Critical: host, port, slave_id - must reload
fn analyze_parameter_changes(
    old_params: &std::collections::HashMap<String, serde_json::Value>,
    new_params: &std::collections::HashMap<String, serde_json::Value>,
    name_changed: bool,
    description_changed: bool,
    protocol_changed: bool,
) -> ParameterChangeType {
    use ParameterChangeType::*;

    // Protocol change is always critical
    if protocol_changed {
        return Critical;
    }

    // Check if only metadata changed
    if !name_changed && !description_changed && old_params == new_params {
        return MetadataOnly;
    }

    // Define critical parameters (connection-related)
    let critical_params: Vec<&str> = vec![
        "host",
        "ip",
        "address",
        "server",
        "port",
        "slave_id",
        "device_id",
        "unit_id",
        "node_id",
        "baud_rate",
        "data_bits",
        "stop_bits",
        "parity",
        "serial_port",
        "device",
        "tty",
    ];

    // Define non-critical parameters (performance tuning)
    let non_critical_params: Vec<&str> = vec![
        "timeout",
        "timeout_ms",
        "connect_timeout",
        "retry",
        "max_retries",
        "retry_count",
        "poll_interval",
        "poll_rate",
        "scan_rate",
        "keepalive",
        "heartbeat",
    ];

    // Check for critical parameter changes
    for key in critical_params.iter() {
        if old_params.get(*key) != new_params.get(*key) {
            tracing::debug!("Critical change: {}", key);
            return Critical;
        }
    }

    // Check for non-critical parameter changes
    for key in non_critical_params.iter() {
        if old_params.get(*key) != new_params.get(*key) {
            tracing::debug!("Param change: {}", key);
            return NonCritical;
        }
    }

    // Check for any other parameter changes (treat as non-critical)
    let all_keys: std::collections::HashSet<_> =
        old_params.keys().chain(new_params.keys()).collect();

    for key in all_keys {
        if old_params.get(key.as_str()) != new_params.get(key.as_str()) {
            tracing::debug!("Unknown param: {}", key);
            return NonCritical;
        }
    }

    // Only metadata changed
    MetadataOnly
}

/// Perform hot reload for a running channel (async, non-blocking)
///
/// Removes the old channel, creates a new one with updated config.
/// Connection is attempted in background (non-blocking).
/// Returns Ok("reloaded") immediately after channel creation.
async fn perform_hot_reload(
    id: u32,
    state: &AppState,
    new_config: crate::core::config::ChannelConfig,
) -> Result<String, String> {
    let manager = state.channel_manager.write().await;

    // 1. Remove old channel (allow failure)
    if let Err(e) = manager.remove_channel(id).await {
        tracing::warn!("Remove Ch{}: {}", id, e);
    }

    // 2. Create new channel
    let channel_arc = manager
        .create_channel(Arc::new(new_config))
        .await
        .map_err(|e| format!("Failed to create channel: {}", e))?;

    drop(manager);

    // 3. Async connection (don't wait)
    tokio::spawn(async move {
        let mut channel = channel_arc.write().await;
        match channel.connect().await {
            Ok(_) => tracing::debug!("Ch{} connected", id),
            Err(e) => tracing::warn!("Ch{} connect: {}", id, e),
        }
    });

    tracing::debug!("Ch{} reloaded", id);
    Ok("reloaded".to_string())
}

/// Create a new channel with hot startup
///
/// @route POST /api/channels
/// @input State(state): AppState - Application state with manager and SQLite
/// @input Json(req): ChannelCreateRequest - Channel configuration
/// @output Json<ApiResponse<ChannelCrudResult>> - Creation result
/// @status 200 - Channel created and started successfully
/// @status 400 - Invalid request or validation error
/// @status 500 - Database or runtime error
/// @side-effects Creates channel in SQLite and starts it in runtime
#[utoipa::path(
    post,
    path = "/api/channels",
    request_body(
        content = crate::dto::ChannelCreateRequest,
        description = "Channel creation request with protocol-specific parameters",
        examples(
            ("Modbus TCP - Basic" = (
                summary = "Modbus TCP channel (basic parameters)",
                value = json!({
                    "name": "PV Inverter 01",
                    "description": "Primary PV inverter communication",
                    "protocol": "modbus_tcp",
                    "enabled": true,
                    "parameters": {
                        "host": "192.168.1.100",
                        "port": 502
                    }
                })
            )),
            ("Modbus TCP - Full" = (
                summary = "Modbus TCP channel (all parameters)",
                value = json!({
                    "name": "PV Inverter 02",
                    "description": "Secondary PV inverter with custom settings",
                    "protocol": "modbus_tcp",
                    "enabled": true,
                    "parameters": {
                        "host": "192.168.1.101",
                        "port": 502,
                        "timeout_ms": 5000,
                        "retry_count": 3
                    }
                })
            )),
            ("Modbus RTU - Full" = (
                summary = "Modbus RTU channel (complete configuration)",
                description = "Battery Management System with all RTU parameters",
                value = json!({
                    "name": "Battery Pack RTU",
                    "description": "Battery management system with full parameters",
                    "protocol": "modbus_rtu",
                    "enabled": true,
                    "parameters": {
                        "device": "/dev/ttyUSB0",
                        "baud_rate": 9600,
                        "data_bits": 8,
                        "stop_bits": 1,
                        "parity": "None",
                        "timeout_ms": 1000,
                        "retry_count": 3,
                        "poll_interval_ms": 500
                    }
                })
            )),
            ("DI/DO GPIO" = (
                summary = "DI/DO GPIO channel for digital I/O",
                description = "Digital input/output channel using Linux sysfs GPIO interface (e.g., ECU-1170)",
                value = json!({
                    "name": "ECU1170 GPIO",
                    "description": "Digital I/O for industrial controller",
                    "protocol": "di_do",
                    "enabled": true,
                    "parameters": {
                        "driver": "sysfs",
                        "gpio_base_path": "/sys/class/gpio",
                        "di_poll_interval_ms": 200
                    }
                })
            )),
            ("Virtual - Test" = (
                summary = "Virtual test channel",
                value = json!({
                    "name": "Test Channel",
                    "description": "Virtual channel for testing",
                    "protocol": "virtual",
                    "enabled": true,
                    "parameters": {
                        "update_interval_ms": 1000
                    }
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Channel created successfully", body = crate::dto::ChannelCrudResult),
        (status = 400, description = "Invalid request (validation failed)"),
        (status = 409, description = "Conflict (name or ID already exists)"),
        (status = 500, description = "Internal server error")
    ),
    tag = "comsrv"
)]
pub async fn create_channel_handler(
    State(state): State<AppState>,
    Json(req): Json<crate::dto::ChannelCreateRequest>,
) -> Result<Json<SuccessResponse<crate::dto::ChannelCrudResult>>, AppError> {
    use crate::core::config::ChannelConfig;

    // 1. Check if channel name already exists (enforced uniqueness)
    let existing_name: Option<i64> =
        sqlx::query_scalar("SELECT channel_id FROM channels WHERE name = ?")
            .bind(&req.name)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?;

    if let Some(existing_id) = existing_name {
        return Err(AppError::conflict(format!(
            "Channel name '{}' already exists (ID: {})",
            req.name, existing_id
        )));
    }

    // 2. Determine channel ID (auto-assign or use provided)
    let channel_id = if let Some(id) = req.channel_id {
        // Manual ID specified - validate it doesn't exist
        let manager = state.channel_manager.read().await;
        if manager.get_channel(id).is_some() {
            return Err(AppError::conflict(format!(
                "Channel ID {} already exists in runtime",
                id
            )));
        }
        drop(manager);

        // Check if ID exists in database
        let db_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channels WHERE channel_id = ?)")
                .bind(id as i64)
                .fetch_one(&state.sqlite_pool)
                .await
                .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?;

        if db_exists {
            return Err(AppError::conflict(format!(
                "Channel ID {} already exists in database",
                id
            )));
        }

        tracing::debug!("Manual ID: {}", id);
        id
    } else {
        // Auto-assign ID: find MAX + 1
        let max_id: Option<i64> = sqlx::query_scalar("SELECT MAX(channel_id) FROM channels")
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {}", e)))?;

        let next_id = (max_id.unwrap_or(0) + 1) as u32;
        tracing::debug!("Auto ID: {}", next_id);
        next_id
    };

    tracing::debug!("Creating Ch{}: {} ({})", channel_id, req.name, req.protocol);

    let enabled = req.enabled.unwrap_or(true);

    // 3. Build channel config
    let channel_config = ChannelConfig {
        core: ChannelCore {
            id: channel_id,
            name: req.name.clone(),
            description: req.description.clone(),
            protocol: req.protocol.clone(),
            enabled,
        },
        parameters: req.parameters.clone(),
        logging: ChannelLoggingConfig::default(),
    };

    // Determine runtime status based on enabled flag
    let runtime_status = if enabled {
        // enabled = true: Create runtime channel and connect in background (non-blocking)
        let manager = state.channel_manager.write().await;
        match manager.create_channel(Arc::new(channel_config)).await {
            Ok(channel_arc) => {
                // Spawn background connection to avoid failing API on initial connect error
                let channel_id_for_log = channel_id;
                tokio::spawn(async move {
                    let mut channel = channel_arc.write().await;
                    if let Err(e) = channel.connect().await {
                        tracing::warn!("Ch{} connect: {}", channel_id_for_log, e);
                    } else {
                        tracing::debug!("Ch{} connected", channel_id_for_log);
                    }
                });
                "connecting".to_string()
            },
            Err(e) => {
                tracing::warn!("Create Ch{} runtime: {}", channel_id, e);
                "not_started".to_string()
            },
        }
    } else {
        // enabled = false: Skip runtime creation, just write to database
        tracing::debug!("Ch{} created (disabled)", channel_id);
        "stopped".to_string()
    };

    // 3. Runtime successful, now write to database
    // Build config JSON in structured format
    let logging = ChannelLoggingConfig::default();
    let config_json =
        build_channel_config_json(req.description.as_ref(), &req.parameters, &logging)
            .map_err(|e| AppError::internal_error(format!("Failed to build config JSON: {}", e)))?;

    if let Err(e) = sqlx::query(
        "INSERT INTO channels (channel_id, name, protocol, enabled, config) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(channel_id as i64)
    .bind(&req.name)
    .bind(&req.protocol)
    .bind(enabled)
    .bind(&config_json)
    .execute(&state.sqlite_pool)
    .await
    {
        tracing::error!("Insert Ch{}: {}", channel_id, e);
        // Database write failed, remove the runtime channel
        let manager = state.channel_manager.write().await;
        let _ = manager.remove_channel(channel_id).await;
        return Err(AppError::internal_error(format!("Database error: {}", e)));
    }

    let result = crate::dto::ChannelCrudResult {
        core: ChannelCore {
            id: channel_id,
            name: req.name,
            description: req.description,
            protocol: req.protocol,
            enabled,
        },
        runtime_status,
        message: Some(if enabled {
            "Channel created and started successfully".to_string()
        } else {
            "Channel created in disabled state".to_string()
        }),
    };

    Ok(Json(SuccessResponse::new(result)))
}

/// Update an existing channel configuration
///
/// Updates channel configuration (name, protocol, parameters) without changing enabled state.
/// If the channel is running, it will be hot-reloaded with the new configuration.
/// Use PUT /api/channels/{id}/enabled to change the enabled state.
///
/// @route PUT /api/channels/{id}
/// @input Path(id): u16 - Channel ID to update
/// @input State(state): AppState - Application state
/// @input Json(req): ChannelConfigUpdateRequest - Update parameters
/// @output Json<ApiResponse<ChannelCrudResult>> - Update result
/// @status 200 - Channel updated successfully
/// @status 404 - Channel not found
/// @status 500 - Database or runtime error
/// @side-effects Updates SQLite and hot-reloads if running
#[utoipa::path(
    put,
    path = "/api/channels/{id}",
    params(
        ("id" = u32, Path, description = "Channel identifier")
    ),
    request_body = crate::dto::ChannelConfigUpdateRequest,
    responses(
        (status = 200, description = "Channel updated", body = crate::dto::ChannelCrudResult)
    ),
    tag = "comsrv"
)]
pub async fn update_channel_handler(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(req): Json<crate::dto::ChannelConfigUpdateRequest>,
) -> Result<Json<SuccessResponse<crate::dto::ChannelCrudResult>>, AppError> {
    tracing::debug!("Ch{} updating", id);

    // 1. Check if channel is currently running
    let is_running = {
        let manager = state.channel_manager.read().await;
        manager.get_channel(id).is_some()
    };

    // 2. Load current configuration from database
    let current: Option<(String, String, bool, Option<String>)> =
        sqlx::query_as("SELECT name, protocol, enabled, config FROM channels WHERE channel_id = ?")
            .bind(id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("DB err: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    let Some((current_name, current_protocol, enabled, current_config_str)) = current else {
        return Err(AppError::not_found(format!(
            "Channel {} not found in database",
            id
        )));
    };

    // 3. Begin database transaction
    let mut tx = state
        .sqlite_pool
        .begin()
        .await
        .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;

    // 4. Apply configuration updates to database
    let name = req.name.clone().unwrap_or(current_name.clone());
    let protocol = req.protocol.clone().unwrap_or(current_protocol.clone());

    // Check if new name conflicts with existing channels (excluding current channel)
    if req.name.is_some() {
        let existing_name: Option<i64> = sqlx::query_scalar(
            "SELECT channel_id FROM channels WHERE name = ? AND channel_id != ?",
        )
        .bind(&name)
        .bind(id as i64)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;

        if let Some(existing_id) = existing_name {
            return Err(AppError::conflict(format!(
                "Channel name '{}' already exists (ID: {})",
                name, existing_id
            )));
        }

        sqlx::query("UPDATE channels SET name = ? WHERE channel_id = ?")
            .bind(&name)
            .bind(id as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;
    }

    if req.protocol.is_some() {
        sqlx::query("UPDATE channels SET protocol = ? WHERE channel_id = ?")
            .bind(&protocol)
            .bind(id as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;
    }

    // Extract current configuration for restoration and apply requested updates
    let (previous_description, previous_parameters, _previous_logging) =
        parse_channel_config(current_config_str.clone());

    let mut description = previous_description.clone();
    let mut parameters = previous_parameters.clone();

    if let Some(new_description) = &req.description {
        description = Some(new_description.clone());
    }

    if let Some(params) = &req.parameters {
        for (key, value) in params {
            parameters.insert(key.clone(), value.clone());
        }
    }

    if req.description.is_some() || req.parameters.is_some() {
        let logging = ChannelLoggingConfig::default();
        let config_json = build_channel_config_json(description.as_ref(), &parameters, &logging)
            .map_err(|e| {
                tracing::error!("Serialize Ch{}: {}", id, e);
                AppError::internal_error(format!("Failed to build config JSON: {}", e))
            })?;

        sqlx::query("UPDATE channels SET config = ? WHERE channel_id = ?")
            .bind(&config_json)
            .bind(id as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;
    }

    // 5. Analyze parameter changes to determine if hot reload is needed
    let name_changed = req.name.is_some() && req.name.as_ref() != Some(&current_name);
    let description_changed = req.description.is_some() && req.description != previous_description;
    let protocol_changed =
        req.protocol.is_some() && req.protocol.as_ref() != Some(&current_protocol);

    let change_type = if is_running {
        analyze_parameter_changes(
            &previous_parameters,
            &parameters,
            name_changed,
            description_changed,
            protocol_changed,
        )
    } else {
        ParameterChangeType::MetadataOnly // Not running, no need to reload
    };

    tracing::debug!("Ch{} change: {:?}", id, change_type);

    // 6. Hot reload if channel is running and changes require it
    let runtime_status = if is_running {
        use ParameterChangeType::*;

        match change_type {
            MetadataOnly => {
                // Only name or description changed, no hot reload needed
                tracing::debug!("Ch{} metadata only", id);
                tx.commit().await.map_err(|e| {
                    AppError::internal_error(format!("Database operation failed: {}", e))
                })?;
                "running".to_string()
            },
            NonCritical => {
                // Performance parameters changed, proceed with hot reload
                tracing::debug!("Ch{} non-critical change, hot reload", id);

                // Commit database first
                tx.commit().await.map_err(|e| {
                    AppError::internal_error(format!("Database operation failed: {}", e))
                })?;

                // Build new config
                let new_config = crate::core::config::ChannelConfig {
                    core: ChannelCore {
                        id,
                        name: name.clone(),
                        description: description.clone(),
                        protocol: protocol.clone(),
                        enabled,
                    },
                    parameters,
                    logging: ChannelLoggingConfig::default(),
                };

                // Spawn background hot reload
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = perform_hot_reload(id, &state_clone, new_config).await {
                        tracing::error!("Ch{} hot reload: {}", id, e);
                    }
                });

                "updated".to_string()
            },
            Critical => {
                // Critical parameters changed, proceed with hot reload
                tracing::debug!("Ch{} critical change, hot reload", id);

                // Commit database first
                tx.commit().await.map_err(|e| {
                    AppError::internal_error(format!("Database operation failed: {}", e))
                })?;

                // Build new config
                let new_config = crate::core::config::ChannelConfig {
                    core: ChannelCore {
                        id,
                        name: name.clone(),
                        description: description.clone(),
                        protocol: protocol.clone(),
                        enabled,
                    },
                    parameters,
                    logging: ChannelLoggingConfig::default(),
                };

                // Spawn background hot reload
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = perform_hot_reload(id, &state_clone, new_config).await {
                        tracing::error!("Ch{} hot reload: {}", id, e);
                    }
                });

                "updated".to_string()
            },
        }
    } else {
        // Channel not running, just commit DB changes
        tx.commit()
            .await
            .map_err(|e| AppError::internal_error(format!("Database operation failed: {}", e)))?;
        tracing::debug!("Ch{} updated (stopped)", id);
        "stopped".to_string()
    };

    // Use the final description after applying updates (or keep previous if not provided)
    let result = crate::dto::ChannelCrudResult {
        core: ChannelCore {
            id,
            name,
            description, // propagate actual description
            protocol,
            enabled,
        },
        runtime_status,
        message: Some("Channel configuration updated successfully".to_string()),
    };

    Ok(Json(SuccessResponse::new(result)))
}

/// Set channel enabled state
///
/// Enable or disable a channel, controlling its runtime lifecycle.
/// This is a higher-level operation than update - it manages whether the channel should run.
///
/// @route PUT /api/channels/{id}/enabled
/// @input Path(id): u16 - Channel ID
/// @input State(state): AppState - Application state
/// @input Json(req): ChannelEnabledRequest - Enabled state
/// @output Json<ApiResponse<ChannelCrudResult>> - Operation result
/// @status 200 - State changed successfully
/// @status 404 - Channel not found
/// @status 500 - Database or runtime error
/// @side-effects Updates SQLite and starts/stops channel
#[utoipa::path(
    put,
    path = "/api/channels/{id}/enabled",
    params(
        ("id" = u32, Path, description = "Channel identifier")
    ),
    request_body = crate::dto::ChannelEnabledRequest,
    responses(
        (status = 200, description = "Channel enabled state updated", body = crate::dto::ChannelCrudResult)
    ),
    tag = "comsrv"
)]
pub async fn set_channel_enabled_handler(
    Path(id): Path<u32>,
    State(state): State<AppState>,
    Json(req): Json<crate::dto::ChannelEnabledRequest>,
) -> Result<Json<SuccessResponse<crate::dto::ChannelCrudResult>>, AppError> {
    use crate::core::config::ChannelConfig;

    tracing::debug!("Ch{} enabled={}", id, req.enabled);

    // 1. Load current configuration from database
    let current: Option<(String, String, bool, Option<String>)> =
        sqlx::query_as("SELECT name, protocol, enabled, config FROM channels WHERE channel_id = ?")
            .bind(id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("DB err: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    let Some((name, protocol, current_enabled, config_str)) = current else {
        return Err(AppError::not_found(format!(
            "Channel {} not found in database",
            id
        )));
    };

    // 2. Parse config for runtime (before early return so we can populate description correctly)
    let (description, parameters, _logging) = parse_channel_config(config_str);

    // 3. Check if state actually changed
    if current_enabled == req.enabled {
        // State unchanged - enabled is a configuration state independent of connection
        return Ok(Json(SuccessResponse::new(crate::dto::ChannelCrudResult {
            core: ChannelCore {
                id,
                name,
                description, // propagate existing description
                protocol,
                enabled: req.enabled,
            },
            runtime_status: if req.enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            message: Some(format!(
                "Channel already {}",
                if req.enabled { "enabled" } else { "disabled" }
            )),
        })));
    }

    // 4. Execute enable or disable
    let runtime_status = if req.enabled {
        // Enable: create and start channel
        let config = ChannelConfig {
            core: ChannelCore {
                id,
                name: name.clone(),
                description: description.clone(),
                protocol: protocol.clone(),
                enabled: true,
            },
            parameters,
            logging: ChannelLoggingConfig::default(),
        };

        let manager = state.channel_manager.write().await;
        match manager.create_channel(Arc::new(config)).await {
            Ok(channel_arc) => {
                // Trigger asynchronous connection in background
                // Don't wait for connection result - let reconnection mechanism handle failures
                let channel_clone = channel_arc.clone();
                let channel_id_for_log = id;
                tokio::spawn(async move {
                    let mut channel = channel_clone.write().await;
                    match channel.connect().await {
                        Ok(_) => tracing::debug!("Ch{} connected", channel_id_for_log),
                        Err(e) => tracing::warn!("Ch{} connect: {}", channel_id_for_log, e),
                    }
                });
                drop(manager);

                // Update database
                if let Err(e) = sqlx::query("UPDATE channels SET enabled = ? WHERE channel_id = ?")
                    .bind(true)
                    .bind(id as i64)
                    .execute(&state.sqlite_pool)
                    .await
                {
                    tracing::error!("Ch{} DB update: {}", id, e);
                    // DB update failed - remove the runtime channel
                    let manager = state.channel_manager.write().await;
                    let _ = manager.remove_channel(id).await;
                    return Err(AppError::internal_error(format!(
                        "Database update failed: {}",
                        e
                    )));
                }

                tracing::debug!("Ch{} enabled (bg connect)", id);
                "connecting".to_string()
            },
            Err(e) => {
                tracing::warn!("Ch{} runtime create: {}", id, e);
                drop(manager);

                // Update database to enabled even if runtime creation failed
                if let Err(e) = sqlx::query("UPDATE channels SET enabled = ? WHERE channel_id = ?")
                    .bind(true)
                    .bind(id as i64)
                    .execute(&state.sqlite_pool)
                    .await
                {
                    tracing::error!("Ch{} DB update: {}", id, e);
                    return Err(AppError::internal_error(format!(
                        "Database update failed: {}",
                        e
                    )));
                }

                tracing::debug!("Ch{} enabled (no runtime)", id);
                "enabled".to_string()
            },
        }
    } else {
        // Disable: stop and remove channel
        let manager = state.channel_manager.write().await;
        if let Err(e) = manager.remove_channel(id).await {
            tracing::warn!("Ch{} remove: {}", id, e);
        }
        drop(manager);

        // Update database
        if let Err(e) = sqlx::query("UPDATE channels SET enabled = ? WHERE channel_id = ?")
            .bind(false)
            .bind(id as i64)
            .execute(&state.sqlite_pool)
            .await
        {
            tracing::error!("Ch{} DB update: {}", id, e);
            return Err(AppError::internal_error(format!(
                "Database update failed: {}",
                e
            )));
        }

        tracing::debug!("Ch{} disabled", id);
        "stopped".to_string()
    };

    let result = crate::dto::ChannelCrudResult {
        core: ChannelCore {
            id,
            name,
            description, // propagate existing description
            protocol,
            enabled: req.enabled,
        },
        runtime_status,
        message: Some(format!(
            "Channel {} successfully",
            if req.enabled { "enabled" } else { "disabled" }
        )),
    };

    Ok(Json(SuccessResponse::new(result)))
}

/// Delete a channel with hot stop
///
/// @route DELETE /api/channels/{id}
/// @input Path(id): u16 - Channel ID to delete
/// @input State(state): AppState - Application state
/// @output Json<ApiResponse<String>> - Deletion result
/// @status 200 - Channel deleted successfully
/// @status 404 - Channel not found
/// @status 500 - Database or runtime error
/// @side-effects Stops channel and removes from SQLite
#[utoipa::path(
    delete,
    path = "/api/channels/{id}",
    params(
        ("id" = u32, Path, description = "Channel identifier")
    ),
    responses(
        (status = 200, description = "Channel deleted", body = String)
    ),
    tag = "comsrv"
)]
pub async fn delete_channel_handler(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<String>>, AppError> {
    tracing::debug!("Deleting Ch{}", id);

    // 1. Begin transaction for atomic deletion
    let mut tx = state.sqlite_pool.begin().await.map_err(|e| {
        tracing::error!("Ch{} delete tx begin: {}", id, e);
        AppError::internal_error(format!("Failed to begin transaction: {}", e))
    })?;

    // 2. Delete channel from database within transaction
    let result = sqlx::query("DELETE FROM channels WHERE channel_id = ?")
        .bind(id as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Ch{} delete: {}", id, e);
            AppError::internal_error(format!("Failed to delete channel from database: {}", e))
        })?;

    if result.rows_affected() == 0 {
        let _ = tx.rollback().await;
        return Err(AppError::not_found(format!(
            "Channel {} not found in database",
            id
        )));
    }

    // 3. Commit transaction BEFORE removing runtime channel
    // This ensures database consistency is preserved
    tx.commit().await.map_err(|e| {
        tracing::error!("Ch{} delete tx commit: {}", id, e);
        AppError::internal_error(format!("Failed to commit transaction: {}", e))
    })?;

    // 4. Remove from runtime (best effort - doesn't affect data consistency)
    // Even if this fails, the channel is gone from database which is the source of truth
    {
        let manager = state.channel_manager.write().await;
        if let Err(e) = manager.remove_channel(id).await {
            tracing::warn!("Ch{} runtime remove: {}", id, e);
        }
    }

    tracing::info!("Ch{} deleted", id);
    Ok(Json(SuccessResponse::new(format!(
        "Channel {} deleted successfully",
        id
    ))))
}

/// Reload all channels from SQLite configuration
///
/// @route POST /api/channels/reload
/// @input State(state): AppState - Application state
/// @output Json<ApiResponse<ReloadConfigResult>> - Reload result
/// @status 200 - Reload completed (may have errors)
/// @side-effects Synchronizes runtime with SQLite configuration
#[utoipa::path(
    post,
    path = "/api/channels/reload",
    responses(
        (status = 200, description = "Configuration reloaded", body = crate::dto::ReloadConfigResult)
    ),
    tag = "comsrv"
)]
pub async fn reload_configuration_handler(
    State(state): State<AppState>,
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
        let manager = state.channel_manager.read().await;
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
        let manager = state.channel_manager.write().await;
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

    // 5. Add new channels from SQLite
    for id in &to_add {
        if let Some((_, name, protocol, enabled)) =
            db_channels.iter().find(|(cid, _, _, _)| *cid as u32 == *id)
        {
            // Load description and parameters from config JSON
            let (description, parameters): (
                Option<String>,
                std::collections::HashMap<String, serde_json::Value>,
            ) = {
                let config_str: Option<String> =
                    sqlx::query_scalar("SELECT config FROM channels WHERE channel_id = ?")
                        .bind(*id as i64)
                        .fetch_optional(&state.sqlite_pool)
                        .await
                        .ok()
                        .flatten();

                let (desc, params, _logging) = parse_channel_config(config_str);
                (desc, params)
            };

            let channel_config = ChannelConfig {
                core: ChannelCore {
                    id: *id,
                    name: name.clone(),
                    description,
                    protocol: protocol.clone(),
                    enabled: *enabled,
                },
                parameters,
                logging: ChannelLoggingConfig::default(),
            };

            // Only create and connect if enabled
            if *enabled {
                let manager = state.channel_manager.write().await;
                match manager.create_channel(Arc::new(channel_config)).await {
                    Ok(channel_arc) => {
                        // Try to connect
                        let mut channel = channel_arc.write().await;
                        if let Err(e) = channel.connect().await {
                            tracing::warn!("Ch{} connect: {}", id, e);
                        }
                        channels_added.push(*id);
                        tracing::debug!("Ch{} added", id);
                    },
                    Err(e) => {
                        errors.push(format!("Failed to add channel {}: {}", id, e));
                    },
                }
            } else {
                // Channel is disabled, don't create runtime instance
                channels_added.push(*id);
                tracing::debug!("Ch{} added (disabled)", id);
            }
        }
    }

    // 6. Update existing channels (reload them)
    for id in &to_update {
        if let Some((_, name, protocol, enabled)) =
            db_channels.iter().find(|(cid, _, _, _)| *cid as u32 == *id)
        {
            // Load description and parameters from config JSON
            let (description, parameters): (
                Option<String>,
                std::collections::HashMap<String, serde_json::Value>,
            ) = {
                let config_str: Option<String> =
                    sqlx::query_scalar("SELECT config FROM channels WHERE channel_id = ?")
                        .bind(*id as i64)
                        .fetch_optional(&state.sqlite_pool)
                        .await
                        .ok()
                        .flatten();

                let (desc, params, _logging) = parse_channel_config(config_str);
                (desc, params)
            };

            let manager = state.channel_manager.write().await;

            // Remove old channel (if exists)
            if let Err(e) = manager.remove_channel(*id).await {
                tracing::debug!("Ch{} not in runtime: {}", id, e);
            }

            // Only create and connect if enabled
            if *enabled {
                let channel_config = ChannelConfig {
                    core: ChannelCore {
                        id: *id,
                        name: name.clone(),
                        description,
                        protocol: protocol.clone(),
                        enabled: *enabled,
                    },
                    parameters,
                    logging: ChannelLoggingConfig::default(),
                };

                match manager.create_channel(Arc::new(channel_config)).await {
                    Ok(channel_arc) => {
                        let mut channel = channel_arc.write().await;
                        if let Err(e) = channel.connect().await {
                            tracing::warn!("Ch{} connect: {}", id, e);
                        }
                        channels_updated.push(*id);
                        tracing::debug!("Ch{} updated", id);
                    },
                    Err(e) => {
                        errors.push(format!("Failed to update channel {}: {}", id, e));
                    },
                }
            } else {
                // Channel is disabled, don't create runtime instance
                channels_updated.push(*id);
                tracing::debug!("Ch{} updated (disabled)", id);
            }
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
/// @input State(state): AppState - Application state
/// @output Json<ApiResponse<RoutingReloadResult>> - Routing reload result
/// @status 200 - Routing cache refreshed successfully
/// @status 500 - Internal server error
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
pub async fn reload_routing_handler(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::RoutingReloadResult>>, AppError> {
    use crate::core::channels::ChannelManager;

    tracing::debug!("Reloading routing");

    let start_time = std::time::Instant::now();
    let mut errors = Vec::new();

    // Get routing_cache reference from channel_manager
    let (c2m_count, m2c_count, c2c_count) = {
        let manager = state.channel_manager.read().await;

        // Call the public reload_routing_cache method
        match ChannelManager::reload_routing_cache(&state.sqlite_pool, &manager.routing_cache).await
        {
            Ok(counts) => counts,
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
