#![allow(clippy::disallowed_methods)]

//! Single-point CRUD handlers (Create, Update, Delete)

use crate::api::routes::AppState;
use crate::core::config::TelemetryPoint;
use crate::dto::{AppError, SuccessResponse};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use voltage_model::{KeySpaceConfig, PointType};
use voltage_rtdb::Rtdb;

use super::point_helpers::{
    trigger_channel_reload_if_needed, validate_channel_exists, validate_point_uniqueness,
};
use super::point_types::{PointCrudResult, PointUpdateRequest};

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
pub async fn create_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
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
pub async fn create_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id_u64 = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?;
    let payload_point_id = u32::try_from(payload_point_id_u64).map_err(|_| {
        AppError::bad_request(format!("point_id {} out of range", payload_point_id_u64))
    })?;

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
pub async fn create_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id_u64 = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?;
    let payload_point_id = u32::try_from(payload_point_id_u64).map_err(|_| {
        AppError::bad_request(format!("point_id {} out of range", payload_point_id_u64))
    })?;

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
pub async fn create_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    // Extract standard fields from payload
    let payload_point_id_u64 = payload
        .get("point_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::bad_request("Missing field: point_id"))?;
    let payload_point_id = u32::try_from(payload_point_id_u64).map_err(|_| {
        AppError::bad_request(format!("point_id {} out of range", payload_point_id_u64))
    })?;

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
// Update Point Handler (Universal for all types)
// ----------------------------------------------------------------------------

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
/// Internal implementation for update_point_handler
///
/// Uses parameterized queries to prevent SQL injection.
/// Each point type has its own UPDATE statement due to different table schemas.
pub(super) async fn update_point_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
    reload_query: crate::dto::AutoReloadQuery,
    update: PointUpdateRequest,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    let point_type_upper = point_type.to_ascii_uppercase();

    // Validate channel exists
    validate_channel_exists(&state.sqlite_pool, channel_id).await?;

    // Check if any field is provided for update
    let has_update = update.signal_name.is_some()
        || update.description.is_some()
        || update.unit.is_some()
        || update.scale.is_some()
        || update.offset.is_some()
        || update.data_type.is_some()
        || update.reverse.is_some();

    if !has_update {
        return Err(AppError::bad_request("No fields provided for update"));
    }

    // Execute type-specific parameterized UPDATE query
    // Using COALESCE(?, column) pattern: NULL parameter keeps original value
    let signal_name: String = match point_type_upper.as_str() {
        "T" => {
            // Telemetry points: signal_name, description, unit, scale, offset, data_type, reverse
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE telemetry_points SET
                    signal_name = COALESCE(?, signal_name),
                    description = COALESCE(?, description),
                    unit = COALESCE(?, unit),
                    scale = COALESCE(?, scale),
                    offset = COALESCE(?, offset),
                    data_type = COALESCE(?, data_type),
                    reverse = COALESCE(?, reverse)
                WHERE channel_id = ? AND point_id = ?
                RETURNING signal_name",
            )
            .bind(update.signal_name.as_deref())
            .bind(update.description.as_deref())
            .bind(update.unit.as_deref())
            .bind(update.scale)
            .bind(update.offset)
            .bind(update.data_type.as_deref())
            .bind(update.reverse)
            .bind(channel_id as i64)
            .bind(point_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Update telemetry point: {}", e);
                AppError::internal_error("Failed to update point")
            })?;

            result.ok_or_else(|| {
                AppError::not_found(format!(
                    "Point {} (type T) not found in channel {}",
                    point_id, channel_id
                ))
            })?
        },
        "S" => {
            // Signal points: signal_name, description, unit, scale, offset, data_type, reverse, normal_state
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE signal_points SET
                    signal_name = COALESCE(?, signal_name),
                    description = COALESCE(?, description),
                    unit = COALESCE(?, unit),
                    scale = COALESCE(?, scale),
                    offset = COALESCE(?, offset),
                    data_type = COALESCE(?, data_type),
                    reverse = COALESCE(?, reverse)
                WHERE channel_id = ? AND point_id = ?
                RETURNING signal_name",
            )
            .bind(update.signal_name.as_deref())
            .bind(update.description.as_deref())
            .bind(update.unit.as_deref())
            .bind(update.scale)
            .bind(update.offset)
            .bind(update.data_type.as_deref())
            .bind(update.reverse)
            .bind(channel_id as i64)
            .bind(point_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Update signal point: {}", e);
                AppError::internal_error("Failed to update point")
            })?;

            result.ok_or_else(|| {
                AppError::not_found(format!(
                    "Point {} (type S) not found in channel {}",
                    point_id, channel_id
                ))
            })?
        },
        "C" => {
            // Control points: signal_name, description, unit, scale, offset, data_type, reverse
            // Note: control_type, on_value, off_value, pulse_duration_ms are not in current schema
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE control_points SET
                    signal_name = COALESCE(?, signal_name),
                    description = COALESCE(?, description),
                    unit = COALESCE(?, unit),
                    scale = COALESCE(?, scale),
                    offset = COALESCE(?, offset),
                    data_type = COALESCE(?, data_type),
                    reverse = COALESCE(?, reverse)
                WHERE channel_id = ? AND point_id = ?
                RETURNING signal_name",
            )
            .bind(update.signal_name.as_deref())
            .bind(update.description.as_deref())
            .bind(update.unit.as_deref())
            .bind(update.scale)
            .bind(update.offset)
            .bind(update.data_type.as_deref())
            .bind(update.reverse)
            .bind(channel_id as i64)
            .bind(point_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Update control point: {}", e);
                AppError::internal_error("Failed to update point")
            })?;

            result.ok_or_else(|| {
                AppError::not_found(format!(
                    "Point {} (type C) not found in channel {}",
                    point_id, channel_id
                ))
            })?
        },
        "A" => {
            // Adjustment points: signal_name, description, unit, scale, offset, data_type, reverse
            // Note: min_value, max_value, step are not in current schema
            let result = sqlx::query_scalar::<_, String>(
                "UPDATE adjustment_points SET
                    signal_name = COALESCE(?, signal_name),
                    description = COALESCE(?, description),
                    unit = COALESCE(?, unit),
                    scale = COALESCE(?, scale),
                    offset = COALESCE(?, offset),
                    data_type = COALESCE(?, data_type),
                    reverse = COALESCE(?, reverse)
                WHERE channel_id = ? AND point_id = ?
                RETURNING signal_name",
            )
            .bind(update.signal_name.as_deref())
            .bind(update.description.as_deref())
            .bind(update.unit.as_deref())
            .bind(update.scale)
            .bind(update.offset)
            .bind(update.data_type.as_deref())
            .bind(update.reverse)
            .bind(channel_id as i64)
            .bind(point_id as i64)
            .fetch_optional(&state.sqlite_pool)
            .await
            .map_err(|e| {
                tracing::error!("Update adjustment point: {}", e);
                AppError::internal_error("Failed to update point")
            })?;

            result.ok_or_else(|| {
                AppError::not_found(format!(
                    "Point {} (type A) not found in channel {}",
                    point_id, channel_id
                ))
            })?
        },
        _ => {
            return Err(AppError::bad_request(format!(
                "Invalid point type '{}'. Must be T, S, C, or A",
                point_type
            )));
        },
    };

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
/// Internal implementation for delete_point_handler
pub(super) async fn delete_point_handler_inner<R: Rtdb>(
    channel_id: u32,
    point_type: &str,
    point_id: u32,
    state: AppState<R>,
    reload_query: crate::dto::AutoReloadQuery,
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
    // point_type_upper was already validated above in the match statement
    let pt = PointType::from_str(&point_type_upper).expect("validated point type");
    let keyspace = KeySpaceConfig::production_cached();
    let redis_key = keyspace.channel_key(channel_id, pt);
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

// ============================================================================
// Type-specific wrapper handlers (delegate to *_inner functions)
// ============================================================================

// --- PUT wrappers ---

/// Update telemetry point
pub async fn update_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "T", point_id, state, reload_query, update).await
}

/// Update signal point
pub async fn update_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "S", point_id, state, reload_query, update).await
}

/// Update control point
pub async fn update_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "C", point_id, state, reload_query, update).await
}

/// Update adjustment point
pub async fn update_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
    Json(update): Json<PointUpdateRequest>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    update_point_handler_inner(channel_id, "A", point_id, state, reload_query, update).await
}

// --- DELETE wrappers ---

/// Delete telemetry point
pub async fn delete_telemetry_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "T", point_id, state, reload_query).await
}

/// Delete signal point
pub async fn delete_signal_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "S", point_id, state, reload_query).await
}

/// Delete control point
pub async fn delete_control_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "C", point_id, state, reload_query).await
}

/// Delete adjustment point
pub async fn delete_adjustment_point_handler<R: Rtdb>(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    State(state): State<AppState<R>>,
    Query(reload_query): Query<crate::dto::AutoReloadQuery>,
) -> Result<Json<SuccessResponse<PointCrudResult>>, AppError> {
    delete_point_handler_inner(channel_id, "A", point_id, state, reload_query).await
}
