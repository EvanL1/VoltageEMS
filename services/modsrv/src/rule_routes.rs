//! Rule Engine API Routes
//!
//! Provides Vue Flow-based rule management and execution endpoints.
//! These routes are served on port 6003 (separate from modsrv's main port 6002).

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::error::ModSrvError;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utoipa::OpenApi;
use voltage_config::api::SuccessResponse;
use voltage_rtdb::traits::Rtdb;
use voltage_rules::{self as rule_repository, RuleScheduler};

/// Rule Engine state shared across handlers
pub struct RuleEngineState<R: Rtdb + ?Sized> {
    /// SQLite pool for rule persistence
    pub pool: SqlitePool,
    /// Rule scheduler (owns the executor)
    pub scheduler: Arc<RuleScheduler<R>>,
}

impl<R: Rtdb + ?Sized + 'static> RuleEngineState<R> {
    pub fn new(pool: SqlitePool, scheduler: Arc<RuleScheduler<R>>) -> Self {
        Self { pool, scheduler }
    }
}

/// Create rule engine API routes
pub fn create_rule_routes<R: Rtdb + ?Sized + Send + Sync + 'static>(
    state: Arc<RuleEngineState<R>>,
) -> Router {
    Router::new()
        .route("/health", get(health_check::<R>))
        // Rule management (Vue Flow-based)
        .route("/api/rules", get(list_rules::<R>).post(create_rule::<R>))
        .route(
            "/api/rules/{id}",
            get(get_rule::<R>)
                .put(update_rule::<R>)
                .delete(delete_rule::<R>),
        )
        .route("/api/rules/{id}/enable", post(enable_rule::<R>))
        .route("/api/rules/{id}/disable", post(disable_rule::<R>))
        .route("/api/rules/{id}/execute", post(execute_rule_now::<R>))
        // Scheduler control
        .route("/api/scheduler/status", get(scheduler_status::<R>))
        .route("/api/scheduler/reload", post(scheduler_reload::<R>))
        // Apply HTTP request logging middleware
        .layer(axum::middleware::from_fn(common::logging::http_request_logger))
        .with_state(state)
}

// ============================================================================
// OpenAPI Documentation
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    paths(list_rules, create_rule, get_rule, update_rule, delete_rule, enable_rule, disable_rule, execute_rule_now, scheduler_status, scheduler_reload),
    tags(
        (name = "rules", description = "Rule management and execution")
    )
)]
pub struct RuleApiDoc;

// ============================================================================
// Handlers
// ============================================================================

/// Health check endpoint for rule engine
async fn health_check<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let status = state.scheduler.status().await;

    Ok(Json(SuccessResponse::new(json!({
        "status": "healthy",
        "service": "modsrv-rules",
        "scheduler": {
            "running": status.running,
            "total_rules": status.total_rules,
            "enabled_rules": status.enabled_rules,
            "tick_interval_ms": status.tick_interval_ms
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))))
}

/// List all rules
#[utoipa::path(
    get,
    path = "/api/rules",
    responses(
        (status = 200, description = "List rules", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn list_rules<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match rule_repository::list_rules(&state.pool).await {
        Ok(rules) => Ok(Json(SuccessResponse::new(json!(rules)))),
        Err(e) => {
            error!("Failed to list rules: {}", e);
            Err(ModSrvError::InternalError(
                "Failed to list rules".to_string(),
            ))
        },
    }
}

/// Create a new rule
#[utoipa::path(
    post,
    path = "/api/rules",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule created", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn create_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Json(rule): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let rule_id = rule["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("rule_{}", uuid::Uuid::new_v4()));

    if let Err(e) = rule_repository::upsert_rule(&state.pool, &rule_id, &rule).await {
        error!("Failed to create rule {}: {}", rule_id, e);
        return Err(ModSrvError::InternalError(
            "Failed to create rule".to_string(),
        ));
    }

    // Reload scheduler to pick up new rule
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule create: {}", e);
    }

    debug!("Created rule: {}", rule_id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": rule_id, "status": "OK" }),
    )))
}

/// Get rule by ID
#[utoipa::path(
    get,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule details", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn get_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match rule_repository::get_rule(&state.pool, &id).await {
        Ok(rule) => Ok(Json(SuccessResponse::new(rule))),
        Err(e) => {
            error!("Failed to get rule {}: {}", id, e);
            Err(ModSrvError::RuleNotFound(id))
        },
    }
}

/// Update rule
#[utoipa::path(
    put,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule updated", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn update_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
    Json(mut rule): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Ensure the rule ID in JSON matches the path parameter
    rule["id"] = serde_json::json!(id.clone());

    if let Err(e) = rule_repository::upsert_rule(&state.pool, &id, &rule).await {
        error!("Failed to update rule {} in SQLite: {}", id, e);
        return Err(ModSrvError::InternalError(format!(
            "Failed to update rule in database: {}",
            e
        )));
    }

    // Reload scheduler to pick up changes
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule update: {}", e);
    }

    info!("Rule {} updated successfully", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Delete rule
#[utoipa::path(
    delete,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule deleted", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn delete_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    if let Err(e) = rule_repository::delete_rule(&state.pool, &id).await {
        error!("Failed to delete rule {}: {}", id, e);
        return Err(ModSrvError::InternalError(
            "Failed to delete rule".to_string(),
        ));
    }

    // Reload scheduler to remove the rule
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule delete: {}", e);
    }

    info!("Deleted rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Enable rule
#[utoipa::path(
    post,
    path = "/api/rules/{id}/enable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule enabled", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn enable_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    if let Err(e) = rule_repository::set_rule_enabled(&state.pool, &id, true).await {
        error!("Failed to enable rule {}: {}", id, e);
        return Err(ModSrvError::InternalError(
            "Failed to enable rule".to_string(),
        ));
    }

    // Reload scheduler
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule enable: {}", e);
    }

    info!("Enabled rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Disable rule
#[utoipa::path(
    post,
    path = "/api/rules/{id}/disable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule disabled", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn disable_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    if let Err(e) = rule_repository::set_rule_enabled(&state.pool, &id, false).await {
        error!("Failed to disable rule {}: {}", id, e);
        return Err(ModSrvError::InternalError(
            "Failed to disable rule".to_string(),
        ));
    }

    // Reload scheduler
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule disable: {}", e);
    }

    info!("Disabled rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Execute rule immediately (manual trigger)
#[utoipa::path(
    post,
    path = "/api/rules/{id}/execute",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule execution result", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn execute_rule_now<R: Rtdb + ?Sized + Send + Sync + 'static>(
    Path(id): Path<String>,
    State(state): State<Arc<RuleEngineState<R>>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let execution_id = format!("manual-{}", uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now();

    // Execute through scheduler (which handles rule loading)
    let result = state.scheduler.execute_rule(&id).await?;

    // Format action results for response
    let action_results: Vec<serde_json::Value> = result
        .actions_executed
        .iter()
        .map(|a| {
            json!({
                "target_type": a.target_type,
                "target_id": a.target_id,
                "point_type": a.point_type,
                "point_id": a.point_id,
                "value": a.value,
                "success": a.success
            })
        })
        .collect();

    // Build response based on execution result
    if result.success {
        Ok(Json(SuccessResponse::new(json!({
            "result": "executed",
            "rule_id": result.rule_id,
            "execution_id": execution_id,
            "success": true,
            "actions_executed": action_results,
            "execution_path": result.execution_path,
            "timestamp": timestamp
        }))))
    } else {
        Ok(Json(SuccessResponse::new(json!({
            "result": "failed",
            "rule_id": result.rule_id,
            "execution_id": execution_id,
            "success": false,
            "error": result.error,
            "actions_executed": action_results,
            "execution_path": result.execution_path,
            "timestamp": timestamp
        }))))
    }
}

/// Get scheduler status
#[utoipa::path(
    get,
    path = "/api/scheduler/status",
    responses(
        (status = 200, description = "Scheduler status", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn scheduler_status<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let status = state.scheduler.status().await;

    Ok(Json(SuccessResponse::new(json!({
        "running": status.running,
        "total_rules": status.total_rules,
        "enabled_rules": status.enabled_rules,
        "tick_interval_ms": status.tick_interval_ms
    }))))
}

/// Reload scheduler rules from database
#[utoipa::path(
    post,
    path = "/api/scheduler/reload",
    responses(
        (status = 200, description = "Rules reloaded", body = serde_json::Value)
    ),
    tag = "rules"
)]
pub async fn scheduler_reload<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state.scheduler.reload_rules().await {
        Ok(count) => {
            info!("Scheduler reloaded {} rules", count);
            Ok(Json(SuccessResponse::new(json!({
                "status": "OK",
                "rules_loaded": count
            }))))
        },
        Err(e) => {
            error!("Failed to reload scheduler: {}", e);
            Err(ModSrvError::SchedulerError(format!(
                "Failed to reload rules: {}",
                e
            )))
        },
    }
}
