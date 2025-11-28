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
use tracing::{debug, error, info, warn};
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

/// Rule list query parameters (pagination)
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
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

/// List all rules
#[utoipa::path(
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
)]
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
                        "id": rule.get("id").cloned().unwrap_or_else(|| json!(null)),
                        "name": rule.get("name").cloned().unwrap_or_else(|| json!(null)),
                        "enabled": rule.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                        "description": rule.get("description").cloned().unwrap_or_else(|| json!(null)),
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

/// Create a new rule
///
/// 创建或更新规则。规则使用 Vue Flow 格式描述执行拓扑。
///
/// ## 字段说明
///
/// - `format`: 格式标注，目前支持 "vue-flow"（默认）
/// - `flow_json`: 完整的 Vue Flow JSON（包含 nodes、edges、positions 等 UI 信息），供前端回显编辑
///
/// 后端会自动从 `flow_json` 提取执行拓扑并存储。
///
/// 参见 `docs/examples/soc-strategy-rule.json` 获取完整示例。
#[utoipa::path(
    post,
    path = "/api/rules",
    request_body(
        content = serde_json::Value,
        description = "Vue Flow 规则 JSON。format 字段标注格式类型，flow_json 包含完整 Vue Flow 结构。",
        example = json!({
            "id": "soc-strategy-001",
            "name": "SOC 电池管理策略",
            "description": "根据电池 SOC 值自动调节光伏和柴油发电机功率",
            "format": "vue-flow",
            "enabled": true,
            "priority": 100,
            "cooldown_ms": 5000,
            "flow_json": {
                "nodes": [
                    { "id": "start", "type": "start", "position": { "x": 100, "y": 100 }, "data": { "config": { "wires": { "default": ["switch-soc"] } } } },
                    { "id": "switch-soc", "type": "custom", "position": { "x": 300, "y": 100 }, "data": {
                        "type": "function-switch",
                        "label": "SOC 判断",
                        "config": {
                            "variables": [{ "name": "X1", "type": "single", "instance": "battery_01", "pointType": "measurement", "point": 3 }],
                            "rule": [
                                { "name": "out001", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": ">=", "value": 99 }] },
                                { "name": "out002", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": ">=", "value": 49 }] },
                                { "name": "out003", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": "<=", "value": 5 }] }
                            ],
                            "wires": { "out001": ["action-high"], "out002": ["action-mid"], "out003": ["action-low"] }
                        }
                    }},
                    { "id": "action-high", "type": "custom", "position": { "x": 600, "y": 50 }, "data": {
                        "type": "action-changeValue",
                        "label": "高电量动作",
                        "config": {
                            "variables": [{ "name": "Y1", "type": "single", "instance": "pv_01", "pointType": "action", "point": 5 }],
                            "rule": [{ "Variables": "Y1", "value": 78 }],
                            "wires": { "default": ["end"] }
                        }
                    }},
                    { "id": "end", "type": "end", "position": { "x": 900, "y": 100 } }
                ],
                "edges": [
                    { "id": "e1", "source": "start", "target": "switch-soc" },
                    { "id": "e2", "source": "switch-soc", "sourceHandle": "out001", "target": "action-high" },
                    { "id": "e3", "source": "action-high", "target": "end" }
                ]
            }
        })
    ),
    responses(
        (status = 200, description = "规则创建成功", body = serde_json::Value,
         example = json!({ "success": true, "data": { "id": "soc-strategy-001", "status": "OK" } }))
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
///
/// 更新已存在的规则。请求体格式与创建规则相同。
#[utoipa::path(
    put,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "规则标识符")),
    request_body(
        content = serde_json::Value,
        description = "Vue Flow 规则 JSON，格式同创建规则"
    ),
    responses(
        (status = 200, description = "规则更新成功", body = serde_json::Value,
         example = json!({ "success": true, "data": { "id": "soc-strategy-001", "status": "OK" } }))
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
///
/// 手动触发规则执行，返回执行结果和已执行的动作列表。
#[utoipa::path(
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
