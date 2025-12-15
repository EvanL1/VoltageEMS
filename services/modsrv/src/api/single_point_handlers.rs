//! Single Point Routing API Handlers
//!
//! Provides RESTful API endpoints for managing routing of individual points.
//! Supports separate paths for measurement points and action points.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use common::SuccessResponse;
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::dto::{SinglePointRoutingRequest, ToggleRoutingRequest};
use crate::error::ModSrvError;

// ============================================================================
// Measurement Point Handlers
// ============================================================================

/// Get a single measurement point with routing configuration
#[utoipa::path(
    get,
    path = "/api/instances/{id}/measurements/{point_id}",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    responses(
        (status = 200, description = "Measurement point with routing", body = crate::dto::InstanceMeasurementPoint),
        (status = 404, description = "Instance or point not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn get_measurement_point(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<crate::dto::InstanceMeasurementPoint>>, ModSrvError> {
    match state
        .instance_manager
        .load_single_measurement_point(id, point_id)
        .await
    {
        Ok(point) => Ok(Json(SuccessResponse::new(point))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to load measurement point: {}",
            e
        ))),
    }
}

/// Create or update routing for a single measurement point
#[utoipa::path(
    put,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    request_body = crate::dto::SinglePointRoutingRequest,
    responses(
        (status = 200, description = "Routing created/updated", body = serde_json::Value,
            example = json!({"message": "Routing updated for measurement point 101"})
        ),
        (status = 400, description = "Invalid routing configuration"),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn upsert_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<SinglePointRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Upsert routing in database
    state
        .instance_manager
        .upsert_measurement_routing(id, point_id, request)
        .await
        .map_err(|e| ModSrvError::InvalidData(format!("Failed to upsert routing: {}", e)))?;

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after upsert measurement routing: {}",
            e
        );
    }

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing updated for measurement point {}", point_id)
    }))))
}

/// Delete routing for a single measurement point
#[utoipa::path(
    delete,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    responses(
        (status = 200, description = "Routing deleted", body = serde_json::Value,
            example = json!({"message": "Routing deleted for measurement point 101", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn delete_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Delete routing from database
    let rows_affected = state
        .instance_manager
        .delete_measurement_routing(id, point_id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to delete routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for measurement point {} in instance {}",
            point_id, id
        )));
    }

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after delete measurement routing: {}",
            e
        );
    }

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing deleted for measurement point {}", point_id),
        "rows_affected": rows_affected
    }))))
}

/// Toggle enabled state for a single measurement point routing
#[utoipa::path(
    patch,
    path = "/api/instances/{id}/measurements/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Measurement point ID")
    ),
    request_body = crate::dto::ToggleRoutingRequest,
    responses(
        (status = 200, description = "Routing enabled/disabled", body = serde_json::Value,
            example = json!({"message": "Routing enabled for measurement point 101", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn toggle_measurement_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<ToggleRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Toggle routing in database
    let rows_affected = state
        .instance_manager
        .toggle_measurement_routing(id, point_id, request.enabled)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to toggle routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for measurement point {} in instance {}",
            point_id, id
        )));
    }

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after toggle measurement routing: {}",
            e
        );
    }

    let action = if request.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing {} for measurement point {}", action, point_id),
        "rows_affected": rows_affected
    }))))
}

// ============================================================================
// Action Point Handlers
// ============================================================================

/// Get a single action point with routing configuration
#[utoipa::path(
    get,
    path = "/api/instances/{id}/actions/{point_id}",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    responses(
        (status = 200, description = "Action point with routing", body = crate::dto::InstanceActionPoint),
        (status = 404, description = "Instance or point not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn get_action_point(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<crate::dto::InstanceActionPoint>>, ModSrvError> {
    match state
        .instance_manager
        .load_single_action_point(id, point_id)
        .await
    {
        Ok(point) => Ok(Json(SuccessResponse::new(point))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to load action point: {}",
            e
        ))),
    }
}

/// Create or update routing for a single action point
#[utoipa::path(
    put,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    request_body = crate::dto::SinglePointRoutingRequest,
    responses(
        (status = 200, description = "Routing created/updated", body = serde_json::Value,
            example = json!({"message": "Routing updated for action point 201"})
        ),
        (status = 400, description = "Invalid routing configuration"),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn upsert_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<SinglePointRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Upsert routing in database
    state
        .instance_manager
        .upsert_action_routing(id, point_id, request)
        .await
        .map_err(|e| ModSrvError::InvalidData(format!("Failed to upsert routing: {}", e)))?;

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after upsert action routing: {}",
            e
        );
    }

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing updated for action point {}", point_id)
    }))))
}

/// Delete routing for a single action point
#[utoipa::path(
    delete,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    responses(
        (status = 200, description = "Routing deleted", body = serde_json::Value,
            example = json!({"message": "Routing deleted for action point 201", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn delete_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Delete routing from database
    let rows_affected = state
        .instance_manager
        .delete_action_routing(id, point_id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to delete routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for action point {} in instance {}",
            point_id, id
        )));
    }

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after delete action routing: {}",
            e
        );
    }

    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing deleted for action point {}", point_id),
        "rows_affected": rows_affected
    }))))
}

/// Toggle enabled state for a single action point routing
#[utoipa::path(
    patch,
    path = "/api/instances/{id}/actions/{point_id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID"),
        ("point_id" = u32, Path, description = "Action point ID")
    ),
    request_body = crate::dto::ToggleRoutingRequest,
    responses(
        (status = 200, description = "Routing enabled/disabled", body = serde_json::Value,
            example = json!({"message": "Routing enabled for action point 201", "rows_affected": 1})
        ),
        (status = 404, description = "Instance or routing not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn toggle_action_routing(
    State(state): State<Arc<AppState>>,
    Path((id, point_id)): Path<(u32, u32)>,
    Json(request): Json<ToggleRoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Toggle routing in database
    let rows_affected = state
        .instance_manager
        .toggle_action_routing(id, point_id, request.enabled)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to toggle routing: {}", e)))?;

    if rows_affected == 0 {
        return Err(ModSrvError::InternalError(format!(
            "No routing found for action point {} in instance {}",
            point_id, id
        )));
    }

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        tracing::warn!(
            "Failed to refresh routing cache after toggle action routing: {}",
            e
        );
    }

    let action = if request.enabled {
        "enabled"
    } else {
        "disabled"
    };
    Ok(Json(SuccessResponse::new(json!({
        "message": format!("Routing {} for action point {}", action, point_id),
        "rows_affected": rows_affected
    }))))
}
