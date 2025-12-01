//! Rule Engine API Routes
//!
//! Provides Vue Flow-based rule management and execution endpoints.
//! These routes are integrated into modsrv and served on port 6002.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::error::ModSrvError;
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{error, info, warn};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
use voltage_config::api::{PaginatedResponse, SuccessResponse};
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

#[cfg(feature = "swagger-ui")]
#[derive(OpenApi)]
#[openapi(
    paths(list_rules, create_rule, get_rule, update_rule, delete_rule, enable_rule, disable_rule, execute_rule_now, scheduler_status, scheduler_reload),
    components(
        schemas(
            CreateRuleRequest,
            UpdateRuleRequest,
            RuleListQuery
        )
    ),
    tags(
        (name = "rules", description = "Rule management and execution")
    )
)]
pub struct RuleApiDoc;

// ============================================================================
// Handlers
// ============================================================================

/// Rule list query parameters (pagination)
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(utoipa::ToSchema))]
pub struct RuleListQuery {
    /// Page number (starting from 1)
    #[serde(default = "default_page")]
    pub page: usize,
    /// Items per page
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}

/// Request DTO for creating a new rule (empty shell, ID auto-generated)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[cfg_attr(feature = "swagger-ui", derive(utoipa::ToSchema))]
pub struct CreateRuleRequest {
    /// Rule name (required)
    #[cfg_attr(feature = "swagger-ui", schema(example = "Battery SOC Protection"))]
    pub name: String,

    /// Rule description (optional)
    #[cfg_attr(
        feature = "swagger-ui",
        schema(example = "Protect battery when SOC is too low")
    )]
    pub description: Option<String>,
}

/// Request DTO for updating an existing rule (all fields optional, partial update)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[cfg_attr(feature = "swagger-ui", derive(utoipa::ToSchema))]
pub struct UpdateRuleRequest {
    /// Rule name (optional)
    #[cfg_attr(feature = "swagger-ui", schema(example = "Battery SOC Protection v2"))]
    pub name: Option<String>,

    /// Rule description (optional)
    #[cfg_attr(feature = "swagger-ui", schema(example = "Updated protection logic"))]
    pub description: Option<String>,

    /// Whether the rule is enabled (optional)
    #[cfg_attr(feature = "swagger-ui", schema(example = true))]
    pub enabled: Option<bool>,

    /// Execution priority (optional)
    #[cfg_attr(feature = "swagger-ui", schema(example = 20))]
    pub priority: Option<u32>,

    /// Cooldown period in milliseconds (optional)
    #[cfg_attr(feature = "swagger-ui", schema(example = 10000))]
    pub cooldown_ms: Option<u64>,

    /// Vue Flow complete data (nodes, edges, viewport)
    #[cfg_attr(feature = "swagger-ui", schema(value_type = Option<Object>))]
    pub flow_json: Option<serde_json::Value>,
}

/// List all rules
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/rules",
    params(
        ("page" = Option<usize>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<usize>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "List rules (paginated)", body = voltage_config::api::PaginatedResponse<serde_json::Value>,
            example = json!({
                "success": true,
                "data": {
                    "list": [
                        { "id": "rule-001", "name": "Test Rule", "enabled": true, "description": "demo rule" }
                    ],
                    "total": 1,
                    "page": 1,
                    "page_size": 20,
                    "total_pages": 1,
                    "has_next": false,
                    "has_previous": false
                }
            })
        )
    ),
    tag = "rules"
))]
pub async fn list_rules<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<SuccessResponse<PaginatedResponse<serde_json::Value>>>, ModSrvError> {
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);

    match rule_repository::list_rules_paginated(&state.pool, page, page_size).await {
        Ok((rules, total)) => {
            // Only expose summary fields for list view
            let summaries: Vec<serde_json::Value> = rules
                .into_iter()
                .map(|rule| {
                    json!({
                        "id": rule.get("id").cloned().unwrap_or(serde_json::Value::Null),
                        "name": rule.get("name").cloned().unwrap_or(serde_json::Value::Null),
                        "enabled": rule.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                        "description": rule.get("description").cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect();

            let paginated = PaginatedResponse::new(summaries, total, page, page_size);
            Ok(Json(SuccessResponse::new(paginated)))
        },
        Err(e) => {
            error!("Failed to list rules: {}", e);
            Err(ModSrvError::InternalError(
                "Failed to list rules".to_string(),
            ))
        },
    }
}

/// Create a new rule (metadata only)
///
/// 创建规则元数据。ID 由后端自动生成（顺序递增：1, 2, 3...）。
/// 规则的执行拓扑（flow_json）后续通过 PUT 接口更新。
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/rules",
    request_body(
        content = CreateRuleRequest,
        description = "规则元数据（ID 自动生成）"
    ),
    responses(
        (status = 200, description = "规则创建成功", body = serde_json::Value,
         example = json!({ "success": true, "data": { "id": "1", "name": "Battery Protection", "status": "created" } }))
    ),
    tag = "rules"
))]
pub async fn create_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Get next sequential ID
    let next_id: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(CAST(id AS INTEGER)), 0) + 1 FROM rules")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(1);
    let rule_id = next_id.to_string();

    // Insert empty rule record (metadata only, no flow)
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO rules (id, name, description, nodes_json, flow_json, format, enabled, priority, cooldown_ms)
        VALUES (?, ?, ?, '{}', NULL, 'vue-flow', FALSE, 0, 0)
        "#,
    )
    .bind(&rule_id)
    .bind(&req.name)
    .bind(&req.description)
    .execute(&state.pool)
    .await
    {
        error!("Failed to create rule {}: {}", rule_id, e);
        return Err(ModSrvError::InternalError(
            "Failed to create rule".to_string(),
        ));
    }

    // Reload scheduler to pick up new rule
    if let Err(e) = state.scheduler.reload_rules().await {
        warn!("Failed to reload scheduler after rule create: {}", e);
    }

    info!("Created rule: {} ({})", req.name, rule_id);
    Ok(Json(SuccessResponse::new(json!({
        "id": rule_id,
        "name": req.name,
        "status": "created"
    }))))
}

/// Get rule by ID
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule details", body = serde_json::Value)
    ),
    tag = "rules"
))]
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

/// Update rule metadata
///
/// 更新规则元数据。只有提供的字段会被更新（部分更新）。
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    put,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "规则 ID")),
    request_body(
        content = UpdateRuleRequest,
        description = "要更新的字段（只有提供的字段会被更新）"
    ),
    responses(
        (status = 200, description = "规则更新成功", body = serde_json::Value,
         example = json!({ "success": true, "data": { "id": "1", "status": "updated" } })),
        (status = 404, description = "规则不存在")
    ),
    tag = "rules"
))]
pub async fn update_rule<R: Rtdb + ?Sized + Send + Sync + 'static>(
    State(state): State<Arc<RuleEngineState<R>>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRuleRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    // Check rule exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rules WHERE id = ?)")
        .bind(&id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);

    if !exists {
        return Err(ModSrvError::RuleNotFound(id.clone()));
    }

    // Build dynamic UPDATE query for provided fields only (partial update)
    let mut updates = Vec::new();

    if req.name.is_some() {
        updates.push("name = ?");
    }
    if req.description.is_some() {
        updates.push("description = ?");
    }
    if req.enabled.is_some() {
        updates.push("enabled = ?");
    }
    if req.priority.is_some() {
        updates.push("priority = ?");
    }
    if req.cooldown_ms.is_some() {
        updates.push("cooldown_ms = ?");
    }
    if req.flow_json.is_some() {
        updates.push("flow_json = ?");
        updates.push("nodes_json = ?"); // Also update compact format for execution
    }

    if updates.is_empty() {
        return Err(ModSrvError::InvalidRule("No fields to update".to_string()));
    }

    let sql = format!("UPDATE rules SET {} WHERE id = ?", updates.join(", "));
    let mut query = sqlx::query(&sql);

    // Bind values in order
    if let Some(name) = &req.name {
        query = query.bind(name);
    }
    if let Some(desc) = &req.description {
        query = query.bind(desc);
    }
    if let Some(enabled) = req.enabled {
        query = query.bind(enabled);
    }
    if let Some(priority) = req.priority {
        query = query.bind(priority as i64);
    }
    if let Some(cooldown) = req.cooldown_ms {
        query = query.bind(cooldown as i64);
    }
    if let Some(flow) = &req.flow_json {
        // Bind flow_json (original Vue Flow data for editor)
        let flow_str = serde_json::to_string(flow)
            .map_err(|e| ModSrvError::SerializationError(e.to_string()))?;
        query = query.bind(flow_str);

        // Extract and bind nodes_json (compact format for execution)
        let compact_flow = voltage_rules::extract_rule_flow(flow)
            .map_err(|e| ModSrvError::ParseError(e.to_string()))?;
        let nodes_str = serde_json::to_string(&compact_flow)
            .map_err(|e| ModSrvError::SerializationError(e.to_string()))?;
        query = query.bind(nodes_str);
    }
    query = query.bind(&id);

    if let Err(e) = query.execute(&state.pool).await {
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
    Ok(Json(SuccessResponse::new(json!({
        "id": id,
        "status": "updated"
    }))))
}

/// Delete rule
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    delete,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule deleted", body = serde_json::Value)
    ),
    tag = "rules"
))]
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
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/rules/{id}/enable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule enabled", body = serde_json::Value)
    ),
    tag = "rules"
))]
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
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/rules/{id}/disable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule disabled", body = serde_json::Value)
    ),
    tag = "rules"
))]
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
///
/// 手动触发规则执行，返回执行结果和已执行的动作列表。
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/rules/{id}/execute",
    params(("id" = String, Path, description = "规则标识符")),
    responses(
        (status = 200, description = "规则执行结果", body = serde_json::Value,
         example = json!({
             "success": true,
             "data": {
                 "result": "executed",
                 "rule_id": "soc-strategy-001",
                 "execution_id": "manual-a1b2c3d4",
                 "success": true,
                 "actions_executed": [
                     { "target_type": "instance", "target_id": "pv_01", "point_type": "action", "point_id": 5, "value": 78.0, "success": true }
                 ],
                 "execution_path": ["start", "switch-soc", "action-high", "end"],
                 "timestamp": "2024-01-01T12:00:00Z"
             }
         }))
    ),
    tag = "rules"
))]
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
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/api/scheduler/status",
    responses(
        (status = 200, description = "Scheduler status", body = serde_json::Value)
    ),
    tag = "rules"
))]
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
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/api/scheduler/reload",
    responses(
        (status = 200, description = "Rules reloaded", body = serde_json::Value)
    ),
    tag = "rules"
))]
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
