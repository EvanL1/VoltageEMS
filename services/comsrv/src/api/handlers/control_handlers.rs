//! Control and command handlers for channel operations
//!
//! This module contains handlers for:
//! - Channel control operations (start, stop, restart)
//! - Point-level control commands
//! - Point-level adjustment commands
//! - Batch control and adjustment operations

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::api::routes::AppState;
use crate::dto::{AppError, ChannelOperation, SuccessResponse, WritePointRequest, WriteResponse};
use axum::{
    extract::{Path, State},
    response::Json,
};
use voltage_model::PointType;
use voltage_rtdb::KeySpaceConfig;
use voltage_rtdb::Rtdb;

/// Control channel operation (start/stop/restart)
///
/// @route POST /api/channels/{id}/control
/// @input State(state): AppState - Application state with factory
/// @input Path(id): String - Channel identifier
/// @input Json(operation): ChannelOperation - Operation to perform (start/stop/restart)
/// @output `Json<ApiResponse<String>>` - Operation result message
/// @status 200 - Operation completed successfully
/// @status 404 - Channel not found
/// @status 500 - Operation failed
#[utoipa::path(
    post,
    path = "/api/channels/{id}/control",
    params(
        ("id" = String, Path, description = "Channel identifier")
    ),
    request_body = crate::dto::ChannelOperation,
    responses(
        (status = 200, description = "Channel operation accepted", body = String,
            example = json!({
                "success": true,
                "data": "Channel 1 connected successfully"
            })
        )
    ),
    tag = "comsrv"
)]
pub async fn control_channel<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path(id): Path<String>,
    Json(operation): Json<ChannelOperation>,
) -> Result<Json<SuccessResponse<String>>, AppError> {
    let channel_id = id
        .parse::<u32>()
        .map_err(|_| AppError::bad_request(format!("Invalid channel ID format: {}", id)))?;
    let manager = state.channel_manager.read().await;

    // Check if channel exists and get the channel
    let Some(channel_impl) = manager.get_channel(channel_id) else {
        return Err(AppError::not_found(format!(
            "Channel {} not found",
            channel_id
        )));
    };

    // Execute operation based on type using ChannelImpl's unified interface
    match operation.operation.as_str() {
        "start" => {
            if let Err(e) = channel_impl.write().await.connect().await {
                tracing::error!("Ch{} connect: {}", channel_id, e);
                return Err(AppError::internal_error(format!(
                    "Failed to connect channel {}: {}",
                    channel_id, e
                )));
            }
            Ok(Json(SuccessResponse::new(format!(
                "Channel {channel_id} connected successfully"
            ))))
        },
        "stop" => {
            if let Err(e) = channel_impl.write().await.disconnect().await {
                tracing::error!("Ch{} disconnect: {}", channel_id, e);
                return Err(AppError::internal_error(format!(
                    "Failed to disconnect channel {}: {}",
                    channel_id, e
                )));
            }
            Ok(Json(SuccessResponse::new(format!(
                "Channel {channel_id} disconnected successfully"
            ))))
        },
        "restart" => {
            // First stop the channel
            if let Err(e) = channel_impl.write().await.disconnect().await {
                tracing::error!("Ch{} stop: {}", channel_id, e);
                return Err(AppError::internal_error(format!(
                    "Failed to stop channel {}: {}",
                    channel_id, e
                )));
            }

            // Wait a moment before starting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Then start it again
            if let Err(e) = channel_impl.write().await.connect().await {
                tracing::error!("Ch{} restart: {}", channel_id, e);
                return Err(AppError::internal_error(format!(
                    "Failed to restart channel {}: {}",
                    channel_id, e
                )));
            }
            Ok(Json(SuccessResponse::new(format!(
                "Channel {channel_id} restarted successfully"
            ))))
        },
        _ => Err(AppError::bad_request(format!(
            "Invalid operation: {}",
            operation.operation
        ))),
    }
}

/// Unified write point endpoint (supports all point types: T/S/C/A, single and batch)
///
/// This is the new unified API for writing values to channel points.
/// It automatically detects single vs batch operations and supports full type names.
///
/// ## Supported Point Types
/// - **T** / **Telemetry**: For testing/simulation (normally read-only)
/// - **S** / **Signal**: For testing/simulation (normally read-only)
/// - **C** / **Control**: Remote control commands (0/1 for on/off)
/// - **A** / **Adjustment**: Setpoint adjustments (floating point values)
///
/// ## Example Requests
///
/// **Single Control**:
/// ```json
/// POST /api/channels/1001/write
/// {
///   "type": "C",
///   "id": "101",
///   "value": 1.0
/// }
/// ```
///
/// **Single Adjustment** with full type name:
/// ```json
/// POST /api/channels/1001/write
/// {
///   "type": "Adjustment",
///   "id": "201",
///   "value": 4500.0
/// }
/// ```
///
/// **Batch Adjustment**:
/// ```json
/// POST /api/channels/1001/write
/// {
///   "type": "A",
///   "points": [
///     {"id": "201", "value": 4500.0},
///     {"id": "202", "value": 380.0}
///   ]
/// }
/// ```
///
/// @route POST /api/channels/{channel_id}/write
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/write",
    params(
        ("channel_id" = u16, Path, description = "Channel identifier", example = 1001)
    ),
    request_body = WritePointRequest,
    responses(
        (status = 200, description = "Write operation completed (single or batch)",
            body = WriteResponse),
        (status = 400, description = "Invalid point type or parameters", body = String),
        (status = 500, description = "Write operation failed", body = String)
    ),
    tag = "comsrv"
)]
pub async fn write_channel_point<R: Rtdb>(
    State(state): State<AppState<R>>,
    Path(channel_id): Path<u32>,
    Json(request): Json<WritePointRequest>,
) -> Result<Json<SuccessResponse<crate::dto::WriteResponse>>, AppError> {
    use crate::dto::{BatchCommandError, BatchCommandResult, WritePointData, WriteResponse};

    let rtdb = &state.rtdb;

    // Normalize point type: support both short (T/S/C/A) and full names (Telemetry/Signal/Control/Adjustment)
    let point_type = normalize_point_type(&request.r#type)?;

    // Handle single vs batch based on request data (use cached config to avoid allocation)
    let config = KeySpaceConfig::production_cached();

    match &request.data {
        WritePointData::Single { id, value } => {
            // Single point write using voltage-rtdb helper
            let point_id = id
                .parse::<u32>()
                .map_err(|_| AppError::bad_request(format!("Invalid point ID: {}", id)))?;

            let timestamp_ms = voltage_rtdb::helpers::write_point_auto_trigger(
                rtdb.as_ref(),
                config,
                channel_id,
                point_type,
                point_id,
                *value,
            )
            .await
            .map_err(|e| {
                tracing::error!("Write Ch{}:{:?}:{}: {}", channel_id, point_type, id, e);
                AppError::internal_error(format!("Failed to write point value: {}", e))
            })?;

            tracing::debug!(
                "Write Ch{}:{:?}:{} = {} @{}",
                channel_id,
                point_type,
                id,
                value,
                timestamp_ms
            );

            let response = crate::dto::WritePointResponse {
                channel_id,
                point_type: point_type.as_str().to_string(),
                point_id,
                value: *value,
                timestamp_ms,
            };

            Ok(Json(SuccessResponse::new(WriteResponse::Single(response))))
        },
        WritePointData::Batch { points } => {
            // Batch write using application layer function
            let mut errors = Vec::new();
            let total = points.len();
            let mut succeeded = 0;

            for point in points {
                // Parse point ID
                let point_id = match point.id.parse::<u32>() {
                    Ok(id) => id,
                    Err(_) => {
                        tracing::warn!("Invalid ID: Ch{}:{}:{}", channel_id, point_type, point.id);
                        errors.push(BatchCommandError {
                            point_id: 0,
                            error: format!("Invalid point ID: {}", point.id),
                        });
                        continue;
                    },
                };

                // Write point using voltage-rtdb helper
                match voltage_rtdb::helpers::write_point_auto_trigger(
                    rtdb.as_ref(),
                    config,
                    channel_id,
                    point_type,
                    point_id,
                    point.value,
                )
                .await
                {
                    Ok(_) => {
                        succeeded += 1;
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Write Ch{}:{:?}:{}: {}",
                            channel_id,
                            point_type,
                            point.id,
                            e
                        );
                        errors.push(BatchCommandError {
                            point_id,
                            error: format!("Failed to write: {}", e),
                        });
                    },
                }
            }

            tracing::debug!(
                "Batch Ch{}:{:?}: {}/{} ok",
                channel_id,
                point_type,
                succeeded,
                total
            );

            let result = BatchCommandResult {
                total,
                succeeded,
                failed: total - succeeded,
                errors,
            };

            Ok(Json(SuccessResponse::new(WriteResponse::Batch(result))))
        },
    }
}

/// Normalize point type from full name or short name to single letter
fn normalize_point_type(type_str: &str) -> Result<PointType, AppError> {
    match type_str {
        "T" | "t" | "Telemetry" | "telemetry" | "TELEMETRY" => Ok(PointType::Telemetry),
        "S" | "s" | "Signal" | "signal" | "SIGNAL" => Ok(PointType::Signal),
        "C" | "c" | "Control" | "control" | "CONTROL" => Ok(PointType::Control),
        "A" | "a" | "Adjustment" | "adjustment" | "ADJUSTMENT" => Ok(PointType::Adjustment),
        _ => Err(AppError::bad_request(format!(
            "Invalid point type '{}'. Must be one of: T/Telemetry, S/Signal, C/Control, A/Adjustment",
            type_str
        ))),
    }
}
