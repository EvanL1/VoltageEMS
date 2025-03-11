//! Calculation Management API Handlers
//!
//! This module provides CRUD operations for calculation definitions and execution endpoints.
//! Handles registration, retrieval, updating, deletion, and execution of calculations.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use voltage_config::api::SuccessResponse;

use crate::app_state::AppState;
use crate::calculation_engine::calculation_from_json;
use crate::dto::BatchExecuteRequest;
use crate::error::ModSrvError;

/// List all registered calculations
///
/// @route GET /api/calculations
/// @output Json<SuccessResponse<serde_json::Value>> - List of calculation definitions
/// @redis-read modsrv:calculations:* - All calculation configurations
/// @side-effects None (read-only operation)
#[utoipa::path(
    get,
    path = "/api/calculations",
    responses(
        (status = 200, description = "List calculations", body = serde_json::Value,
            example = json!({"calculations": [{"id": "power_calc", "formula": "P1 * P2"}], "count": 1})
        )
    ),
    tag = "modsrv"
)]
pub async fn list_calculations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let calculations = state.calculation_engine.list_calculations().await;
    Ok(Json(SuccessResponse::new(json!({
        "calculations": calculations,
        "count": calculations.len()
    }))))
}

/// Register a new calculation definition
///
/// @route POST /api/calculations
/// @input calc_json: Json - Calculation definition with formula and variables
/// @output Json - Success with calculation_id or error message
/// @throws anyhow::Error - Invalid calculation definition
/// @redis-write modsrv:calculations:{id} - Stores calculation definition
/// @side-effects Registers calculation in engine for execution
#[utoipa::path(
    post,
    path = "/api/calculations",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Calculation created", body = serde_json::Value,
            example = json!({"calculation_id": "power_calc"})
        )
    ),
    tag = "modsrv"
)]
pub async fn create_calculation(
    State(state): State<Arc<AppState>>,
    Json(calc_json): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match calculation_from_json(calc_json) {
        Ok(calc) => {
            let calc_id = calc.id.clone();
            match state.calculation_engine.register_calculation(calc).await {
                Ok(_) => {
                    info!("Registered calculation: {}", calc_id);
                    Ok(Json(SuccessResponse::new(json!({
                        "calculation_id": calc_id
                    }))))
                },
                Err(e) => Err(ModSrvError::InternalError(format!(
                    "Failed to register calculation: {}",
                    e
                ))),
            }
        },
        Err(e) => Err(ModSrvError::InvalidData(format!(
            "Invalid calculation definition: {}",
            e
        ))),
    }
}

/// Get calculation definition by ID
///
/// @route GET /api/calculations/{id}
/// @input id: Path<String> - Calculation identifier
/// @output Json<Calculation> - Calculation definition
/// @redis-read modsrv:calculations:{id} - Calculation configuration
/// @side-effects None (read-only operation)
#[utoipa::path(
    get,
    path = "/api/calculations/{id}",
    params(
        ("id" = String, Path, description = "Calculation identifier")
    ),
    responses(
        (status = 200, description = "Calculation details", body = serde_json::Value,
            example = json!({"id": "power_calc", "formula": "P1 * P2", "variables": ["P1", "P2"]})
        )
    ),
    tag = "modsrv"
)]
pub async fn get_calculation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.calculation_engine.get_calculation(&id).await {
        Some(calc) => Ok(Json(SuccessResponse::new(
            serde_json::to_value(calc).unwrap_or(json!({})),
        ))),
        None => Err(ModSrvError::InternalError(format!(
            "Not found: Calculation '{}' not found",
            id
        ))),
    }
}

/// Update existing calculation definition
///
/// @route PUT /api/calculations/{id}
/// @input id: Path<String> - Calculation identifier
/// @input calc_json: Json - Updated calculation definition
/// @output Json - Success or error status
/// @redis-write modsrv:calculations:{id} - Updates calculation
/// @side-effects Re-registers calculation in engine
#[utoipa::path(
    put,
    path = "/api/calculations/{id}",
    params(
        ("id" = String, Path, description = "Calculation identifier")
    ),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Calculation updated", body = serde_json::Value,
            example = json!({"calculation_id": "power_calc"})
        )
    ),
    tag = "modsrv"
)]
pub async fn update_calculation(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(calc_json): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Reuse create_calculation logic
    create_calculation(State(state), Json(calc_json)).await
}

/// Delete a calculation definition
///
/// @route DELETE /api/calculations/{id}
/// @input id: Path<String> - Calculation to delete
/// @output Json - Success status
/// @redis-delete modsrv:calculations:{id} - Removes calculation
/// @side-effects Unregisters from calculation engine
#[utoipa::path(
    delete,
    path = "/api/calculations/{id}",
    params(
        ("id" = String, Path, description = "Calculation identifier")
    ),
    responses(
        (status = 200, description = "Calculation deleted", body = serde_json::Value,
            example = json!({"message": "Calculation power_calc deleted"})
        )
    ),
    tag = "modsrv"
)]
pub async fn delete_calculation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.calculation_engine.delete_calculation(&id).await {
        Ok(_) => Ok(Json(SuccessResponse::new(json!({
            "message": format!("Calculation {} deleted", id)
        })))),
        Err(e) => Err(ModSrvError::InternalError(e.to_string())),
    }
}

/// Execute a single calculation
///
/// @route POST /api/calculations/{id}/execute
/// @input id: Path<String> - Calculation to execute
/// @output Json<CalculationResult> - Execution result with values
/// @throws anyhow::Error - Calculation not found or execution error
/// @redis-read modsrv instance data for variable values
/// @side-effects May trigger virtual point updates
#[utoipa::path(
    post,
    path = "/api/calculations/{id}/execute",
    params(
        ("id" = String, Path, description = "Calculation identifier")
    ),
    responses(
        (status = 200, description = "Execution result", body = serde_json::Value,
            example = json!({
                "calculation_id": "power_calc",
                "result": 8000.0,
                "variables": {"P1": 400.0, "P2": 20.0},
                "formula": "P1 * P2",
                "timestamp": "2025-10-15T14:30:00Z"
            })
        )
    ),
    tag = "modsrv"
)]
pub async fn execute_calculation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.calculation_engine.execute_calculation(&id).await {
        Ok(result) => match serde_json::to_value(result) {
            Ok(value) => Ok(Json(SuccessResponse::new(value))),
            Err(_) => Err(ModSrvError::InternalError(
                "Failed to serialize result".to_string(),
            )),
        },
        Err(e) => Err(ModSrvError::InternalError(format!(
            "Calculation execution failed: {}",
            e
        ))),
    }
}

/// Execute multiple calculations in batch
///
/// @route POST /api/calculations/batch/execute
/// @input request: Json<BatchExecuteRequest> - List of calculation IDs
/// @output Json<Vec<CalculationResult>> - Results for each calculation
/// @redis-read Multiple instance data points
/// @side-effects May trigger multiple virtual point updates
pub async fn execute_batch_calculations(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BatchExecuteRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let mut results = Vec::new();

    for calc_id in request.calculation_ids {
        let result = state.calculation_engine.execute_calculation(&calc_id).await;
        results.push(match result {
            Ok(res) => serde_json::to_value(res).unwrap_or(json!({
                "calculation_id": calc_id,
                "error": "Serialization failed"
            })),
            Err(e) => json!({
                "calculation_id": calc_id,
                "error": e.to_string()
            }),
        });
    }

    Ok(Json(SuccessResponse::new(json!({
        "results": results,
        "executed": results.len()
    }))))
}
