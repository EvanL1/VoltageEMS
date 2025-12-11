//! Instance Management API Handlers
//!
//! Handles CRUD operations and synchronization for model instances.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};
use voltage_config::api::SuccessResponse;
use voltage_config::modsrv::CreateInstanceRequest;

use crate::app_state::AppState;
use crate::dto::{ActionRequest, CreateInstanceDto, UpdateInstanceDto};
use crate::error::ModSrvError;

/// Create a new model instance
///
/// Creates an instance from a product template with optional property overrides.
///
/// @route POST /api/instances
/// @input Json(dto): CreateInstanceDto - Instance configuration
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Created instance details
/// @status 200 - Success with instance data
/// @status 400 - Invalid product_name or duplicate instance_name
/// @status 500 - Database error
/// @side-effects Creates instance in database and initializes Redis keys
#[utoipa::path(
    post,
    path = "/api/instances",
    request_body = crate::dto::CreateInstanceDto,
    responses(
        (status = 200, description = "Instance created", body = serde_json::Value,
            example = json!({
                "instance": {
                    "instance_id": 1,
                    "instance_name": "pv_inverter_01",
                    "product_name": "pv_inverter",
                    "properties": {
                        "rated_power": 5000.0,
                        "manufacturer": "Huawei",
                        "model": "SUN2000-5KTL-L1"
                    },
                    "created_at": "2025-10-15T10:30:00Z",
                    "updated_at": "2025-10-15T10:30:00Z"
                }
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn create_instance(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateInstanceDto>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Auto-generate instance_id if not provided
    let instance_id = if let Some(id) = dto.instance_id {
        id
    } else {
        // Get next available ID from database
        match state.instance_manager.get_next_instance_id().await {
            Ok(id) => id,
            Err(e) => {
                return Err(ModSrvError::InternalError(format!(
                    "Failed to generate instance ID: {}",
                    e
                )));
            },
        }
    };

    let req = CreateInstanceRequest {
        instance_id,
        instance_name: dto.instance_name,
        product_name: dto.product_name,
        properties: dto.properties.unwrap_or_default(),
    };

    match state.instance_manager.create_instance(req).await {
        Ok(instance) => Ok(Json(SuccessResponse::new(json!({
            "instance": instance
        })))),
        Err(e) => {
            // Check for specific error types with improved messages
            let error_msg = e.to_string();
            if error_msg.contains("already exists") {
                // Extract instance name from error message if possible
                let instance_name = error_msg.split('\'').nth(1).unwrap_or("unknown");
                Err(ModSrvError::InstanceExists(format!(
                    "Instance name '{}' is already in use. Please choose a different name.",
                    instance_name
                )))
            } else if error_msg.contains("UNIQUE constraint failed: instances.instance_name") {
                // Database-level unique constraint violation
                Err(ModSrvError::InstanceExists(
                    "Instance name must be unique. This name is already taken.".to_string(),
                ))
            } else if error_msg.contains("product") && error_msg.contains("not found") {
                Err(ModSrvError::InvalidData(format!(
                    "Invalid product_name: {}",
                    e
                )))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to create instance: {}",
                    e
                )))
            }
        },
    }
}

/// Update instance name and/or properties
///
/// Updates the instance_name and/or properties of an existing instance.
/// At least one field (instance_name or properties) must be provided.
///
/// @route PUT /api/instances/{id}
/// @input Path(id): u16 - Instance ID
/// @input Json(dto): UpdateInstanceDto - Fields to update
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Updated instance
/// @status 200 - Success with updated instance details
/// @status 400 - No fields to update or invalid request
/// @status 404 - Instance not found
/// @status 409 - Instance name already exists (conflict)
/// @status 500 - Database or Redis error
#[utoipa::path(
    put,
    path = "/api/instances/{id}",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = UpdateInstanceDto,
    responses(
        (status = 200, description = "Instance updated successfully", body = serde_json::Value,
            example = json!({
                "instance": {
                    "instance_id": 1,
                    "instance_name": "pv_inverter_renamed",
                    "product_name": "pv_inverter",
                    "properties": {
                        "rated_power": 5000.0,
                        "manufacturer": "Huawei",
                        "model": "SUN2000-5KTL-L1"
                    },
                    "created_at": "2025-10-15T10:30:00Z",
                    "updated_at": "2025-10-20T14:25:00Z"
                }
            })
        ),
        (status = 400, description = "No fields to update"),
        (status = 404, description = "Instance not found"),
        (status = 409, description = "Instance name already exists"),
        (status = 500, description = "Database or Redis error")
    ),
    tag = "modsrv"
)]
pub async fn update_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    Json(dto): Json<UpdateInstanceDto>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Validate: at least one field must be provided
    if dto.instance_name.is_none() && dto.properties.is_none() {
        return Err(ModSrvError::InvalidData(
            "At least one field (instance_name or properties) must be provided".to_string(),
        ));
    }

    // Query current instance_name for logging and Redis operations
    let old_instance_name: String =
        match sqlx::query_scalar("SELECT instance_name FROM instances WHERE instance_id = ?")
            .bind(id as i32)
            .fetch_one(&state.instance_manager.pool)
            .await
        {
            Ok(name) => name,
            Err(_) => return Err(ModSrvError::InstanceNotFound(id.to_string())),
        };

    // Determine the final instance name
    let new_instance_name = dto.instance_name.as_deref().unwrap_or(&old_instance_name);
    let is_renaming = dto.instance_name.is_some() && new_instance_name != old_instance_name;

    // Handle renaming
    if is_renaming {
        // Rename in SQLite (includes transaction)
        if let Err(e) = state
            .instance_manager
            .rename_instance(id, new_instance_name)
            .await
        {
            let error_msg = e.to_string();
            if error_msg.contains("already exists") {
                return Err(ModSrvError::InstanceExists(format!(
                    "Instance name '{}' already exists",
                    new_instance_name
                )));
            }
            return Err(ModSrvError::InternalError(format!(
                "Failed to rename instance: {}",
                e
            )));
        }

        // Rename in Redis (best effort)
        if let Err(e) = crate::redis_state::rename_instance_in_redis(
            state.instance_manager.rtdb.as_ref(),
            id,
            &old_instance_name,
            new_instance_name,
        )
        .await
        {
            warn!(
                "Instance {} renamed in SQLite but Redis sync failed: {}. Will sync on next reload.",
                id, e
            );
        }
    }

    // Handle properties update
    if let Some(ref properties) = dto.properties {
        let properties_json = match serde_json::to_string(properties) {
            Ok(j) => j,
            Err(e) => {
                return Err(ModSrvError::InternalError(format!(
                    "Failed to serialize properties: {}",
                    e
                )));
            },
        };

        // Update properties in SQLite
        let result = sqlx::query(
            r#"UPDATE instances SET properties = ?, updated_at = CURRENT_TIMESTAMP WHERE instance_id = ?"#,
        )
        .bind(&properties_json)
        .bind(id as i32)
        .execute(&state.instance_manager.pool)
        .await;

        if let Err(e) = result {
            error!("Failed to update properties for instance {}: {}", id, e);
            return Err(ModSrvError::InternalError(format!(
                "Database update failed: {}",
                e
            )));
        }

        // Sync properties to Redis (best effort)
        if let Err(e) = state
            .instance_manager
            .sync_instance_to_redis_internal(new_instance_name, properties)
            .await
        {
            warn!(
                "Instance {} properties updated in SQLite but Redis sync failed: {}. Will sync on next reload.",
                id, e
            );
        }
    }

    info!(
        "Instance {} updated successfully (renamed: {}, properties: {})",
        id,
        is_renaming,
        dto.properties.is_some()
    );

    // Query and return updated instance
    match state.instance_manager.get_instance(id).await {
        Ok(instance) => Ok(Json(SuccessResponse::new(json!({
            "instance": instance
        })))),
        Err(e) => {
            error!("Failed to query updated instance {}: {}", id, e);
            // Update succeeded but query failed - return id as fallback
            Ok(Json(SuccessResponse::new(json!({
                "instance_id": id,
                "instance_name": new_instance_name,
                "message": "Instance updated successfully but failed to retrieve details"
            }))))
        },
    }
}

/// Delete an instance
///
/// Removes an instance from both SQLite and Redis.
///
/// @route DELETE /api/instances/{id}
/// @input Path(id): u16 - Instance ID
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Deletion result
/// @status 200 - Success with deletion confirmation
/// @status 404 - Instance not found
/// @status 500 - Database error
#[utoipa::path(
    delete,
    path = "/api/instances/{id}",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    responses(
        (status = 200, description = "Instance deleted", body = serde_json::Value,
            example = json!({
                "message": "Instance 1 deleted"
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn delete_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.instance_manager.delete_instance(id).await {
        Ok(_) => Ok(Json(SuccessResponse::new(json!({
            "message": format!("Instance {} deleted", id)
        })))),
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("not found") {
                Err(ModSrvError::InstanceNotFound(id.to_string()))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to delete instance: {}",
                    e
                )))
            }
        },
    }
}

/// Sync measurement data to an instance
///
/// Updates measurement point values in Redis for the instance.
///
/// @route POST /api/instances/{id}/sync
/// @input Path(id): u16 - Instance ID
/// @input Json(data): HashMap - Measurement point values
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Sync result
/// @status 200 - Success confirmation
/// @status 404 - Instance not found
/// @status 500 - Database error
/// @side-effects Updates Redis measurement keys
#[utoipa::path(
    post,
    path = "/api/instances/{id}/sync",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = std::collections::HashMap<String, serde_json::Value>,
    responses(
        (status = 200, description = "Measurement synced", body = serde_json::Value,
            example = json!({
                "message": "Measurement synced"
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn sync_instance_measurement(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    Json(data): Json<HashMap<String, serde_json::Value>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.instance_manager.sync_measurement(id, data).await {
        Ok(_) => Ok(Json(SuccessResponse::new(json!({
            "message": "Measurement synced"
        })))),
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("not found") {
                Err(ModSrvError::InstanceNotFound(id.to_string()))
            } else {
                Err(ModSrvError::InternalError(format!(
                    "Failed to sync measurement: {}",
                    e
                )))
            }
        },
    }
}

/// Sync all instances to Redis
///
/// Reloads all instance configurations from SQLite to Redis.
///
/// @route POST /api/instances/sync/all
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Sync result
/// @status 200 - Success confirmation
/// @status 500 - Sync error
/// @side-effects Overwrites all instance keys in Redis
pub async fn sync_all_instances(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.instance_manager.sync_instances_to_redis().await {
        Ok(_) => Ok(Json(SuccessResponse::new(json!({
            "message": "All instances synced to Redis"
        })))),
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to sync instances: {}",
            e
        ))),
    }
}

/// Reload instances from database
///
/// Forces a reload of all instances from SQLite to Redis.
/// Useful after manual database updates.
///
/// @route POST /api/instances/reload
/// @output Result<Json<SuccessResponse<serde_json::Value>>, AppError> - Reload result
/// @status 200 - Success confirmation
/// @status 500 - Database error
pub async fn reload_instances_from_db(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Use unified ReloadableService interface for incremental sync
    use voltage_config::ReloadableService;
    match ReloadableService::reload_from_database(
        &*state.instance_manager,
        &state.instance_manager.pool,
    )
    .await
    {
        Ok(result) => {
            info!(
                "Instances reloaded: {} added, {} updated, {} removed, {} errors",
                result.added.len(),
                result.updated.len(),
                result.removed.len(),
                result.errors.len()
            );
            Ok(Json(SuccessResponse::new(json!({
                "message": "Instances reloaded successfully",
                "result": result
            }))))
        },
        Err(e) => {
            error!("Failed to reload instances: {}", e);
            Err(ModSrvError::InternalError(format!(
                "Failed to reload instances: {}",
                e
            )))
        },
    }
}

// ============================================================================
// Action Execution
// ============================================================================

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
    Path(id): Path<u32>,
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
