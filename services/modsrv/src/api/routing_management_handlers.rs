//! Instance Routing Management API Handlers
//!
//! This module provides API handlers for managing routing configurations.
//! It includes functions to create, update, delete, and validate routing
//! configurations between channels and model instances.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::json;
use std::sync::Arc;
use voltage_config::api::SuccessResponse;

use crate::app_state::AppState;
use crate::dto::RoutingRequest;
use crate::error::ModSrvError;
use crate::routing_loader::{ActionRoutingRow, MeasurementRoutingRow};

/// Create a new routing for an instance
///
/// Creates a new channel-to-instance point routing. Validates that both
/// the channel and instance points exist before creating.
///
/// @route POST /api/instances/{id}/routing
/// @input Path(id): u16 - Instance ID
/// @input Json(routing): RoutingRequest - Routing configuration
/// @output Json<SuccessResponse<serde_json::Value>> - Creation result
/// @status 200 - Success with routing details
/// @status 400 - Validation error
/// @status 404 - Instance not found
/// @status 500 - Database error
/// @side-effects Inserts into point_routing table and Redis route:c2m
#[utoipa::path(
    post,
    path = "/api/instances/{id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = crate::dto::RoutingRequest,
    responses(
        (status = 200, description = "Routing created", body = serde_json::Value,
            example = json!({
                "routing": {
                    "instance_id": 1,
                    "channel": {
                        "id": 1,
                        "four_remote": "T",
                        "point_id": 101
                    },
                    "point_id": 101
                }
            })
        ),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Instance not found"),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn create_instance_routing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
    Json(routing): Json<RoutingRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Validate instance exists
    let instance_exists = match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_id = ?)",
    )
    .bind(id)
    .fetch_one(&state.instance_manager.pool)
    .await
    {
        Ok(exists) => exists,
        Err(e) => {
            return Err(ModSrvError::InternalError(format!("Database error: {}", e)));
        },
    };

    if !instance_exists {
        return Err(ModSrvError::InternalError(format!(
            "Not found: Instance {} does not exist",
            id
        )));
    }

    // Get instance name for validation
    let instance = state
        .instance_manager
        .get_instance(id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to get instance: {}", e)))?;
    let instance_name = &instance.core.instance_name;

    // Extract values before validation
    let channel_type = routing.four_remote;

    // Validate based on channel type
    let validation_result = if channel_type.is_input() {
        // Measurement routing (T/S → M)
        let routing_row = MeasurementRoutingRow {
            channel_id: routing.channel_id,
            channel_type: routing.four_remote,
            channel_point_id: routing.channel_point_id,
            measurement_id: routing.point_id,
        };
        state
            .instance_manager
            .validate_measurement_routing(&routing_row, instance_name)
            .await
    } else if channel_type.is_output() {
        // Action routing (A → C/A)
        let routing_row = ActionRoutingRow {
            action_id: routing.point_id,
            channel_id: routing.channel_id,
            channel_type: routing.four_remote,
            channel_point_id: routing.channel_point_id,
        };
        state
            .instance_manager
            .validate_action_routing(&routing_row, instance_name)
            .await
    } else {
        Err(anyhow::anyhow!("Invalid channel type: {}", channel_type))
    };

    match validation_result {
        Ok(validation) => {
            if !validation.is_valid {
                return Err(ModSrvError::InvalidData(format!(
                    "Invalid routing: {:?}",
                    validation.errors
                )));
            }
        },
        Err(e) => {
            return Err(ModSrvError::InvalidData(format!(
                "Validation failed: {}",
                e
            )));
        },
    }

    // Insert into database based on channel_type
    let insert_result = if channel_type.is_input() {
        // Insert into measurement_routing
        sqlx::query(
            r#"
            INSERT INTO measurement_routing
            (instance_id, instance_name, channel_id, channel_type, channel_point_id,
             measurement_id, enabled)
            VALUES (?, (SELECT instance_name FROM instances WHERE instance_id = ?), ?, ?, ?, ?, true)
            "#,
        )
        .bind(id)
        .bind(id)
        .bind(routing.channel_id)
        .bind(channel_type.as_str())
        .bind(routing.channel_point_id)
        .bind(routing.point_id)
        .execute(&state.instance_manager.pool)
        .await
    } else {
        // Insert into action_routing
        sqlx::query(
            r#"
            INSERT INTO action_routing
            (instance_id, instance_name, action_id, channel_id, channel_type,
             channel_point_id, enabled)
            VALUES (?, (SELECT instance_name FROM instances WHERE instance_id = ?), ?, ?, ?, ?, true)
            "#,
        )
        .bind(id)
        .bind(id)
        .bind(routing.point_id)
        .bind(routing.channel_id)
        .bind(channel_type.as_str())
        .bind(routing.channel_point_id)
        .execute(&state.instance_manager.pool)
        .await
    };

    // Check insert result
    if let Err(e) = insert_result {
        return Err(ModSrvError::InternalError(format!(
            "Failed to insert routing: {}",
            e
        )));
    }

    // Refresh routing cache after successful database update
    if let Err(e) = crate::bootstrap::refresh_routing_cache(
        &state.instance_manager.pool,
        state.instance_manager.routing_cache(),
    )
    .await
    {
        // Log error but don't fail the request - cache will be refreshed on next service restart
        tracing::warn!("Failed to refresh routing cache after create: {}", e);
    }

    Ok(Json(SuccessResponse::new(json!({
        "routing": {
            "instance_id": id,
            "channel": {
                "id": routing.channel_id,
                "four_remote": channel_type,
                "point_id": routing.channel_point_id
            },
            "point_id": routing.point_id
        }
    }))))
}

/// Update all routings for an instance (replace)
///
/// Replaces all existing routings with the provided new routings.
/// Uses a transaction to ensure atomic operation.
///
/// @route PUT /api/instances/{id}/routing
/// @input Path(id): u16 - Instance ID
/// @input Json(routings): Vec<RoutingRequest> - New routings to set
/// @output Json<SuccessResponse<serde_json::Value>> - Update result
/// @status 200 - Success with count
/// @status 400 - Validation errors
/// @status 500 - Transaction error
#[utoipa::path(
    put,
    path = "/api/instances/{id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = [crate::dto::RoutingRequest],
    responses(
        (status = 200, description = "Routings updated", body = serde_json::Value,
            example = json!({"message": "Updated 5 routings"})
        ),
        (status = 400, description = "Validation errors"),
        (status = 500, description = "Transaction error")
    ),
    tag = "modsrv"
)]
pub async fn update_instance_routing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
    Json(routings): Json<Vec<RoutingRequest>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Begin transaction
    let mut tx = match state.instance_manager.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            return Err(ModSrvError::InternalError(format!(
                "Failed to start transaction: {}",
                e
            )));
        },
    };

    // Delete existing routings from both tables
    if let Err(e) = sqlx::query("DELETE FROM measurement_routing WHERE instance_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return Err(ModSrvError::InternalError(format!(
            "Failed to delete measurement routings: {}",
            e
        )));
    }

    if let Err(e) = sqlx::query("DELETE FROM action_routing WHERE instance_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        return Err(ModSrvError::InternalError(format!(
            "Failed to delete existing routings: {}",
            e
        )));
    }

    // Get instance name for validation
    let instance = match state.instance_manager.get_instance(id).await {
        Ok(inst) => inst,
        Err(e) => {
            let _ = tx.rollback().await;
            return Err(ModSrvError::InternalError(format!(
                "Failed to get instance: {}",
                e
            )));
        },
    };
    let instance_name = &instance.core.instance_name;

    // Insert new routings
    let mut success_count = 0;
    let mut errors = Vec::new();

    for routing in routings {
        // Validate based on channel type
        let validation_result = if routing.four_remote.is_input() {
            // Measurement routing (T/S → M)
            let routing_row = MeasurementRoutingRow {
                channel_id: routing.channel_id,
                channel_type: routing.four_remote,
                channel_point_id: routing.channel_point_id,
                measurement_id: routing.point_id,
            };
            state
                .instance_manager
                .validate_measurement_routing(&routing_row, instance_name)
                .await
        } else if routing.four_remote.is_output() {
            // Action routing (A → C/A)
            let routing_row = ActionRoutingRow {
                action_id: routing.point_id,
                channel_id: routing.channel_id,
                channel_type: routing.four_remote,
                channel_point_id: routing.channel_point_id,
            };
            state
                .instance_manager
                .validate_action_routing(&routing_row, instance_name)
                .await
        } else {
            Err(anyhow::anyhow!(
                "Invalid channel type: {}",
                routing.four_remote
            ))
        };

        match validation_result {
            Ok(validation) => {
                if !validation.is_valid {
                    errors.extend(validation.errors);
                    continue;
                }
            },
            Err(e) => {
                errors.push(e.to_string());
                continue;
            },
        }

        // Insert into appropriate table based on channel type
        let result = if routing.four_remote.is_input() {
            // Insert into measurement_routing
            sqlx::query(
                r#"
                INSERT INTO measurement_routing
                (instance_id, instance_name, channel_id, channel_type, channel_point_id,
                 measurement_id, enabled)
                VALUES (?, (SELECT instance_name FROM instances WHERE instance_id = ?), ?, ?, ?, ?, true)
                "#,
            )
            .bind(id)
            .bind(id)
            .bind(routing.channel_id)
            .bind(routing.four_remote.as_str())
            .bind(routing.channel_point_id)
            .bind(routing.point_id)
            .execute(&mut *tx)
            .await
        } else {
            // Insert into action_routing
            sqlx::query(
                r#"
                INSERT INTO action_routing
                (instance_id, instance_name, action_id, channel_id, channel_type,
                 channel_point_id, enabled)
                VALUES (?, (SELECT instance_name FROM instances WHERE instance_id = ?), ?, ?, ?, ?, true)
                "#,
            )
            .bind(id)
            .bind(id)
            .bind(routing.point_id)
            .bind(routing.channel_id)
            .bind(routing.four_remote.as_str())
            .bind(routing.channel_point_id)
            .execute(&mut *tx)
            .await
        };

        if result.is_ok() {
            success_count += 1;
        } else if let Err(e) = result {
            errors.push(e.to_string());
        }
    }

    if errors.is_empty() {
        if let Err(e) = tx.commit().await {
            return Err(ModSrvError::InternalError(format!(
                "Failed to commit transaction: {}",
                e
            )));
        }

        // Refresh routing cache after successful database update
        if let Err(e) = crate::bootstrap::refresh_routing_cache(
            &state.instance_manager.pool,
            state.instance_manager.routing_cache(),
        )
        .await
        {
            tracing::warn!("Failed to refresh routing cache after update: {}", e);
        }

        Ok(Json(SuccessResponse::new(json!({
            "message": format!("Updated {} routings", success_count)
        }))))
    } else {
        let _ = tx.rollback().await;
        Err(ModSrvError::InvalidData(format!(
            "Update failed: {:?}",
            errors
        )))
    }
}

/// Delete all routings for an instance
///
/// @route DELETE /api/instances/{id}/routing
/// @input id: Path<u16> - Instance ID
/// @output Json<SuccessResponse<serde_json::Value>> - Success status with deleted count
/// @throws sqlx::Error - Database deletion error
/// @redis-delete route:c2m - Removes all routings for instance
/// @transaction Atomic deletion of all instance routings
/// @side-effects Removes all channel-to-instance routing
#[utoipa::path(
    delete,
    path = "/api/instances/{id}/routing",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    responses(
        (status = 200, description = "Routings deleted", body = serde_json::Value,
            example = json!({"message": "Deleted 5 routings (3 measurement, 2 action)"})
        ),
        (status = 500, description = "Database error")
    ),
    tag = "modsrv"
)]
pub async fn delete_instance_routing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.instance_manager.delete_all_routing(id).await {
        Ok((measurement_count, action_count)) => {
            let total_count = measurement_count + action_count;

            // Refresh routing cache after successful database update
            if let Err(e) = crate::bootstrap::refresh_routing_cache(
                &state.instance_manager.pool,
                state.instance_manager.routing_cache(),
            )
            .await
            {
                tracing::warn!("Failed to refresh routing cache after delete: {}", e);
            }

            Ok(Json(SuccessResponse::new(json!({
                "message": format!(
                    "Deleted {} routings ({} measurement, {} action)",
                    total_count, measurement_count, action_count
                )
            }))))
        },
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Failed to delete routings: {}",
            e
        ))),
    }
}

/// Validate routing for an instance
///
/// @route POST /api/instances/{id}/routing/validate
/// @input id: Path<u16> - Instance ID
/// @input routings: Json<Vec<RoutingRequest>> - Routings to validate
/// @output Json<SuccessResponse<serde_json::Value>> - Validation results for each routing
/// @throws None - Validation errors are returned in response
/// @redis-read Products and instance configurations
/// @side-effects None (validation only)
#[utoipa::path(
    post,
    path = "/api/instances/{id}/routing/validate",
    params(
        ("id" = u16, Path, description = "Instance ID")
    ),
    request_body = [crate::dto::RoutingRequest],
    responses(
        (status = 200, description = "Validation completed", body = serde_json::Value,
            example = json!({
                "instance_id": 1,
                "validations": [
                    {"channel": "1:T:101", "valid": true, "errors": []},
                    {"channel": "1:T:102", "valid": false, "errors": ["Point not found"]}
                ]
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn validate_instance_routing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u16>,
    Json(routings): Json<Vec<RoutingRequest>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Get instance name for validation
    let instance = state
        .instance_manager
        .get_instance(id)
        .await
        .map_err(|e| ModSrvError::InternalError(format!("Failed to get instance: {}", e)))?;
    let instance_name = &instance.core.instance_name;

    let mut results = Vec::new();

    for routing in routings {
        // Save channel info for response
        let channel_info = format!(
            "{}:{}:{}",
            routing.channel_id, routing.four_remote, routing.channel_point_id
        );

        // Validate based on channel type
        let validation_result = if routing.four_remote.is_input() {
            // Measurement routing (T/S → M)
            let routing_row = MeasurementRoutingRow {
                channel_id: routing.channel_id,
                channel_type: routing.four_remote,
                channel_point_id: routing.channel_point_id,
                measurement_id: routing.point_id,
            };
            state
                .instance_manager
                .validate_measurement_routing(&routing_row, instance_name)
                .await
        } else if routing.four_remote.is_output() {
            // Action routing (A → C/A)
            let routing_row = ActionRoutingRow {
                action_id: routing.point_id,
                channel_id: routing.channel_id,
                channel_type: routing.four_remote,
                channel_point_id: routing.channel_point_id,
            };
            state
                .instance_manager
                .validate_action_routing(&routing_row, instance_name)
                .await
        } else {
            Err(anyhow::anyhow!(
                "Invalid channel type: {}",
                routing.four_remote
            ))
        };

        match validation_result {
            Ok(validation) => {
                results.push(json!({
                    "channel": &channel_info,
                    "valid": validation.is_valid,
                    "errors": validation.errors
                }));
            },
            Err(e) => {
                results.push(json!({
                    "channel": &channel_info,
                    "valid": false,
                    "errors": vec![e.to_string()]
                }));
            },
        }
    }

    Ok(Json(SuccessResponse::new(json!({
        "instance_id": id,
        "validations": results
    }))))
}

// NOTE: The following handlers have been removed as they are now replaced by global routing APIs:
// - clear_routing_handler → use delete_all_routing_handler in global_routing_handlers.rs
// - clear_routing_for_instance_handler → use delete_instance_routing_handler in global_routing_handlers.rs
// - clear_routing_for_channel_handler → use delete_channel_routing_handler in global_routing_handlers.rs
