//! Channel Query and Status Handlers
//!
//! Provides endpoints for querying channel information and status.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};

use crate::api::routes::AppState;
use crate::dto::{
    AppError, ChannelConfig, ChannelDetail, ChannelListQuery, ChannelRuntimeStatus, ChannelStatus,
    ChannelStatusResponse, PaginatedResponse, PointCounts, SuccessResponse,
};

/// List all channels with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/channels",
    params(
        ("page" = Option<usize>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<usize>, Query, description = "Items per page (default: 20)"),
        ("protocol" = Option<String>, Query, description = "Filter by protocol type"),
        ("enabled" = Option<bool>, Query, description = "Filter by enabled status"),
        ("connected" = Option<bool>, Query, description = "Filter by connection status")
    ),
    responses(
        (status = 200, description = "Paginated list of channels", body = crate::dto::PaginatedResponse<crate::dto::ChannelStatusResponse>,
            example = json!({
                "list": [
                    {
                        "id": 1,
                        "name": "PV Inverter 01",
                        "protocol": "modbus_tcp",
                        "description": "Primary PV inverter communication",
                        "enabled": true,
                        "connected": true,
                        "last_update": "2025-10-15T10:30:00Z"
                    },
                    {
                        "id": 2,
                        "name": "Battery Pack RTU",
                        "protocol": "modbus_rtu",
                        "description": "Battery management system",
                        "enabled": true,
                        "connected": false,
                        "last_update": "2025-10-15T10:28:15Z"
                    },
                    {
                        "id": 4,
                        "name": "Virtual Test Channel",
                        "protocol": "virtual",
                        "description": "Virtual channel for testing",
                        "enabled": true,
                        "connected": true,
                        "last_update": "2025-10-15T10:30:05Z"
                    }
                ],
                "page": 1,
                "page_size": 20,
                "total": 7
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_all_channels(
    State(state): State<AppState>,
    Query(query): Query<ChannelListQuery>,
) -> Result<Json<SuccessResponse<PaginatedResponse<ChannelStatusResponse>>>, AppError> {
    // Load all channels from database first
    let db_channels: Vec<(i64, String, String, bool, Option<String>)> =
        sqlx::query_as("SELECT channel_id, name, protocol, enabled, config FROM channels")
            .fetch_all(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load channels from database: {}", e);
                AppError::internal_error(format!("Failed to load channels from database: {}", e))
            })?;

    let manager = state.channel_manager.read().await;
    let mut all_channels = Vec::new();

    for (channel_id_i64, name, protocol, enabled, config_str) in db_channels {
        let channel_id = channel_id_i64 as u16;

        // Extract description from config JSON
        let description = config_str
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .and_then(|obj| {
                obj.get("description")
                    .and_then(|d| d.as_str().map(|s| s.to_string()))
            });

        // Get runtime status if channel is running
        let (connected, last_update) = if let Some(channel) = manager.get_channel(channel_id) {
            let channel_guard = channel.read().await;
            let status = channel_guard.get_status().await;
            (
                status.is_connected,
                DateTime::<Utc>::from_timestamp(status.last_update, 0).unwrap_or_else(Utc::now),
            )
        } else {
            (false, Utc::now())
        };

        let channel_response = ChannelStatusResponse {
            id: channel_id,
            name,
            description,
            protocol: protocol.clone(),
            enabled,
            connected,
            last_update,
        };

        // Apply filters
        let mut should_include = true;

        if let Some(ref filter_protocol) = query.protocol {
            if &protocol != filter_protocol {
                should_include = false;
            }
        }

        if let Some(filter_enabled) = query.enabled {
            if enabled != filter_enabled {
                should_include = false;
            }
        }

        if let Some(filter_connected) = query.connected {
            if connected != filter_connected {
                should_include = false;
            }
        }

        if should_include {
            all_channels.push(channel_response);
        }
    }
    drop(manager);

    // Calculate pagination
    let total = all_channels.len();
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);

    let start_index = (page - 1) * page_size;
    let end_index = start_index + page_size;

    let list = if start_index < all_channels.len() {
        all_channels[start_index..end_index.min(all_channels.len())].to_vec()
    } else {
        Vec::new()
    };

    let paginated_response = PaginatedResponse::new(list, total, page, page_size);

    Ok(Json(SuccessResponse::new(paginated_response)))
}

/// Get channel status
#[utoipa::path(
    get,
    path = "/api/channels/{id}/status",
    params(
        ("id" = String, Path, description = "Channel identifier")
    ),
    responses(
        (status = 200, description = "Channel status", body = crate::dto::ChannelStatus,
            example = json!({
                "success": true,
                "data": {
                    "id": 1,
                    "name": "PV Inverter 01",
                    "protocol": "modbus_tcp",
                    "connected": true,
                    "running": true,
                    "last_update": "2025-10-15T10:30:15Z",
                    "statistics": {
                        "total_reads": 15234,
                        "successful_reads": 15230,
                        "failed_reads": 4,
                        "total_writes": 128,
                        "successful_writes": 128,
                        "failed_writes": 0,
                        "uptime_seconds": 86400,
                        "avg_response_time_ms": 12.5
                    }
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_channel_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<ChannelStatus>>, AppError> {
    let id_u16 = id
        .parse::<u16>()
        .map_err(|_| AppError::bad_request(format!("Invalid channel ID format: {}", id)))?;
    let manager = state.channel_manager.read().await;

    if let Some(channel) = manager.get_channel(id_u16) {
        let (name, protocol) = manager
            .get_channel_metadata(id_u16)
            .unwrap_or_else(|| (format!("Channel {id_u16}"), "Unknown".to_string()));

        let channel_guard = channel.read().await;
        let channel_status = channel_guard.get_status().await;
        let is_running = channel_guard.is_connected();
        let diagnostics = channel_guard
            .get_diagnostics()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));

        let status = ChannelStatus {
            id: id_u16,
            name,
            protocol,
            connected: channel_status.is_connected,
            running: is_running,
            last_update: DateTime::<Utc>::from_timestamp(channel_status.last_update, 0)
                .unwrap_or_else(Utc::now),
            statistics: diagnostics
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        };
        Ok(Json(SuccessResponse::new(status)))
    } else {
        Err(AppError::not_found(format!("Channel {} not found", id_u16)))
    }
}

/// Get complete channel details (configuration + runtime + statistics)
#[utoipa::path(
    get,
    path = "/api/channels/{id}",
    params(
        ("id" = String, Path, description = "Channel identifier")
    ),
    responses(
        (status = 200, description = "Channel details", body = crate::dto::ChannelDetail,
            example = json!({
                "success": true,
                "data": {
                    "id": 1,
                    "name": "PV Inverter 01",
                    "description": "Primary PV inverter communication channel",
                    "protocol": "modbus_tcp",
                    "enabled": true,
                    "parameters": {
                        "host": "192.168.1.100",
                        "port": 502,
                        "timeout_ms": 1000,
                        "retry_count": 3,
                        "poll_interval_ms": 500
                    },
                    "runtime_status": {
                        "connected": true,
                        "running": true,
                        "last_update": "2025-10-15T10:30:15Z",
                        "statistics": {
                            "total_reads": 15234,
                            "successful_reads": 15230,
                            "failed_reads": 4,
                            "uptime_seconds": 86400
                        }
                    },
                    "point_counts": {
                        "telemetry": 45,
                        "signal": 12,
                        "control": 8,
                        "adjustment": 6
                    }
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_channel_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<ChannelDetail>>, AppError> {
    let id_u16 = id
        .parse::<u16>()
        .map_err(|_| AppError::bad_request(format!("Invalid channel ID format: {}", id)))?;

    let (name, protocol, enabled, description, parameters, logging_config) = if let Ok(row) =
        sqlx::query_as::<_, (String, String, bool, Option<String>)>(
            "SELECT name, protocol, enabled, config FROM channels WHERE channel_id = ?",
        )
        .bind(id_u16 as i64)
        .fetch_one(&state.sqlite_pool)
        .await
    {
        let config_obj = row
            .3
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let mut obj = config_obj;

        // Extract description
        let desc = obj
            .remove("description")
            .and_then(|d| d.as_str().map(|s| s.to_string()));

        // Extract logging config
        let logging = obj
            .remove("logging")
            .and_then(|l| {
                serde_json::from_value::<voltage_config::comsrv::ChannelLoggingConfig>(l).ok()
            })
            .unwrap_or_default();

        // Extract parameters (the actual protocol parameters)
        let params = obj
            .remove("parameters")
            .and_then(|p| p.as_object().cloned())
            .map(|obj| obj.into_iter().collect())
            .unwrap_or_default();

        (row.0, row.1, row.2, desc, params, logging)
    } else {
        return Err(AppError::not_found(format!("Channel {} not found", id_u16)));
    };

    let manager = state.channel_manager.read().await;
    let (connected, last_update, statistics) = if let Some(ch) = manager.get_channel(id_u16) {
        let guard = ch.read().await;
        let status = guard.get_status().await;
        let diag = guard
            .get_diagnostics()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));
        (
            status.is_connected,
            DateTime::<Utc>::from_timestamp(status.last_update, 0).unwrap_or_else(Utc::now),
            diag.as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        )
    } else {
        (false, Utc::now(), std::collections::HashMap::new())
    };

    let config = ChannelConfig {
        core: voltage_config::comsrv::ChannelCore {
            id: id_u16,
            name,
            description,
            protocol,
            enabled,
        },
        parameters,
        logging: logging_config,
    };

    // Query actual point counts by type for this channel
    let telemetry_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM telemetry_points WHERE channel_id = ?")
            .bind(id_u16 as i64)
            .fetch_one(&state.sqlite_pool)
            .await
            .unwrap_or(0);

    let signal_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM signal_points WHERE channel_id = ?")
            .bind(id_u16 as i64)
            .fetch_one(&state.sqlite_pool)
            .await
            .unwrap_or(0);

    let control_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM control_points WHERE channel_id = ?")
            .bind(id_u16 as i64)
            .fetch_one(&state.sqlite_pool)
            .await
            .unwrap_or(0);

    let adjustment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM adjustment_points WHERE channel_id = ?")
            .bind(id_u16 as i64)
            .fetch_one(&state.sqlite_pool)
            .await
            .unwrap_or(0);

    let detail = ChannelDetail {
        config,
        runtime_status: ChannelRuntimeStatus {
            connected,
            running: connected,
            last_update,
            statistics,
        },
        point_counts: PointCounts {
            telemetry: telemetry_count as usize,
            signal: signal_count as usize,
            control: control_count as usize,
            adjustment: adjustment_count as usize,
        },
    };

    Ok(Json(SuccessResponse::new(detail)))
}
