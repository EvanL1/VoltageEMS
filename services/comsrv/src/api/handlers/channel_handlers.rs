//! Channel Query and Status Handlers
//!
//! Provides endpoints for querying channel information and status.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, Query, RawQuery, State},
    response::Json,
};
use chrono::{DateTime, Utc};

use crate::api::routes::AppState;
use crate::dto::{
    AppError, ChannelConfig, ChannelDetail, ChannelListQuery, ChannelRuntimeStatus, ChannelStatus,
    ChannelStatusResponse, PaginatedResponse, PointCounts, SuccessResponse,
};
use voltage_rtdb::Rtdb;

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
                        "name": "PCS#1",
                        "protocol": "modbus_tcp",
                        "description": "Power Converter #1",
                        "enabled": true,
                        "connected": true,
                        "last_update": "2025-10-15T10:30:00Z"
                    },
                    {
                        "id": 2,
                        "name": "BAMS#1",
                        "protocol": "modbus_tcp",
                        "description": "Battery Management System #1",
                        "enabled": true,
                        "connected": true,
                        "last_update": "2025-10-15T10:28:15Z"
                    },
                    {
                        "id": 3,
                        "name": "GENSET#1",
                        "protocol": "modbus_rtu",
                        "description": "Diesel Generator #1",
                        "enabled": true,
                        "connected": false,
                        "last_update": "2025-10-15T10:25:00Z"
                    },
                    {
                        "id": 4,
                        "name": "ECU1170_GPIO",
                        "protocol": "di_do",
                        "description": "ECU-1170 Onboard DI/DO",
                        "enabled": false,
                        "connected": false,
                        "last_update": "2025-10-15T10:30:05Z"
                    }
                ],
                "page": 1,
                "page_size": 20,
                "total": 4
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_all_channels<R: Rtdb>(
    State(state): State<AppState<R>>,
    Query(query): Query<ChannelListQuery>,
) -> Result<Json<SuccessResponse<PaginatedResponse<ChannelStatusResponse>>>, AppError> {
    // Load all channels from database first
    let db_channels: Vec<(i64, String, String, bool, Option<String>)> =
        sqlx::query_as("SELECT channel_id, name, protocol, enabled, config FROM channels")
            .fetch_all(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Load channels: {}", e);
                AppError::internal_error(format!("Failed to load channels from database: {}", e))
            })?;

    let manager = state.channel_manager.read().await;
    let mut all_channels = Vec::new();

    for (channel_id_i64, name, protocol, enabled, config_str) in db_channels {
        let channel_id = channel_id_i64 as u32;

        // Extract description from config JSON
        let description = config_str
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .and_then(|obj| {
                obj.get("description")
                    .and_then(|d| d.as_str().map(|s| s.to_string()))
            });

        // Get runtime status if channel is running
        let (connected, last_update) = if let Some(channel_impl) = manager.get_channel(channel_id) {
            let wrapper = channel_impl.read().await;
            let status = wrapper.get_status().await;
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

    // Use shared pagination utility
    let paginated_response =
        PaginatedResponse::from_slice(all_channels, query.page, query.page_size);

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
                    "name": "PCS#1",
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
pub async fn get_channel_status<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<ChannelStatus>>, AppError> {
    let id_u16 = id
        .parse::<u32>()
        .map_err(|_| AppError::bad_request(format!("Invalid channel ID format: {}", id)))?;
    let manager = state.channel_manager.read().await;

    if let Some(channel_impl) = manager.get_channel(id_u16) {
        let (name, protocol) = manager
            .get_channel_metadata(id_u16)
            .unwrap_or_else(|| (format!("Channel {id_u16}"), "Unknown".to_string()));

        let wrapper = channel_impl.read().await;
        let channel_status = wrapper.get_status().await;
        let is_running = wrapper.is_connected().await;
        let diagnostics = wrapper
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
                    "name": "PCS#1",
                    "description": "Power Converter #1",
                    "protocol": "modbus_tcp",
                    "enabled": true,
                    "parameters": {
                        "host": "192.168.1.10",
                        "port": 502,
                        "connect_timeout_ms": 3000,
                        "read_timeout_ms": 3000
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
pub async fn get_channel_detail_handler<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<ChannelDetail>>, AppError> {
    let id_u16 = id
        .parse::<u32>()
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
                serde_json::from_value::<crate::core::config::ChannelLoggingConfig>(l).ok()
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
    let (connected, last_update, statistics) =
        if let Some(channel_impl) = manager.get_channel(id_u16) {
            let wrapper = channel_impl.read().await;
            let status = wrapper.get_status().await;
            let diag = wrapper
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
        core: crate::core::config::ChannelCore {
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

/// Search channels by name with fuzzy matching (no pagination)
///
/// Returns all channels matching the search keyword. Use this for autocomplete
/// or quick lookup scenarios where you need all matches without pagination.
///
/// URL format: `/api/channels/search?{keyword}`
/// - The keyword is passed directly as the raw query string (no parameter name needed)
/// - Empty keyword returns all channels
///
/// @route GET /api/channels/search?{keyword}
#[utoipa::path(
    get,
    path = "/api/channels/search",
    params(
        ("keyword" = Option<String>, Query, description = "Optional fuzzy keyword (legacy raw query also supported)"),
        ("ids" = Option<String>, Query, description = "Optional channel id filter, comma-separated (e.g., ids=1,2,3)")
    ),
    responses(
        (status = 200, description = "Matching channels", body = serde_json::Value,
            example = json!({
                "list": [
                    {
                        "id": 1,
                        "name": "PCS#1",
                        "description": "Power Converter #1",
                        "protocol": "modbus_tcp",
                        "enabled": true,
                        "connected": true
                    }
                ]
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn search_channels<R: Rtdb>(
    State(state): State<AppState<R>>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    // raw_query is Option<String>:
    // /search?modbus                 => Some("modbus")                (legacy keyword-only)
    // /search?ids=1,2,3              => Some("ids=1,2,3")             (filter by ids)
    // /search?keyword=modbus&ids=1,2 => Some("keyword=modbus&ids=1,2") (named params)
    // /search?modbus&ids=1,2         => Some("modbus&ids=1,2")        (mixed legacy + ids)
    // /search?                       => Some("")
    // /search                        => None

    fn parse_ids_param(value: &str) -> Vec<u32> {
        value
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .collect()
    }

    let raw = raw_query.unwrap_or_default();
    let mut keyword = String::new();
    let mut ids: Vec<u32> = Vec::new();

    if raw.contains('=') || raw.contains('&') {
        for part in raw.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "ids" | "id" => ids.extend(parse_ids_param(v)),
                    "keyword" | "q" => {
                        if keyword.is_empty() {
                            keyword = v.to_string();
                        }
                    },
                    _ => {},
                }
            } else if keyword.is_empty() && !part.trim().is_empty() {
                // Legacy keyword in mixed query
                keyword = part.to_string();
            }
        }
    } else {
        keyword = raw;
    }

    let like_pattern = format!("%{}%", keyword);

    // Query from SQLite
    let mut sql = String::from(
        r#"SELECT channel_id, name, protocol, enabled, config
           FROM channels
           WHERE name LIKE ?"#,
    );
    if !ids.is_empty() {
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND channel_id IN ({})", placeholders));
    }
    sql.push_str(" ORDER BY channel_id ASC");

    let mut query =
        sqlx::query_as::<_, (i64, String, String, bool, Option<String>)>(&sql).bind(&like_pattern);
    for id in &ids {
        query = query.bind(*id as i64);
    }

    let channels: Vec<(i64, String, String, bool, Option<String>)> =
        query.fetch_all(&state.sqlite_pool).await.map_err(|e| {
            tracing::error!("Search channels: {}", e);
            AppError::internal_error(format!("Failed to search channels: {}", e))
        })?;

    // Get runtime status for connected info
    let manager = state.channel_manager.read().await;

    // Helper: fetch simplified point list (point_id + signal_name only)
    async fn fetch_point_names(
        pool: &sqlx::SqlitePool,
        table: &str,
        channel_id: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let query = format!(
            "SELECT point_id, signal_name FROM {} WHERE channel_id = ? ORDER BY point_id",
            table
        );
        let rows: Vec<(u32, String)> = sqlx::query_as(&query)
            .bind(channel_id)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|(point_id, signal_name)| {
                serde_json::json!({
                    "point_id": point_id,
                    "signal_name": signal_name
                })
            })
            .collect())
    }

    // Build response (with embedded point definitions)
    let mut list: Vec<serde_json::Value> = Vec::with_capacity(channels.len());
    for (id, name, protocol, enabled, config_str) in channels {
        let channel_id = id as u32;

        // Extract description from config JSON
        let description = config_str
            .as_ref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| {
                v.get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from)
            });

        // Get runtime connected status
        let connected = manager
            .get_channel(channel_id)
            .map(|_| true) // Channel exists in runtime = running
            .unwrap_or(false);

        let channel_id_i64 = id;
        let telemetry_points =
            fetch_point_names(&state.sqlite_pool, "telemetry_points", channel_id_i64)
                .await
                .map_err(|e| {
                    tracing::error!("Fetch T points for channel {}: {}", channel_id, e);
                    AppError::internal_error("Database operation failed")
                })?;
        let signal_points = fetch_point_names(&state.sqlite_pool, "signal_points", channel_id_i64)
            .await
            .map_err(|e| {
                tracing::error!("Fetch S points for channel {}: {}", channel_id, e);
                AppError::internal_error("Database operation failed")
            })?;
        let control_points =
            fetch_point_names(&state.sqlite_pool, "control_points", channel_id_i64)
                .await
                .map_err(|e| {
                    tracing::error!("Fetch C points for channel {}: {}", channel_id, e);
                    AppError::internal_error("Database operation failed")
                })?;
        let adjustment_points =
            fetch_point_names(&state.sqlite_pool, "adjustment_points", channel_id_i64)
                .await
                .map_err(|e| {
                    tracing::error!("Fetch A points for channel {}: {}", channel_id, e);
                    AppError::internal_error("Database operation failed")
                })?;

        list.push(serde_json::json!({
            "id": id,
            "name": name,
            "description": description,
            "protocol": protocol,
            "enabled": enabled,
            "connected": connected,
            "points": {
                "telemetry": telemetry_points,
                "signal": signal_points,
                "control": control_points,
                "adjustment": adjustment_points
            }
        }));
    }

    Ok(Json(SuccessResponse::new(
        serde_json::json!({ "list": list }),
    )))
}

/// List all channels (lightweight: id + name only)
///
/// @route GET /api/channels/list
#[utoipa::path(
    get,
    path = "/api/channels/list",
    responses(
        (status = 200, description = "Channel list", body = serde_json::Value,
            example = json!({
                "list": [
                    {"id": 1, "name": "PCS#1"},
                    {"id": 2, "name": "BAMS#1"}
                ]
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn list_channels<R: Rtdb>(
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    let channels: Vec<(i64, String)> =
        sqlx::query_as("SELECT channel_id, name FROM channels ORDER BY channel_id")
            .fetch_all(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("List channels: {}", e);
                AppError::internal_error(format!("Failed to list channels: {}", e))
            })?;

    let list: Vec<serde_json::Value> = channels
        .into_iter()
        .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
        .collect();

    Ok(Json(SuccessResponse::new(
        serde_json::json!({ "list": list }),
    )))
}

/// Query parameters for global points search
#[derive(Debug, serde::Deserialize)]
pub struct PointsQuery {
    /// Filter by channel ID
    pub channel_id: Option<u32>,
    /// Filter by point type (T/S/C/A)
    #[serde(rename = "type")]
    pub point_type: Option<String>,
    /// Filter by point ID
    pub point_id: Option<u32>,
    /// Fuzzy search by signal name
    pub keyword: Option<String>,
}

/// List all points across channels (global search)
///
/// @route GET /api/points
#[utoipa::path(
    get,
    path = "/api/points",
    params(
        ("channel_id" = Option<u32>, Query, description = "Filter by channel ID"),
        ("type" = Option<String>, Query, description = "Filter by point type (T/S/C/A)"),
        ("point_id" = Option<u32>, Query, description = "Filter by point ID"),
        ("keyword" = Option<String>, Query, description = "Fuzzy search by signal name")
    ),
    responses(
        (status = 200, description = "Points list", body = serde_json::Value,
            example = json!({
                "list": [
                    {"channel_id": 1, "type": "T", "point_id": 1, "signal_name": "System_Fault_status"},
                    {"channel_id": 1, "type": "T", "point_id": 2, "signal_name": "System_ON/OFF_status"}
                ]
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn list_all_points<R: Rtdb>(
    State(state): State<AppState<R>>,
    Query(query): Query<PointsQuery>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    // Determine which tables to query based on type filter
    let tables: Vec<(&str, &str)> = match query.point_type.as_deref() {
        Some("T") => vec![("telemetry_points", "T")],
        Some("S") => vec![("signal_points", "S")],
        Some("C") => vec![("control_points", "C")],
        Some("A") => vec![("adjustment_points", "A")],
        _ => vec![
            ("telemetry_points", "T"),
            ("signal_points", "S"),
            ("control_points", "C"),
            ("adjustment_points", "A"),
        ],
    };

    let mut all_points: Vec<serde_json::Value> = Vec::new();

    for (table, type_code) in tables {
        let mut sql = format!(
            "SELECT channel_id, point_id, signal_name FROM {} WHERE 1=1",
            table
        );
        let mut bindings: Vec<String> = Vec::new();

        if let Some(cid) = query.channel_id {
            sql.push_str(" AND channel_id = ?");
            bindings.push(cid.to_string());
        }
        if let Some(pid) = query.point_id {
            sql.push_str(" AND point_id = ?");
            bindings.push(pid.to_string());
        }
        if let Some(ref kw) = query.keyword {
            sql.push_str(" AND signal_name LIKE ?");
            bindings.push(format!("%{}%", kw));
        }
        sql.push_str(" ORDER BY channel_id, point_id");

        let mut q = sqlx::query_as::<_, (i64, u32, String)>(&sql);
        for b in &bindings {
            q = q.bind(b);
        }

        let rows = q.fetch_all(&state.sqlite_pool).await.map_err(|e| {
            tracing::error!("Query {} failed: {}", table, e);
            AppError::internal_error("Database query failed")
        })?;

        for (channel_id, point_id, signal_name) in rows {
            all_points.push(serde_json::json!({
                "channel_id": channel_id,
                "type": type_code,
                "point_id": point_id,
                "signal_name": signal_name
            }));
        }
    }

    Ok(Json(SuccessResponse::new(
        serde_json::json!({ "list": all_points }),
    )))
}
