//! Point information and query handlers
//!
//! This module contains handlers for:
//! - Getting point information including values and timestamps
//! - Listing all points for a channel
//! - Getting point mapping details

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::api::routes::AppState;
use crate::core::config::ChannelRedisKeys;
use crate::dto::{AppError, SuccessResponse};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};

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
pub async fn get_point_info_handler(
    State(state): State<AppState>,
    Path((channel_id, telemetry_type, point_id)): Path<(u32, String, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, AppError> {
    let telemetry_type_upper = telemetry_type.to_ascii_uppercase();
    if !matches!(telemetry_type_upper.as_str(), "T" | "S" | "C" | "A") {
        return Err(AppError::bad_request(format!(
            "Invalid telemetry type '{}'. Must be T, S, C, or A",
            telemetry_type
        )));
    }

    let field = point_id.to_string();
    let data_key = ChannelRedisKeys::channel_data(channel_id, &telemetry_type_upper);
    let ts_key = format!("{}:ts", data_key);
    let raw_key = format!("{}:raw", data_key);

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
        "telemetry_type": telemetry_type_upper,
        "point_id": point_id,
        "value": value,
        "timestamp": timestamp,
        "raw": raw_value,
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
pub async fn get_channel_points_handler(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
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
pub async fn get_point_mapping_with_type_handler(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState>,
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

// ============================================================================
// Point CRUD Handlers (Create, Update, Delete)
// ============================================================================

use crate::core::config::TelemetryPoint;

/// Point CRUD operation result
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct PointCrudResult {
    #[schema(example = 1)]
    pub channel_id: u32,

    /// Point type: T (Telemetry), S (Signal), C (Control), A (Adjustment)
    #[schema(example = "T")]
    pub point_type: String,

    #[schema(example = 101)]
    pub point_id: u32,

    #[schema(example = "DC_Voltage")]
    pub signal_name: String,

    #[schema(example = "Point updated successfully")]
    pub message: String,
}

// ============================================================================
// Batch Point CRUD Data Structures
// ============================================================================

/// Batch point operations request
///
/// Supports creating, updating, and deleting multiple points in a single request.
/// All operations are optional - provide only the operations you need.
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PointBatchRequest {
    /// Points to create
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub create: Vec<PointBatchCreateItem>,

    /// Points to update
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub update: Vec<PointBatchUpdateItem>,

    /// Points to delete
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delete: Vec<PointBatchDeleteItem>,
}

/// Batch create operation item
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PointBatchCreateItem {
    /// Point type: T (Telemetry), S (Signal), C (Control), A (Adjustment)
    #[schema(example = "T")]
    pub point_type: String,

    /// Point identifier
    #[schema(example = 101)]
    pub point_id: u32,

    /// Force create mode: if true, use INSERT OR REPLACE (upsert), if false, fail on duplicate (default: false)
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,

    /// Point configuration data (structure varies by point type)
    pub data: serde_json::Value,
}

/// Batch update operation item
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PointBatchUpdateItem {
    /// Point type: T (Telemetry), S (Signal), C (Control), A (Adjustment)
    #[schema(example = "T")]
    pub point_type: String,

    /// Point identifier
    #[schema(example = 101)]
    pub point_id: u32,

    /// Fields to update (only provide fields you want to update)
    /// Same structure as PointUpdateRequest, wrapped in "data" for consistency with CREATE
    pub data: PointUpdateRequest,
}

/// Batch delete operation item
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PointBatchDeleteItem {
    /// Point type: T (Telemetry), S (Signal), C (Control), A (Adjustment)
    #[schema(example = "T")]
    pub point_type: String,

    /// Point identifier
    #[schema(example = 101)]
    pub point_id: u32,
}

/// Batch operation result
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PointBatchResult {
    /// Total number of operations requested
    #[schema(example = 10)]
    pub total_operations: usize,

    /// Number of successful operations
    #[schema(example = 8)]
    pub succeeded: usize,

    /// Number of failed operations
    #[schema(example = 2)]
    pub failed: usize,

    /// Statistics per operation type
    pub operation_stats: OperationStats,

    /// Details of failed operations
    pub errors: Vec<PointBatchError>,

    /// Processing duration in milliseconds
    #[schema(example = 250)]
    pub duration_ms: u64,
}

/// Operation statistics grouped by type
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct OperationStats {
    pub create: OperationStat,
    pub update: OperationStat,
    pub delete: OperationStat,
}

/// Statistics for a single operation type
#[derive(Debug, Default, serde::Serialize, utoipa::ToSchema)]
pub struct OperationStat {
    #[schema(example = 5)]
    pub total: usize,
    #[schema(example = 4)]
    pub succeeded: usize,
    #[schema(example = 1)]
    pub failed: usize,
}

/// Error details for a failed batch operation
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PointBatchError {
    /// Operation type: create, update, or delete
    #[schema(example = "create")]
    pub operation: String,

    /// Point type: T, S, C, or A
    #[schema(example = "T")]
    pub point_type: String,

    /// Point identifier
    #[schema(example = 101)]
    pub point_id: u32,

    /// Error message
    #[schema(example = "Point 101 already exists")]
    pub error: String,
}

// ----------------------------------------------------------------------------
// Create Point Handlers
// ----------------------------------------------------------------------------

/// Create a telemetry point (T)
///
/// @route POST /api/channels/{channel_id}/T/points/{point_id}
/// @input Path((channel_id, point_id)): (u16, u32) - Channel and point identifiers
/// @input Json(point): TelemetryPoint - Point configuration
/// @output `Json<ApiResponse<PointCrudResult>>` - Creation result
/// @status 201 - Point created successfully
/// @status 400 - Invalid request
/// @status 404 - Channel not found
/// @status 409 - Point ID already exists
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/T/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_telemetry_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Deserialize to TelemetryPoint
    let point: TelemetryPoint = serde_json::from_value(payload)
        .map_err(|e| AppError::bad_request(format!("Invalid request body: {}", e)))?;
    // Validate channel exists
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // Validate point_id matches path parameter
    if point.base.point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, point.base.point_id
        )));
    }

    // Validate point uniqueness
    validate_point_uniqueness(&state.sqlite_pool, channel_id, "telemetry_points", point_id).await?;

    // Insert point into database
    sqlx::query(
        "INSERT INTO telemetry_points
         (channel_id, point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point.base.point_id as i64)
    .bind(&point.base.signal_name)
    .bind(point.scale)
    .bind(point.offset)
    .bind(&point.base.unit)
    .bind(&point.data_type)
    .bind(point.reverse)
    .bind(&point.base.description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create T point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:T:{} created", channel_id, point_id);

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "T".to_string(),
        point_id,
        signal_name: point.base.signal_name.clone(),
        message: "Telemetry point created successfully".to_string(),
    })))
}

/// Create a signal point (S)
///
/// @route POST /api/channels/{channel_id}/S/points/{point_id}
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/S/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_signal_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?
        as u32;

    let signal_name = payload
        .get("signal_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("Missing field: signal_name"))?;

    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    if payload_point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, payload_point_id
        )));
    }

    validate_point_uniqueness(&state.sqlite_pool, channel_id, "signal_points", point_id).await?;

    // Extract all standard fields
    let scale = payload.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let offset = payload
        .get("offset")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let unit = payload.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let reverse = payload
        .get("reverse")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<bool>().ok())
                .or_else(|| v.as_bool())
        })
        .unwrap_or(false);
    let normal_state = payload
        .get("normal_state")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let data_type = payload
        .get("data_type")
        .and_then(|v| v.as_str())
        .unwrap_or("bool");
    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    sqlx::query(
        "INSERT INTO signal_points
         (channel_id, point_id, signal_name, scale, offset, unit, reverse, normal_state, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point_id as i64)
    .bind(signal_name)
    .bind(scale)
    .bind(offset)
    .bind(unit)
    .bind(reverse)
    .bind(normal_state)
    .bind(data_type)
    .bind(description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create S point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:S:{} created", channel_id, point_id);

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "S".to_string(),
        point_id,
        signal_name: signal_name.to_string(),
        message: "Signal point created successfully".to_string(),
    })))
}

/// Create a control point (C)
///
/// @route POST /api/channels/{channel_id}/C/points/{point_id}
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/C/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_control_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?
        as u32;

    let signal_name = payload
        .get("signal_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("Missing field: signal_name"))?;

    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    if payload_point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, payload_point_id
        )));
    }

    validate_point_uniqueness(&state.sqlite_pool, channel_id, "control_points", point_id).await?;

    // Extract all standard fields
    let scale = payload.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let offset = payload
        .get("offset")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let unit = payload.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let reverse = payload
        .get("reverse")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<bool>().ok())
                .or_else(|| v.as_bool())
        })
        .unwrap_or(false);
    let data_type = payload
        .get("data_type")
        .and_then(|v| v.as_str())
        .unwrap_or("bool");
    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    sqlx::query(
        "INSERT INTO control_points
         (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point_id as i64)
    .bind(signal_name)
    .bind(scale)
    .bind(offset)
    .bind(unit)
    .bind(reverse)
    .bind(data_type)
    .bind(description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create C point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:C:{} created", channel_id, point_id);

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "C".to_string(),
        point_id,
        signal_name: signal_name.to_string(),
        message: "Control point created successfully".to_string(),
    })))
}

/// Create an adjustment point (A)
///
/// @route POST /api/channels/{channel_id}/A/points/{point_id}
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/A/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after creation (default: true)")
    ),
    responses(
        (status = 201, description = "Point created", body = PointCrudResult),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Channel not found"),
        (status = 409, description = "Point ID already exists")
    ),
    tag = "comsrv"
)]
pub async fn create_adjustment_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?
        as u32;

    let signal_name = payload
        .get("signal_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("Missing field: signal_name"))?;

    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    if payload_point_id != point_id {
        return Err(AppError::bad_request(format!(
            "Point ID mismatch: path has {}, body has {}",
            point_id, payload_point_id
        )));
    }

    validate_point_uniqueness(
        &state.sqlite_pool,
        channel_id,
        "adjustment_points",
        point_id,
    )
    .await?;

    // Extract all standard fields
    let scale = payload.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let offset = payload
        .get("offset")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let unit = payload.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    let reverse = payload
        .get("reverse")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<bool>().ok())
                .or_else(|| v.as_bool())
        })
        .unwrap_or(false);
    let data_type = payload
        .get("data_type")
        .and_then(|v| v.as_str())
        .unwrap_or("int16");
    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    sqlx::query(
        "INSERT INTO adjustment_points
         (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(channel_id as i64)
    .bind(point_id as i64)
    .bind(signal_name)
    .bind(scale)
    .bind(offset)
    .bind(unit)
    .bind(reverse)
    .bind(data_type)
    .bind(description)
    .execute(&state.sqlite_pool)
    .await
    .map_err(|e| {
        tracing::error!("Create A point: {}", e);
        AppError::internal_error("Failed to create point")
    })?;

    tracing::debug!("Ch{}:A:{} created", channel_id, point_id);

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: "A".to_string(),
        point_id,
        signal_name: signal_name.to_string(),
        message: "Adjustment point created successfully".to_string(),
    })))
}

// ----------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------

/// Validate that a channel exists
async fn validate_channel_exists(pool: &sqlx::SqlitePool, channel_id: u32) -> Result<(), AppError> {
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
async fn validate_point_uniqueness(
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

// ----------------------------------------------------------------------------
// Update Point Handler (Universal for all types)
// ----------------------------------------------------------------------------

/// Update request for point fields (supports partial updates)
///
/// Only provide fields you want to update. Fields are type-specific:
/// - **Common**: signal_name, description, unit, reverse
/// - **T/A**: scale, offset, data_type (same fields for Telemetry and Adjustment)
/// - **C only**: control_type, on_value, off_value, pulse_duration_ms
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct PointUpdateRequest {
    /// Point signal name (all types)
    #[schema(example = "DC_Voltage")]
    pub signal_name: Option<String>,

    /// Point description (all types)
    #[schema(example = "DC bus voltage")]
    pub description: Option<String>,

    /// Measurement unit (all types)
    #[schema(example = "V")]
    pub unit: Option<String>,

    /// Scale factor for raw value conversion (T/A only)
    #[schema(example = 0.1)]
    pub scale: Option<f64>,

    /// Offset for raw value conversion (T/A only)
    #[schema(example = 0.0)]
    pub offset: Option<f64>,

    /// Data type: float32, int16, uint16, int32, uint32 (T/A only)
    #[schema(example = "float32")]
    pub data_type: Option<String>,

    /// Reverse logic (false=normal, true=inverted) (all types)
    #[schema(example = false)]
    pub reverse: Option<bool>,

    // ========== Control-specific fields (C only) ==========
    /// Control type: momentary or sustained (C only)
    #[schema(example = "momentary")]
    pub control_type: Option<String>,

    /// Value for ON state (C only)
    #[schema(example = 1)]
    pub on_value: Option<u16>,

    /// Value for OFF state (C only)
    #[schema(example = 0)]
    pub off_value: Option<u16>,

    /// Pulse duration in milliseconds for momentary control (C only)
    #[schema(example = 500)]
    pub pulse_duration_ms: Option<u32>,

    // ========== Legacy fields (not stored in database) ==========
    /// Minimum allowed value (DEPRECATED - not stored in database)
    #[schema(example = 0.0)]
    pub min_value: Option<f64>,

    /// Maximum allowed value (DEPRECATED - not stored in database)
    #[schema(example = 1000.0)]
    pub max_value: Option<f64>,

    /// Adjustment step size (DEPRECATED - not stored in database)
    #[schema(example = 0.5)]
    pub step: Option<f64>,
}

/// Update a point (supports all four types: T/S/C/A)
///
/// @route PUT /api/channels/{channel_id}/{type}/points/{point_id}
/// @input Path((channel_id, point_type, point_id)): (u16, String, u32) - Identifiers
/// @input Json(update): PointUpdateRequest - Fields to update
/// @output `Json<ApiResponse<PointCrudResult>>` - Update result
/// @status 200 - Point updated successfully
/// @status 400 - Invalid point type
/// @status 404 - Channel or point not found
#[utoipa::path(
    put,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after update (default: true)")
    ),
    request_body(
        content = PointUpdateRequest,
        description = "Update request for point fields (supports partial updates). Only provide fields you want to update.",
        examples(
            ("Telemetry (T)" = (
                summary = "Update telemetry point",
                description = "Common fields: signal_name, description, unit, scale, offset, data_type, reverse",
                value = json!({
                    "signal_name": "DC_Voltage",
                    "description": "DC bus voltage",
                    "unit": "V",
                    "scale": 0.1,
                    "offset": 0.0,
                    "data_type": "float32",
                    "reverse": false
                })
            )),
            ("Signal (S)" = (
                summary = "Update signal point",
                description = "Common fields: signal_name, description, reverse",
                value = json!({
                    "signal_name": "Grid_Connected",
                    "description": "Grid connection status",
                    "reverse": false
                })
            )),
            ("Control (C)" = (
                summary = "Update control point",
                description = "Control fields: signal_name, description, reverse, control_type, on_value, off_value, pulse_duration_ms",
                value = json!({
                    "signal_name": "Main_Breaker",
                    "description": "Main breaker control",
                    "control_type": "momentary",
                    "on_value": 1,
                    "off_value": 0,
                    "pulse_duration_ms": 500,
                    "reverse": false
                })
            )),
            ("Adjustment (A)" = (
                summary = "Update adjustment point",
                description = "Adjustment fields: signal_name, description, unit, scale, offset, data_type, reverse (same as Telemetry)",
                value = json!({
                    "signal_name": "Target_Power",
                    "description": "Target power setpoint",
                    "unit": "kW",
                    "scale": 1.0,
                    "offset": 0.0,
                    "data_type": "float32",
                    "reverse": false
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Point updated", body = PointCrudResult),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
pub async fn update_point_handler(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
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

    // Verify point exists
    let query = format!(
        "SELECT signal_name FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );
    let existing: Option<(String,)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Point check: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    if existing.is_none() {
        return Err(AppError::not_found(format!(
            "Point {} (type {}) not found in channel {}",
            point_id, point_type_upper, channel_id
        )));
    }

    // Build dynamic UPDATE query with all non-null fields
    let mut updates = Vec::new();

    if update.signal_name.is_some() {
        updates.push(format!(
            "signal_name = '{}'",
            update.signal_name.as_ref().unwrap().replace("'", "''")
        ));
    }
    if let Some(ref desc) = update.description {
        updates.push(format!("description = '{}'", desc.replace("'", "''")));
    }
    if let Some(ref u) = update.unit {
        updates.push(format!("unit = '{}'", u.replace("'", "''")));
    }
    if let Some(scale) = update.scale {
        updates.push(format!("scale = {}", scale));
    }
    if let Some(offset) = update.offset {
        updates.push(format!("offset = {}", offset));
    }
    if let Some(ref dt) = update.data_type {
        updates.push(format!("data_type = '{}'", dt.replace("'", "''")));
    }
    if let Some(reverse) = update.reverse {
        updates.push(format!("reverse = {}", if reverse { 1 } else { 0 }));
    }
    // Control-specific fields
    if let Some(ref ct) = update.control_type {
        updates.push(format!("control_type = '{}'", ct.replace("'", "''")));
    }
    if let Some(on_value) = update.on_value {
        updates.push(format!("on_value = {}", on_value));
    }
    if let Some(off_value) = update.off_value {
        updates.push(format!("off_value = {}", off_value));
    }
    if let Some(pulse) = update.pulse_duration_ms {
        updates.push(format!("pulse_duration_ms = {}", pulse));
    }
    // Adjustment-specific fields
    if let Some(min_value) = update.min_value {
        updates.push(format!("min_value = {}", min_value));
    }
    if let Some(max_value) = update.max_value {
        updates.push(format!("max_value = {}", max_value));
    }
    if let Some(step) = update.step {
        updates.push(format!("step = {}", step));
    }

    if updates.is_empty() {
        return Err(AppError::bad_request("No fields provided for update"));
    }

    let update_sql = format!(
        "UPDATE {} SET {} WHERE channel_id = {} AND point_id = {}",
        table,
        updates.join(", "),
        channel_id,
        point_id
    );

    tracing::debug!("UPDATE SQL: {}", update_sql);

    sqlx::query(&update_sql)
        .execute(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Update point: {}", e);
            AppError::internal_error("Failed to update point")
        })?;

    // Get updated signal_name for response
    let signal_name = update.signal_name.unwrap_or(existing.unwrap().0);

    tracing::debug!("Ch{}:{}:{} updated", channel_id, point_type_upper, point_id);

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: point_type_upper,
        point_id,
        signal_name,
        message: "Point updated successfully".to_string(),
    })))
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
pub async fn get_point_config_handler(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState>,
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
        _ => unreachable!(),
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

// ----------------------------------------------------------------------------
// Type-specific GET wrappers for literal route paths (T/S/C/A)
// ----------------------------------------------------------------------------

/// Get telemetry point configuration (wrapper for literal /T/ route)
pub async fn get_telemetry_point_config_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler(Path((channel_id, "T".to_string(), point_id)), State(state)).await
}

/// Get signal point configuration (wrapper for literal /S/ route)
pub async fn get_signal_point_config_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler(Path((channel_id, "S".to_string(), point_id)), State(state)).await
}

/// Get control point configuration (wrapper for literal /C/ route)
pub async fn get_control_point_config_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler(Path((channel_id, "C".to_string(), point_id)), State(state)).await
}

/// Get adjustment point configuration (wrapper for literal /A/ route)
pub async fn get_adjustment_point_config_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<crate::dto::PointDefinition>>, AppError> {
    get_point_config_handler(Path((channel_id, "A".to_string(), point_id)), State(state)).await
}

// ----------------------------------------------------------------------------
// Delete Point Handler
// ----------------------------------------------------------------------------

/// Delete a point
///
/// @route DELETE /api/channels/{channel_id}/{type}/points/{point_id}
/// @input Path((channel_id, point_type, point_id)): (u16, String, u32) - Identifiers
/// @output `Json<ApiResponse<PointCrudResult>>` - Deletion result
/// @status 200 - Point deleted successfully
/// @status 400 - Invalid point type
/// @status 404 - Channel or point not found
#[utoipa::path(
    delete,
    path = "/api/channels/{channel_id}/{type}/points/{point_id}",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("type" = String, Path, description = "Point type: T, S, C, or A"),
        ("point_id" = u32, Path, description = "Point identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after deletion (default: true)")
    ),
    responses(
        (status = 200, description = "Point deleted", body = PointCrudResult),
        (status = 400, description = "Invalid point type"),
        (status = 404, description = "Channel or point not found")
    ),
    tag = "comsrv"
)]
pub async fn delete_point_handler(
    Path((channel_id, point_type, point_id)): Path<(u32, String, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
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

    // Get point info before deletion (for response)
    let query = format!(
        "SELECT signal_name FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );
    let existing: Option<(String,)> = sqlx::query_as(&query)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Point check: {}", e);
            AppError::internal_error("Database operation failed")
        })?;

    if existing.is_none() {
        return Err(AppError::not_found(format!(
            "Point {} (type {}) not found in channel {}",
            point_id, point_type_upper, channel_id
        )));
    }

    let signal_name = existing.unwrap().0;

    // Delete point
    let delete_sql = format!(
        "DELETE FROM {} WHERE channel_id = ? AND point_id = ?",
        table
    );
    sqlx::query(&delete_sql)
        .bind(channel_id as i64)
        .bind(point_id as i64)
        .execute(&state.sqlite_pool)
        .await
        .map_err(|e| {
            tracing::error!("Delete point: {}", e);
            AppError::internal_error("Failed to delete point")
        })?;

    tracing::debug!("Ch{}:{}:{} deleted", channel_id, point_type_upper, point_id);

    // Clear Redis data for the deleted point
    // Redis structure: comsrv:{channel_id}:{point_type} (Hash) with fields:
    //   - {point_id} (value)
    //   - {point_id}:ts (timestamp)
    //   - {point_id}:raw (raw value)
    let redis_key = format!("comsrv:{}:{}", channel_id, point_type_upper);
    let fields_to_delete = vec![
        point_id.to_string(),
        format!("{}:ts", point_id),
        format!("{}:raw", point_id),
    ];

    for field in &fields_to_delete {
        match state.rtdb.hash_del(&redis_key, field).await {
            Ok(deleted) => {
                if deleted {
                    tracing::debug!(
                        "Cleared Redis field {} from {} for point {}",
                        field,
                        redis_key,
                        point_id
                    );
                }
            },
            Err(e) => {
                tracing::warn!("Redis del {}:{}: {}", redis_key, field, e);
            },
        }
    }

    tracing::debug!(
        "Ch{}:{}:{} Redis cleared",
        channel_id,
        point_type_upper,
        point_id
    );

    // Trigger auto-reload if enabled
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointCrudResult {
        channel_id,
        point_type: point_type_upper,
        point_id,
        signal_name,
        message: "Point deleted successfully".to_string(),
    })))
}

// ----------------------------------------------------------------------------
// Type-specific UPDATE wrappers for literal route paths (T/S/C/A)
// ----------------------------------------------------------------------------

/// Update telemetry point (wrapper for literal /T/ route)
pub async fn update_telemetry_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler(
        Path((channel_id, "T".to_string(), point_id)),
        State(state),
        Query(reload_query),
        Json(update),
    )
    .await
}

/// Update signal point (wrapper for literal /S/ route)
pub async fn update_signal_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler(
        Path((channel_id, "S".to_string(), point_id)),
        State(state),
        Query(reload_query),
        Json(update),
    )
    .await
}

/// Update control point (wrapper for literal /C/ route)
pub async fn update_control_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler(
        Path((channel_id, "C".to_string(), point_id)),
        State(state),
        Query(reload_query),
        Json(update),
    )
    .await
}

/// Update adjustment point (wrapper for literal /A/ route)
pub async fn update_adjustment_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler(
        Path((channel_id, "A".to_string(), point_id)),
        State(state),
        Query(reload_query),
        Json(update),
    )
    .await
}

// ----------------------------------------------------------------------------
// Type-specific DELETE wrappers for literal route paths (T/S/C/A)
// ----------------------------------------------------------------------------

/// Delete telemetry point (wrapper for literal /T/ route)
pub async fn delete_telemetry_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler(
        Path((channel_id, "T".to_string(), point_id)),
        State(state),
        Query(reload_query),
    )
    .await
}

/// Delete signal point (wrapper for literal /S/ route)
pub async fn delete_signal_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler(
        Path((channel_id, "S".to_string(), point_id)),
        State(state),
        Query(reload_query),
    )
    .await
}

/// Delete control point (wrapper for literal /C/ route)
pub async fn delete_control_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler(
        Path((channel_id, "C".to_string(), point_id)),
        State(state),
        Query(reload_query),
    )
    .await
}

/// Delete adjustment point (wrapper for literal /A/ route)
pub async fn delete_adjustment_point_handler(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler(
        Path((channel_id, "A".to_string(), point_id)),
        State(state),
        Query(reload_query),
    )
    .await
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
pub async fn get_unmapped_points_handler(
    Path(channel_id): Path<u32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
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
// Batch Point CRUD Handler
// ============================================================================

/// Batch point operations (create, update, delete)
///
/// Process multiple point operations in a single request. Supports creating,
/// updating, and deleting points of any type (T/S/C/A). Operations are processed
/// independently - a single failure does not affect other operations.
///
/// @route POST /api/channels/{channel_id}/points/batch
/// @input Path(channel_id): u16 - Channel identifier
/// @input Json(request): PointBatchRequest - Batch operations request
/// @output `Json<SuccessResponse<PointBatchResult>>` - Batch operation results
/// @status 200 - Batch operation completed (may contain partial failures)
/// @status 400 - Invalid request (empty operations)
/// @status 404 - Channel not found
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/points/batch",
    params(
        ("channel_id" = u32, Path, description = "Channel identifier"),
        ("auto_reload" = bool, Query, description = "Auto-reload channel after batch operations (default: true)")
    ),
    request_body(
        content = PointBatchRequest,
        description = "Batch operations request. Provide create, update, and/or delete arrays.",
        examples(
            ("Mixed Operations" = (
                summary = "Create, update, and delete in one request",
                description = "Example showing all three operation types",
                value = json!({
                    "create": [
                        {
                            "point_type": "T",
                            "point_id": 101,
                            "data": {
                                "signal_name": "DC_Voltage",
                                "scale": 0.1,
                                "offset": 0.0,
                                "unit": "V",
                                "data_type": "float32",
                                "reverse": false,
                                "description": "DC bus voltage"
                            }
                        },
                        {
                            "point_type": "S",
                            "point_id": 201,
                            "data": {
                                "signal_name": "Grid_Connected",
                                "data_type": "bool",
                                "reverse": false,
                                "description": "Grid connection status"
                            }
                        }
                    ],
                    "update": [
                        {
                            "point_type": "T",
                            "point_id": 102,
                            "data": {
                                "signal_name": "DC_Current",
                                "scale": 0.01,
                                "description": "Updated DC current"
                                // Partial update: only these 3 fields updated, others unchanged
                            }
                        }
                    ],
                    "delete": [
                        {
                            "point_type": "A",
                            "point_id": 301
                        }
                    ]
                })
            )),
            ("Batch Create Only" = (
                summary = "Create multiple points",
                description = "Batch create telemetry points",
                value = json!({
                    "create": [
                        {
                            "point_type": "T",
                            "point_id": 103,
                            "data": {
                                "signal_name": "Temperature_1",
                                "scale": 0.1,
                                "offset": -40.0,
                                "unit": "°C",
                                "data_type": "int16",
                                "description": "Temperature sensor 1"
                            }
                        },
                        {
                            "point_type": "T",
                            "point_id": 104,
                            "data": {
                                "signal_name": "Temperature_2",
                                "scale": 0.1,
                                "offset": -40.0,
                                "unit": "°C",
                                "data_type": "int16",
                                "description": "Temperature sensor 2"
                            }
                        }
                    ]
                })
            )),
            ("Batch Update Only" = (
                summary = "Update multiple points (partial update supported)",
                description = "Batch update point configurations. **Only provide fields you want to update** - other fields remain unchanged. This example shows updating only 2 fields for point 101, and only 1 field for point 102.",
                value = json!({
                    "update": [
                        {
                            "point_type": "T",
                            "point_id": 101,
                            "scale": 0.2,              // Only update scale
                            "description": "Updated description"  // Only update description
                            // Other fields (unit, offset, data_type, etc.) remain unchanged
                        },
                        {
                            "point_type": "T",
                            "point_id": 102,
                            "unit": "kW"              // Only update unit, all other fields unchanged
                        }
                    ]
                })
            )),
            ("Batch Delete Only" = (
                summary = "Delete multiple points",
                description = "Batch delete obsolete points",
                value = json!({
                    "delete": [
                        {
                            "point_type": "A",
                            "point_id": 301
                        },
                        {
                            "point_type": "A",
                            "point_id": 302
                        }
                    ]
                })
            )),
            ("Force Create (UPSERT)" = (
                summary = "Force create with INSERT OR REPLACE behavior",
                description = "Use force=true to enable UPSERT mode: if point exists, it will be replaced; if not, it will be created. This is useful for batch imports where you want to ensure the data matches exactly what you provide, regardless of existing state. **Default behavior (force=false)**: CREATE fails if point already exists.",
                value = json!({
                    "create": [
                        {
                            "point_type": "T",
                            "point_id": 105,
                            "force": false,  // Default mode: fail if point 105 exists
                            "data": {
                                "signal_name": "Voltage_L1",
                                "scale": 0.1,
                                "offset": 0.0,
                                "unit": "V",
                                "data_type": "float32",
                                "reverse": false,
                                "description": "Phase L1 voltage"
                            }
                        },
                        {
                            "point_type": "T",
                            "point_id": 106,
                            "force": true,   // UPSERT mode: replace if exists, create if not
                            "data": {
                                "signal_name": "Voltage_L2",
                                "scale": 0.1,
                                "offset": 0.0,
                                "unit": "V",
                                "data_type": "float32",
                                "reverse": false,
                                "description": "Phase L2 voltage (will replace existing config if any)"
                            }
                        }
                    ]
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Batch operation completed", body = PointBatchResult,
            example = json!({
                "success": true,
                "data": {
                    "total_operations": 4,
                    "succeeded": 3,
                    "failed": 1,
                    "operation_stats": {
                        "create": {
                            "total": 2,
                            "succeeded": 1,
                            "failed": 1
                        },
                        "update": {
                            "total": 1,
                            "succeeded": 1,
                            "failed": 0
                        },
                        "delete": {
                            "total": 1,
                            "succeeded": 1,
                            "failed": 0
                        }
                    },
                    "errors": [
                        {
                            "operation": "create",
                            "point_type": "S",
                            "point_id": 201,
                            "error": "Point 201 already exists"
                        }
                    ],
                    "duration_ms": 145
                }
            })
        ),
        (status = 400, description = "Invalid request (empty operations)"),
        (status = 404, description = "Channel not found")
    ),
    tag = "comsrv"
)]
pub async fn batch_point_operations_handler(
    Path(channel_id): Path<u32>,
    State(state): State<AppState>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(request): Json<PointBatchRequest>,
) -> Result<Json<SuccessResponse<PointBatchResult>>, AppError> {
    use std::time::Instant;
    let start_time = Instant::now();

    // Validate at least one operation is provided
    if request.create.is_empty() && request.update.is_empty() && request.delete.is_empty() {
        return Err(AppError::bad_request(
            "At least one operation (create/update/delete) must be provided",
        ));
    }

    // Validate channel exists (fail fast for invalid channel)
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // Initialize statistics
    let mut create_stat = OperationStat::default();
    let mut update_stat = OperationStat::default();
    let mut delete_stat = OperationStat::default();
    let mut errors = Vec::new();

    // Process operations in order: DELETE → CREATE → UPDATE
    // This order prevents ID conflicts when replacing a point (delete old, create new)

    // 1. Process DELETE operations first (free up IDs for potential re-creation)
    delete_stat.total = request.delete.len();
    for item in request.delete {
        match process_delete_operation(channel_id, &item, &state).await {
            Ok(_) => delete_stat.succeeded += 1,
            Err(e) => {
                delete_stat.failed += 1;
                errors.push(PointBatchError {
                    operation: "delete".to_string(),
                    point_type: item.point_type.to_uppercase(),
                    point_id: item.point_id,
                    error: e.to_string(),
                });
            },
        }
    }

    // 2. Process CREATE operations (can now use IDs freed by deletions)
    create_stat.total = request.create.len();
    for item in request.create {
        match process_create_operation(channel_id, &item, &state).await {
            Ok(_) => create_stat.succeeded += 1,
            Err(e) => {
                create_stat.failed += 1;
                errors.push(PointBatchError {
                    operation: "create".to_string(),
                    point_type: item.point_type.to_uppercase(),
                    point_id: item.point_id,
                    error: e.to_string(),
                });
            },
        }
    }

    // 3. Process UPDATE operations last (may reference newly created points)
    update_stat.total = request.update.len();
    for item in request.update {
        match process_update_operation(channel_id, &item, &state).await {
            Ok(_) => update_stat.succeeded += 1,
            Err(e) => {
                update_stat.failed += 1;
                errors.push(PointBatchError {
                    operation: "update".to_string(),
                    point_type: item.point_type.to_uppercase(),
                    point_id: item.point_id,
                    error: e.to_string(),
                });
            },
        }
    }

    let total_operations = create_stat.total + update_stat.total + delete_stat.total;
    let succeeded = create_stat.succeeded + update_stat.succeeded + delete_stat.succeeded;
    let failed = create_stat.failed + update_stat.failed + delete_stat.failed;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    tracing::debug!(
        "Ch{} batch: {}/{} ok ({}ms)",
        channel_id,
        succeeded,
        total_operations,
        duration_ms
    );

    // Trigger auto-reload if enabled (unified reload after all batch operations)
    trigger_channel_reload_if_needed(channel_id, &state, reload_query.auto_reload).await;

    Ok(Json(SuccessResponse::new(PointBatchResult {
        total_operations,
        succeeded,
        failed,
        operation_stats: OperationStats {
            create: create_stat,
            update: update_stat,
            delete: delete_stat,
        },
        errors,
        duration_ms,
    })))
}

// ----------------------------------------------------------------------------
// Batch Operation Helpers
// ----------------------------------------------------------------------------

/// Process single create operation
async fn process_create_operation(
    channel_id: u32,
    item: &PointBatchCreateItem,
    state: &AppState,
) -> Result<(), String> {
    use crate::core::config::{AdjustmentPoint, ControlPoint, SignalPoint, TelemetryPoint};

    let point_type_upper = item.point_type.to_ascii_uppercase();
    let table = match point_type_upper.as_str() {
        "T" => "telemetry_points",
        "S" => "signal_points",
        "C" => "control_points",
        "A" => "adjustment_points",
        _ => return Err(format!("Invalid point type '{}'", item.point_type)),
    };

    // Validate point uniqueness (skip if force=true for upsert behavior)
    if !item.force {
        let existing: Option<(i64,)> = sqlx::query_as(&format!(
            "SELECT point_id FROM {} WHERE channel_id = ? AND point_id = ?",
            table
        ))
        .bind(channel_id as i64)
        .bind(item.point_id as i64)
        .fetch_optional(&state.sqlite_pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if existing.is_some() {
            return Err(format!("Point {} already exists", item.point_id));
        }
    }

    // Inject point_id into data before deserialization (required by Point struct)
    let mut data_with_id = item.data.clone();
    if let Some(obj) = data_with_id.as_object_mut() {
        obj.insert("point_id".to_string(), serde_json::json!(item.point_id));
    }

    // Deserialize and insert based on point type
    match point_type_upper.as_str() {
        "T" => {
            let point: TelemetryPoint = serde_json::from_value(data_with_id.clone())
                .map_err(|e| format!("Invalid telemetry point data: {}", e))?;

            let sql = if item.force {
                "INSERT OR REPLACE INTO telemetry_points
                 (channel_id, point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            } else {
                "INSERT INTO telemetry_points
                 (channel_id, point_id, signal_name, scale, offset, unit, data_type, reverse, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            };

            sqlx::query(sql)
                .bind(channel_id as i64)
                .bind(item.point_id as i64)
                .bind(&point.base.signal_name)
                .bind(point.scale)
                .bind(point.offset)
                .bind(&point.base.unit)
                .bind(&point.data_type)
                .bind(point.reverse)
                .bind(&point.base.description)
                .execute(&state.sqlite_pool)
                .await
                .map_err(|e| format!("Failed to insert: {}", e))?;
        },
        "S" => {
            let point: SignalPoint = serde_json::from_value(data_with_id.clone())
                .map_err(|e| format!("Invalid signal point data: {}", e))?;

            let sql = if item.force {
                "INSERT OR REPLACE INTO signal_points
                 (channel_id, point_id, signal_name, unit, reverse, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, NULL)"
            } else {
                "INSERT INTO signal_points
                 (channel_id, point_id, signal_name, unit, reverse, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, NULL)"
            };

            sqlx::query(sql)
                .bind(channel_id as i64)
                .bind(item.point_id as i64)
                .bind(&point.base.signal_name)
                .bind(&point.base.unit)
                .bind(point.reverse)
                .bind(&point.base.description)
                .execute(&state.sqlite_pool)
                .await
                .map_err(|e| format!("Failed to insert: {}", e))?;
        },
        "C" => {
            let point: ControlPoint = serde_json::from_value(data_with_id.clone())
                .map_err(|e| format!("Invalid control point data: {}", e))?;

            // Note: control_points table has same schema as telemetry_points
            // ControlPoint's control-specific fields (control_type, on_value, etc.) are not persisted
            let sql = if item.force {
                "INSERT OR REPLACE INTO control_points
                 (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            } else {
                "INSERT INTO control_points
                 (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            };

            sqlx::query(sql)
                .bind(channel_id as i64)
                .bind(item.point_id as i64)
                .bind(&point.base.signal_name)
                .bind(1.0f64) // scale: default for control points
                .bind(0.0f64) // offset: default for control points
                .bind(&point.base.unit)
                .bind(point.reverse)
                .bind("bool") // data_type: default for control points
                .bind(&point.base.description)
                .execute(&state.sqlite_pool)
                .await
                .map_err(|e| format!("Failed to insert: {}", e))?;
        },
        "A" => {
            let point: AdjustmentPoint = serde_json::from_value(data_with_id.clone())
                .map_err(|e| format!("Invalid adjustment point data: {}", e))?;

            // Extract reverse from JSON (not in AdjustmentPoint struct)
            let reverse = data_with_id
                .get("reverse")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let sql = if item.force {
                "INSERT OR REPLACE INTO adjustment_points
                 (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            } else {
                "INSERT INTO adjustment_points
                 (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)"
            };

            sqlx::query(sql)
                .bind(channel_id as i64)
                .bind(item.point_id as i64)
                .bind(&point.base.signal_name)
                .bind(point.scale)
                .bind(point.offset)
                .bind(&point.base.unit)
                .bind(reverse)
                .bind(&point.data_type)
                .bind(&point.base.description)
                .execute(&state.sqlite_pool)
                .await
                .map_err(|e| format!("Failed to insert: {}", e))?;
        },
        _ => unreachable!(),
    }

    Ok(())
}

/// Process single update operation (reuse existing handler logic)
async fn process_update_operation(
    channel_id: u32,
    item: &PointBatchUpdateItem,
    state: &AppState,
) -> Result<(), String> {
    // Reuse the existing update_point_handler logic
    // Note: Batch operations always trigger auto-reload at the end
    let reload_query = crate::dto::AutoReloadQuery { auto_reload: false };
    update_point_handler(
        Path((channel_id, item.point_type.clone(), item.point_id)),
        State(state.clone()),
        Query(reload_query),
        Json(item.data.clone()),
    )
    .await
    .map(|_| ())
    .map_err(|e| format!("{:?}", e))
}

/// Process single delete operation (reuse existing handler logic)
async fn process_delete_operation(
    channel_id: u32,
    item: &PointBatchDeleteItem,
    state: &AppState,
) -> Result<(), String> {
    // Reuse the existing delete_point_handler logic
    // Note: Batch operations always trigger auto-reload at the end
    let reload_query = crate::dto::AutoReloadQuery { auto_reload: false };
    delete_point_handler(
        Path((channel_id, item.point_type.clone(), item.point_id)),
        State(state.clone()),
        Query(reload_query),
    )
    .await
    .map(|_| ())
    .map_err(|e| format!("{:?}", e))
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
pub(crate) async fn trigger_channel_reload_if_needed(
    channel_id: u32,
    state: &AppState,
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
    tokio::spawn(async move {
        if let Err(e) = perform_channel_reload(channel_id, &state_clone).await {
            tracing::error!("Ch{} reload: {}", channel_id, e);
        } else {
            tracing::debug!("Ch{} reloaded", channel_id);
        }
    });
}

/// Perform channel reload (load config from SQLite and hot-reload)
///
/// This is an internal helper function that performs the actual reload logic.
async fn perform_channel_reload(channel_id: u32, state: &AppState) -> anyhow::Result<()> {
    use crate::core::channels::channel_manager::ChannelManager;

    // 1. Load channel configuration from SQLite
    let config = ChannelManager::load_channel_from_db(&state.sqlite_pool, channel_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load channel config: {}", e))?;

    // 2. Remove old channel
    let manager = state.channel_manager.write().await;
    if let Err(e) = manager.remove_channel(channel_id).await {
        tracing::warn!("Ch{} remove: {}", channel_id, e);
    }
    drop(manager);

    // 3. Create new channel with updated config
    let manager = state.channel_manager.write().await;
    let channel_arc = manager
        .create_channel(std::sync::Arc::new(config))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create channel: {}", e))?;
    drop(manager);

    // 4. Connect in background (non-blocking)
    tokio::spawn(async move {
        let mut channel = channel_arc.write().await;
        match channel.connect().await {
            Ok(_) => tracing::debug!("Ch{} connected", channel_id),
            Err(e) => tracing::warn!("Ch{} connect: {}", channel_id, e),
        }
    });

    Ok(())
}
