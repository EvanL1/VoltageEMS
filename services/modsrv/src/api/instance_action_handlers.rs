//! Instance Action API Handlers
//!
//! Handles action execution on model instances.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::json;
use std::sync::Arc;
use voltage_config::api::SuccessResponse;

use crate::app_state::AppState;
use crate::dto::ActionRequest;
use crate::error::ModSrvError;

/// Execute an action on an instance
///
/// Triggers an action point with the specified value.
///
/// @route POST /api/instances/{id}/action
/// @input Path(id): u16 - Instance ID
/// @input Json(req): ActionRequest - Action details
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Execution result
/// @status 200 - Success confirmation
/// @status 404 - Instance or action not found
/// @status 500 - Database error
/// @side-effects Writes to Redis action keys and may trigger downstream routing
#[utoipa::path(
    post,
    path = "/api/instances/{id}/action",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = crate::dto::ActionRequest,
    responses(
        (status = 200, description = "Action executed", body = serde_json::Value,
            example = json!({
                "message": "Action executed"
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn execute_instance_action(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
    Json(req): Json<ActionRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state
        .instance_manager
        .execute_action(id, &req.point_id, req.value)
        .await
    {
        Ok(_) => Ok(Json(SuccessResponse::new(json!({
            "message": "Action executed",
            "instance_id": id,
            "point_id": req.point_id,
            "value": req.value
        })))),
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("not found") {
                Err(ModSrvError::InternalError(format!(
                    "Not found: Instance {} or action point '{}' not found",
                    id, req.point_id
                )))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to execute action: {}",
                    e
                )))
            }
        },
    }
}
