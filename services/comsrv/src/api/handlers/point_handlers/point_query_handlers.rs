#![allow(clippy::disallowed_methods)]

//! Query handlers for point information, configuration, and unmapped points

use crate::api::routes::AppState;
use crate::dto::{AppError, SuccessResponse};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use voltage_model::{KeySpaceConfig, PointType};
use voltage_rtdb::Rtdb;

use super::point_helpers::validate_channel_exists;

/// Get point information including value, timestamp and raw value
///
/// @route GET /api/channels/{channel_id}/{telemetry_type}/{point_id}
/// @input State(state): AppState - Application state with Redis client
/// @input Path((channel_id, telemetry_type, point_id)): (u16, String, u32) - Identifiers
/// @output `Json<ApiResponse<Value>>` - Point information JSON
/// @status 200 - Point information retrieved
/// @status 400 - Invalid telemetry type
/// @status 500 - Redis operation failed
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{telemetry_type}/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("telemetry_type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Point information", body = serde_json::Value,
            example = json!({
                "success": true,
                "data": {
                    "channel_id": 1,
                    "telemetry_type": "T",
                    "point_id": 101,
                    "value": "650.5",
                    "timestamp": "1729000815",
                    "raw": "6505"
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_point_info_handler<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path((channel_id, telemetry_type, point_id)): Path<(u32, String, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    // Parse and validate telemetry type (also serves as validation)
    let point_type = PointType::from_str(&telemetry_type).ok_or_else(|| {
        AppError::bad_request(format!(
            "Invalid telemetry type '{}'. Must be T, S, C, or A",
            telemetry_type
        ))
    })?;

    // Removed VecRtdb - now reading directly from Redis
    // Read from Redis (3 hash_get calls)
    // Use cached &'static config for zero allocation on key generation
    let keyspace = KeySpaceConfig::production_cached();
    let field = point_id.to_string();
    let data_key = keyspace.channel_key(channel_id, point_type);
    let ts_key = keyspace.channel_ts_key(channel_id, point_type);
    let raw_key = keyspace.channel_raw_key(channel_id, point_type);

    let value = match state.rtdb.hash_get(&data_key, &field).await {
        Ok(opt) => opt.map(|bytes| String::from_utf8_lossy(&bytes).to_string()),
        Err(e) => {
            return Err(AppError::internal_error(format!(
                "Failed to read value: {}",
                e
            )))
        },
    };

    let timestamp = match state.rtdb.hash_get(&ts_key, &field).await {
        Ok(opt) => opt.map(|bytes| String::from_utf8_lossy(&bytes).to_string()),
        Err(e) => {
            return Err(AppError::internal_error(format!(
                "Failed to read timestamp: {}",
                e
            )))
        },
    };

    let raw_value = match state.rtdb.hash_get(&raw_key, &field).await {
        Ok(opt) => opt.map(|bytes| String::from_utf8_lossy(&bytes).to_string()),
        Err(e) => {
            return Err(AppError::internal_error(format!(
                "Failed to read raw value: {}",
                e
            )))
        },
    };

    Ok(Json(SuccessResponse::new(serde_json::json!({
        "channel_id": channel_id,
        "telemetry_type": point_type.as_str(),
        "point_id": point_id,
        "value": value,
        "timestamp": timestamp,
        "raw": raw_value,
        "source": "redis"  // Indicate data source for debugging
    }))))
}

/// Get list of points for a channel, optionally filtered by type
///
/// Returns all point definitions for the specified channel.
/// Supports filtering by point type (T, S, C, A).
///
/// @route GET /api/channels/{id}/points
/// @input Path(id): u16 - Channel ID
/// @input Query(query): `Option<String>` - Point type filter (T/S/C/A)
/// @input State(state): AppState - Application state
/// @output `Json<ApiResponse<GroupedPoints>>` - Grouped point definitions by type
/// @status 200 - Points retrieved successfully
/// @status 404 - Channel not found
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/channels/{id}/points",
    params(
        ("id" = u32, Path, description = "Channel identifier"),
        ("type" = Option<String>, Query, description = "Point type filter: T (telemetry), S (signal), C (control), A (adjustment)")
    ),
    responses(
        (status = 200, description = "Points retrieved (grouped)", body = crate::dto::GroupedPoints,
            example = json!({
                "success": true,
                "data": {
                    "telemetry": [
                        {
                            "point_id": 101,
                            "signal_name": "DC_Voltage",
                            "scale": 0.1,
                            "offset": 0.0,
                            "unit": "V",
                            "data_type": "uint16",
                            "reverse": false,
                            "description": "DC bus voltage",
                            "protocol_mapping": {
                                "slave_id": 1,
                                "function_code": 3,
                                "register_address": 100,
                                "data_type": "float32",
                                "byte_order": "ABCD",
                                "bit_position": 0
                            }
                        }
                    ],
                    "signal": [],
                    "control": [],
                    "adjustment": []
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_channel_points_handler<R: Rtdb>(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::GroupedPoints>>, AppError> {
    // 1. Verify channel exists
    let channel_exists: Option<(i64,)> =
        sqlx::query_as("SELECT channel_id FROM channels WHERE channel_id = ?")
            .bind(channel_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Ch check: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    if channel_exists.is_none() {
        return Err(AppError::not_found(format!(
            "Channel {} not found",
            channel_id
        )));
    }

    // 2. Get point type filter from query params
    let point_type = params.get("type").map(|s| s.as_str());

    // 3. Build grouped point lists based on filter
    let mut telemetry_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut signal_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut control_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut adjustment_points: Vec<crate::dto::PointDefinition> = Vec::new();

    // Helper function to fetch points from a table with protocol_mappings
    async fn fetch_points_from_table(
        pool: &sqlx::SqlitePool,
        table: &str,
        channel_id: i64,
        _has_normal_state: bool,
    ) -> Result<Vec<crate::dto::PointDefinition>, sqlx::Error> {
        // Build per-table SELECT with normalized columns to a common shape:
        // (point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
        let query = match table {
            "telemetry_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
                table
            ),
            "signal_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
                table
            ),
            "control_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
                table
            ),
            "adjustment_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
                table
            ),
            _ => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? ORDER BY point_id",
                table
            ),
        };

        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            u32,
            String,
            f64,
            f64,
            String,
            String,
            bool,
            String,
            Option<String>,
        )> = sqlx::query_as(&query)
            .bind(channel_id)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    point_id,
                    signal_name,
                    scale,
                    offset,
                    unit,
                    data_type,
                    reverse,
                    description,
                    protocol_mappings_json,
                )| {
                    // Parse protocol_mappings JSON if present
                    let protocol_mapping = if let Some(json_str) = protocol_mappings_json.as_ref() {
                        if !json_str.trim().is_empty() {
                            match serde_json::from_str::<serde_json::Value>(json_str) {
                                Ok(value) if !value.is_null() => Some(value),
                                Ok(_) => None, // null value
                                Err(e) => {
                                    tracing::warn!("Parse mapping {}: {}", point_id, e);
                                    None
                                },
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    crate::dto::PointDefinition {
                        point_id,
                        signal_name,
                        scale,
                        offset,
                        unit,
                        data_type,
                        reverse,
                        description,
                        protocol_mapping,
                    }
                },
            )
            .collect())
    }

    let channel_id_i64 = channel_id as i64;

    match point_type {
        Some("T") | Some("t") => {
            telemetry_points = fetch_points_from_table(
                &state.sqlite_pool,
                "telemetry_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch T points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some("S") | Some("s") => {
            signal_points =
                fetch_points_from_table(&state.sqlite_pool, "signal_points", channel_id_i64, true)
                    .await
                    .map_err(|e| {
                        tracing::error!("Fetch S points: {}", e);
                        AppError::internal_error("Database operation failed")
                    })?;
        },
        Some("C") | Some("c") => {
            control_points = fetch_points_from_table(
                &state.sqlite_pool,
                "control_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch C points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some("A") | Some("a") => {
            adjustment_points = fetch_points_from_table(
                &state.sqlite_pool,
                "adjustment_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch A points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some(invalid) => {
            return Err(AppError::bad_request(format!(
                "Invalid point type filter '{}'. Must be T, S, C, or A",
                invalid
            )));
        },
        None => {
            telemetry_points = fetch_points_from_table(
                &state.sqlite_pool,
                "telemetry_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch T points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
            signal_points =
                fetch_points_from_table(&state.sqlite_pool, "signal_points", channel_id_i64, true)
                    .await
                    .map_err(|e| {
                        tracing::error!("Fetch S points: {}", e);
                        AppError::internal_error("Database operation failed")
                    })?;
            control_points = fetch_points_from_table(
                &state.sqlite_pool,
                "control_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch C points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
            adjustment_points = fetch_points_from_table(
                &state.sqlite_pool,
                "adjustment_points",
                channel_id_i64,
                false,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch A points: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
    }

    let grouped = crate::dto::GroupedPoints {
        telemetry: telemetry_points,
        signal: signal_points,
        control: control_points,
        adjustment: adjustment_points,
    };

    Ok(Json(SuccessResponse::new(grouped)))
}

/// Get mapping for a specific point with explicit four-remote type
///
/// This endpoint requires the four-remote type (T/S/C/A) to uniquely identify a point mapping,
/// as point_id alone is not unique across different point types within a channel.
///
/// Unique identifier: (channel_id, four_remote_type, point_id)
///
/// @route GET /api/channels/{channel_id}/{type}/points/{point_id}/mapping
/// @param channel_id - Channel ID
/// @param type - Four-remote type (T/S/C/A)
/// @param point_id - Point ID
/// @return PointMappingDetail - Point mapping configuration
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}/mapping",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Four-remote type: T(Telemetry), S(Signal), C(Control), A(Adjustment)"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Mapping retrieved successfully", body = crate::dto::PointMappingDetail),
        (status = 400, description = "Invalid four-remote type (must be T, S, C, or A)"),
        (status = 404, description = "Channel or point not found in specified type")
    ),
    tag = "comsrv"
)]
pub async fn get_point_mapping_with_type_handler<R: Rtdb>(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointMappingDetail>>, AppError> {
    // 1. Validate and map four-remote type to table name
    let table = match point_type.to_uppercase().as_str() {
        "T" => "telemetry_points",
        "S" => "signal_points",
        "C" => "control_points",
        "A" => "adjustment_points",
        _ => {
            return Err(AppError::bad_request(format!(
                "Invalid four-remote type '{}'. Must be T (Telemetry), S (Signal), C (Control), or A (Adjustment)",
                point_type
            )))
        }
    };

    // 2. Verify channel exists
    let channel_exists: Option<(i64,)> =
        sqlx::query_as("SELECT channel_id FROM channels WHERE channel_id = ?")
            .bind(channel_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Ch check: {}", e);
                AppError::internal_error("Database operation failed")
            })?;

    if channel_exists.is_none() {
        return Err(AppError::not_found(format!(
            "Channel {} not found",
            channel_id
        )));
    }

    // 3. Query the specific point table with channel_id and point_id
    let query = format!(
        "SELECT signal_name, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );

    let result: Option<(String, Option<String>)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Query {}: {}", table, e);
            AppError::internal_error("Database operation failed")
        })?;

    match result {
        Some((signal_name, protocol_mappings_json)) => {
            // Parse protocol_mappings JSON if present
            let protocol_data = if let Some(json_str) = protocol_mappings_json {
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(value) => value,
                    Err(e) => {
                        tracing::error!(
                            "Failed to parse protocol_mappings JSON for point {}: {}",
                            point_id,
                            e
                        );
                        serde_json::Value::Object(serde_json::Map::new())
                    },
                }
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            Ok(Json(SuccessResponse::new(crate::dto::PointMappingDetail {
                point_id,
                signal_name,
                protocol_data,
            })))
        },
        None => Err(AppError::not_found(format!(
            "Point {} (type {}) not found in channel {}",
            point_id,
            point_type.to_uppercase(),
            channel_id
        ))),
    }
}

// ----------------------------------------------------------------------------
// Get Point Configuration Handler
// ----------------------------------------------------------------------------

/// Get point configuration from database
///
/// @route GET /api/channels/{channel_id}/{type}/points/{point_id}/config
/// @input Path((channel_id, point_type, point_id)): (u16, String, u32) - Identifiers
/// @output `Json<ApiResponse<PointDefinition>>` - Complete point configuration
/// @status 200 - Point configuration retrieved
/// @status 400 - Invalid point type
/// @status 404 - Channel or point not found
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}/config",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier")
    ),
    responses(
        (status = 200, description = "Point configuration", body = crate::dto::PointDefinition),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
/// Internal implementation for get_point_config_handler
async fn get_point_config_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    let point_type_upper = point_type.to_ascii_uppercase();

    // Validate point type and get table name
    let table = match point_type_upper.as_str() {
        "T" => "telemetry_points",
        "S" => "signal_points",
        "C" => "control_points",
        "A" => "adjustment_points",
        _ => {
            return Err(AppError::bad_request(format!(
                "Invalid point type '{}'. Must be T, S, C, or A",
                point_type
            )));
        },
    };

    // Validate channel exists
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // Query point configuration from database
    // Normalize columns to common shape: (point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
    let query = match table {
        "telemetry_points" => format!(
            "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ),
        "signal_points" => format!(
            "SELECT point_id, signal_name, 1.0 AS scale, 0.0 AS offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ),
        "control_points" => format!(
            "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ),
        "adjustment_points" => format!(
            "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ),
        other => {
            return Err(AppError::internal_error(format!(
                "Invalid table name: {}",
                other
            )));
        },
    };

    #[allow(clippy::type_complexity)]
    let result: Option<(
        u32,
        String,
        f64,
        f64,
        String,
        String,
        bool,
        String,
        Option<String>,
    )> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Query point config: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    match result {
        Some((
            point_id,
            signal_name,
            scale,
            offset,
            unit,
            data_type,
            reverse,
            description,
            protocol_mappings_json,
        )) => {
            // Parse protocol_mappings JSON if present
            let protocol_mapping = if let Some(json_str) = protocol_mappings_json.as_ref() {
                if !json_str.trim().is_empty() {
                    match serde_json::from_str::<serde_json::Value>(json_str) {
                        Ok(value) if !value.is_null() => Some(value),
                        Ok(_) => None,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to parse protocol_mappings JSON for point {}: {}",
                                point_id,
                                e
                            );
                            None
                        },
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let point_def = crate::dto::PointDefinition {
                point_id,
                signal_name,
                scale,
                offset,
                unit,
                data_type,
                reverse,
                description,
                protocol_mapping,
            };

            Ok(Json(SuccessResponse::new(point_def)))
        },
        None => Err(AppError::not_found(format!(
            "Point {} (type {}) not found in channel {}",
            point_id, point_type_upper, channel_id
        ))),
    }
}

// ============================================================================
// Unmapped Points Query Handler
// ============================================================================

/// Get unmapped points for a channel (points without protocol_mappings)
///
/// This endpoint returns points that haven't been configured with protocol mappings yet.
/// Useful for preventing duplicate mappings during configuration operations.
///
/// **Unmapped Definition**: Points where `protocol_mappings IS NULL OR '' OR '{}' OR 'null'`
///
/// @route GET /api/channels/{id}/unmapped-points
/// @input Path(channel_id): u16 - Channel ID
/// @input Query(params): type filter (T/S/C/A, optional)
/// @output `Json<ApiResponse<GroupedPoints>>` - Unmapped points grouped by type
/// @status 200 - Unmapped points retrieved successfully
/// @status 404 - Channel not found
/// @status 500 - Database error
#[utoipa::path(
    get,
    path = "/api/channels/{id}/unmapped-points",
    params(
        ("id" = u32, Path, description = "Channel identifier"),
        ("type" = Option<String>, Query, description = "Point type filter: T (telemetry), S (signal), C (control), A (adjustment)")
    ),
    responses(
        (status = 200, description = "Unmapped points retrieved (grouped by type)", body = crate::dto::GroupedPoints,
            example = json!({
                "success": true,
                "data": {
                    "telemetry": [
                        {
                            "point_id": 101,
                            "signal_name": "DC_Voltage",
                            "scale": 0.1,
                            "offset": 0.0,
                            "unit": "V",
                            "data_type": "uint16",
                            "reverse": false,
                            "description": "DC bus voltage",
                            "protocol_mapping": null
                        }
                    ],
                    "signal": [],
                    "control": [],
                    "adjustment": []
                }
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn get_unmapped_points_handler<R: Rtdb>(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::GroupedPoints>>, AppError> {
    // 1. Verify channel exists
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // 2. Get point type filter from query params
    let point_type = params.get("type").map(|s| s.as_str());

    // 3. Build grouped point lists based on filter
    let mut telemetry_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut signal_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut control_points: Vec<crate::dto::PointDefinition> = Vec::new();
    let mut adjustment_points: Vec<crate::dto::PointDefinition> = Vec::new();

    // Helper function to fetch unmapped points from a table
    async fn fetch_unmapped_points_from_table(
        pool: &sqlx::SqlitePool,
        table: &str,
        channel_id: i64,
    ) -> Result<Vec<crate::dto::PointDefinition>, sqlx::Error> {
        // Build per-table SELECT with normalized columns to a common shape
        // Filter for unmapped points: protocol_mappings IS NULL OR '' OR '{}' OR 'null'
        let query = match table {
            "telemetry_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings
                 FROM {}
                 WHERE channel_id = ?
                 AND (protocol_mappings IS NULL
                      OR protocol_mappings = ''
                      OR protocol_mappings = '{{}}'
                      OR protocol_mappings = 'null')
                 ORDER BY point_id",
                table
            ),
            "signal_points" => format!(
                "SELECT point_id, signal_name, 1.0 AS scale, 0.0 AS offset, unit, data_type, reverse, description, protocol_mappings
                 FROM {}
                 WHERE channel_id = ?
                 AND (protocol_mappings IS NULL
                      OR protocol_mappings = ''
                      OR protocol_mappings = '{{}}'
                      OR protocol_mappings = 'null')
                 ORDER BY point_id",
                table
            ),
            "control_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings
                 FROM {}
                 WHERE channel_id = ?
                 AND (protocol_mappings IS NULL
                      OR protocol_mappings = ''
                      OR protocol_mappings = '{{}}'
                      OR protocol_mappings = 'null')
                 ORDER BY point_id",
                table
            ),
            "adjustment_points" => format!(
                "SELECT point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings
                 FROM {}
                 WHERE channel_id = ?
                 AND (protocol_mappings IS NULL
                      OR protocol_mappings = ''
                      OR protocol_mappings = '{{}}'
                      OR protocol_mappings = 'null')
                 ORDER BY point_id",
                table
            ),
            _ => format!(
                "SELECT point_id, signal_name, 1.0 AS scale, 0.0 AS offset, unit, data_type, 0 AS reverse, description, protocol_mappings
                 FROM {}
                 WHERE channel_id = ?
                 AND (protocol_mappings IS NULL
                      OR protocol_mappings = ''
                      OR protocol_mappings = '{{}}'
                      OR protocol_mappings = 'null')
                 ORDER BY point_id",
                table
            ),
        };

        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            u32,
            String,
            f64,
            f64,
            String,
            String,
            bool,
            String,
            Option<String>,
        )> = sqlx::query_as(&query)
            .bind(channel_id)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    point_id,
                    signal_name,
                    scale,
                    offset,
                    unit,
                    data_type,
                    reverse,
                    description,
                    _protocol_mappings_json,
                )| {
                    // For unmapped points, protocol_mapping is always None
                    crate::dto::PointDefinition {
                        point_id,
                        signal_name,
                        scale,
                        offset,
                        unit,
                        data_type,
                        reverse,
                        description,
                        protocol_mapping: None,
                    }
                },
            )
            .collect())
    }

    let channel_id_i64 = channel_id as i64;

    match point_type {
        Some("T") | Some("t") => {
            telemetry_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "telemetry_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped T: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some("S") | Some("s") => {
            signal_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "signal_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped S: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some("C") | Some("c") => {
            control_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "control_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped C: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some("A") | Some("a") => {
            adjustment_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "adjustment_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped A: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
        Some(invalid) => {
            return Err(AppError::bad_request(format!(
                "Invalid point type filter '{}'. Must be T, S, C, or A",
                invalid
            )));
        },
        None => {
            // Fetch all types
            telemetry_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "telemetry_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped T: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
            signal_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "signal_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped S: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
            control_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "control_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped C: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
            adjustment_points = fetch_unmapped_points_from_table(
                &state.sqlite_pool,
                "adjustment_points",
                channel_id_i64,
            )
            .await
            .map_err(|e| {
                tracing::error!("Fetch unmapped A: {}", e);
                AppError::internal_error("Database operation failed")
            })?;
        },
    }

    let grouped = crate::dto::GroupedPoints {
        telemetry: telemetry_points,
        signal: signal_points,
        control: control_points,
        adjustment: adjustment_points,
    };

    Ok(Json(SuccessResponse::new(grouped)))
}

// ============================================================================
// Type-specific GET wrapper handlers (delegate to *_inner functions)
// ============================================================================

/// Get telemetry point configuration
pub async fn get_telemetry_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "T", point_id, state).await
}

/// Get signal point configuration
pub async fn get_signal_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "S", point_id, state).await
}

/// Get control point configuration
pub async fn get_control_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "C", point_id, state).await
}

/// Get adjustment point configuration
pub async fn get_adjustment_point_config_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler_inner(channel_id, "A", point_id, state).await
}

// ============================================================================
// Tests for cache priority read
// ============================================================================

// VecRtdb cache tests removed - now using two-tier architecture (SharedMemory -> Redis)
#[cfg(test)]
mod cache_tests {
    use super::*;
    use axum::extract::{Path, State};
    use std::sync::Arc;
    use voltage_rtdb::{Bytes, MemoryRtdb};

    use crate::api::routes::AppState;
    use crate::core::channels::ChannelManager;
    use voltage_routing::RoutingCache;

    /// Helper: Create in-memory SQLite pool for testing
    async fn create_test_sqlite_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        common::test_utils::schema::init_comsrv_schema(&pool)
            .await
            .unwrap();

        pool
    }

    /// Helper: Create test AppState (removed VecRtdb)
    async fn create_test_state(rtdb: Arc<MemoryRtdb>) -> AppState<MemoryRtdb> {
        let sqlite_pool = create_test_sqlite_pool().await;
        let routing_cache = Arc::new(RoutingCache::default());
        let channel_manager = Arc::new(ChannelManager::new(rtdb.clone(), routing_cache));
        let command_tx_cache = Arc::new(crate::api::command_cache::CommandTxCache::new());

        AppState {
            channel_manager,
            rtdb,
            sqlite_pool,
            command_tx_cache,
        }
    }

    /// Test: Read point info from Redis
    #[tokio::test]
    async fn test_get_point_info_from_redis() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let channel_id: u32 = 1;
        let point_id: u32 = 102;

        // Populate Redis (MemoryRtdb) with data
        let keyspace = KeySpaceConfig::production_cached();
        let data_key = keyspace.channel_key(channel_id, PointType::Telemetry);
        let ts_key = keyspace.channel_ts_key(channel_id, PointType::Telemetry);
        let raw_key = keyspace.channel_raw_key(channel_id, PointType::Telemetry);
        let field = point_id.to_string();

        rtdb.hash_set(&data_key, &field, Bytes::from("750.0"))
            .await
            .unwrap();
        rtdb.hash_set(&ts_key, &field, Bytes::from("1729001000"))
            .await
            .unwrap();
        rtdb.hash_set(&raw_key, &field, Bytes::from("7500"))
            .await
            .unwrap();

        let state = create_test_state(rtdb).await;

        // Call handler
        let result =
            get_point_info_handler(State(state), Path((channel_id, "T".to_string(), point_id)))
                .await;

        // Verify: source should be "redis"
        let response = result.expect("Handler should succeed");
        let data = &response.0.data;
        assert_eq!(data["source"], "redis");
        assert_eq!(data["value"], "750.0");
    }

    /// Test: Invalid telemetry type returns 400
    #[tokio::test]
    async fn test_get_point_info_invalid_type() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let state = create_test_state(rtdb).await;

        // Call handler with invalid type
        let result = get_point_info_handler(
            State(state),
            Path((1, "X".to_string(), 100)), // "X" is invalid
        )
        .await;

        // Verify: should return error
        let err = result.expect_err("Should return error for invalid type");
        assert!(
            format!("{:?}", err).contains("Invalid telemetry type"),
            "Error should mention invalid type"
        );
    }
}
