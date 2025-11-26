//! API routes and handlers for Rule Service
//!
//! Provides Vue Flow-based rule chain management and execution endpoints.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::app::AppState;
use crate::chain_executor::ChainExecutor;
use crate::error::RuleSrvError;
use crate::rules_repository;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use utoipa::OpenApi;
use voltage_config::api::SuccessResponse;

/// Create all API routes with state
pub fn create_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        // Rule chain management (Vue Flow-based)
        .route("/api/rules", get(list_rules).post(create_rule))
        .route(
            "/api/rules/{id}",
            get(get_rule).put(update_rule).delete(delete_rule),
        )
        .route("/api/rules/{id}/enable", post(enable_rule))
        .route("/api/rules/{id}/disable", post(disable_rule))
        .route("/api/rules/{id}/execute", post(execute_rule_now))
        // Apply HTTP request logging middleware
        .layer(axum::middleware::from_fn(common::logging::http_request_logger))
        .with_state(state)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn build_test_state() -> Arc<AppState> {
        // Use MemoryRtdb for testing (no actual Redis required)
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());

        // In-memory SQLite with rules table
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Use standard rulesrv schema from common test utils
        common::test_utils::schema::init_rulesrv_schema(&pool)
            .await
            .unwrap();

        let sqlite_client = Some(Arc::new(common::sqlite::SqliteClient::from_pool(pool)));
        let config = Arc::new(crate::app::Config::default());
        let chains_cache = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());

        Arc::new(crate::app::AppState {
            rtdb,
            routing_cache,
            config,
            sqlite_client,
            chains_cache,
        })
    }

    /// Generate a minimal valid Vue Flow JSON for testing
    fn make_test_flow_json() -> serde_json::Value {
        serde_json::json!({
            "id": "test-chain",
            "name": "Test Chain",
            "nodes": [
                {
                    "id": "start-1",
                    "type": "start",
                    "data": {
                        "config": {
                            "wires": {
                                "default": ["end-1"]
                            }
                        }
                    }
                },
                {
                    "id": "end-1",
                    "type": "end",
                    "data": { "config": {} }
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_health_ok() {
        let state = build_test_state().await;
        let app = create_routes(state);
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_rules_ok() {
        let state = build_test_state().await;
        let app = create_routes(state);
        let req = Request::builder()
            .uri("/api/rules")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rules_crud_basic() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // Create rule using valid Vue Flow JSON
        let body = serde_json::json!({
            "id": "r1",
            "name": "rule1",
            "enabled": true,
            "priority": 10,
            "flow_json": make_test_flow_json()
        });
        let req = Request::builder()
            .uri("/api/rules")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Get rule
        let req_get = Request::builder()
            .uri("/api/rules/r1")
            .body(Body::empty())
            .unwrap();
        let resp_get = app.clone().oneshot(req_get).await.unwrap();
        assert_eq!(resp_get.status(), axum::http::StatusCode::OK);

        // Enable/Disable
        let req_en = Request::builder()
            .uri("/api/rules/r1/enable")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let resp_en = app.clone().oneshot(req_en).await.unwrap();
        assert_eq!(resp_en.status(), axum::http::StatusCode::OK);

        let req_dis = Request::builder()
            .uri("/api/rules/r1/disable")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let resp_dis = app.oneshot(req_dis).await.unwrap();
        assert_eq!(resp_dis.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_execute_rule_now_with_chain_executor() {
        let state = build_test_state().await;

        // First, create a rule chain in SQLite
        let flow_json = make_test_flow_json();
        let parsed = crate::parse_flow_json(&flow_json).unwrap();

        sqlx::query(
            "INSERT INTO rules (id, name, description, flow_json, enabled, priority, cooldown_ms, variables_json, nodes_json, start_node_id)
             VALUES ('r_exec', 'Test Exec', 'Test rule for execution', ?, 1, 0, 0, ?, ?, ?)"
        )
        .bind(serde_json::to_string(&flow_json).unwrap())
        .bind(serde_json::to_string(&parsed.variables).unwrap())
        .bind(serde_json::to_string(&parsed.nodes).unwrap())
        .bind(&parsed.start_node_id)
        .execute(state.sqlite_client.as_ref().unwrap().pool())
        .await
        .unwrap();

        let app = create_routes(state);
        let req = Request::builder()
            .uri("/api/rules/r_exec/execute")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Parse response body
        let body = axum::body::to_bytes(resp.into_body(), 10000).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Check that execution was successful (start -> end path)
        assert!(result["data"]["success"].as_bool().unwrap_or(false));
    }
}

// ============================================================================
// OpenAPI Documentation
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    paths(list_rules, create_rule, get_rule, update_rule, delete_rule, enable_rule, disable_rule, execute_rule_now),
    tags(
        (name = "rulesrv", description = "Rule chain management and execution")
    )
)]
pub struct ApiDoc;

// ============================================================================
// Handlers
// ============================================================================

/// Health check endpoint
async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let sqlite_status = if state.sqlite_client.is_some() {
        "connected"
    } else {
        "not configured"
    };

    let chains_count = state.chains_cache.read().await.len();

    Ok(Json(SuccessResponse::new(json!({
        "status": "healthy",
        "service": "rulesrv",
        "sqlite": sqlite_status,
        "chains_cached": chains_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))))
}

/// List all rule chains
#[utoipa::path(
    get,
    path = "/api/rules",
    responses(
        (status = 200, description = "List rule chains", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn list_rules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    match rules_repository::list_rules(&state).await {
        Ok(rules) => Ok(Json(SuccessResponse::new(json!(rules)))),
        Err(e) => {
            error!("Failed to list rules: {}", e);
            Err(RuleSrvError::InternalError(
                "Failed to list rules".to_string(),
            ))
        },
    }
}

/// Create a new rule chain
#[utoipa::path(
    post,
    path = "/api/rules",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule created", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let rule_id = rule["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("rule_{}", uuid::Uuid::new_v4()));

    if let Err(e) = rules_repository::upsert_rule(&state, &rule_id, &rule).await {
        error!("Failed to create rule {}: {}", rule_id, e);
        return Err(RuleSrvError::InternalError(
            "Failed to create rule".to_string(),
        ));
    }

    debug!("Created rule: {}", rule_id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": rule_id, "status": "OK" }),
    )))
}

/// Get rule chain by ID
#[utoipa::path(
    get,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule details", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    match rules_repository::get_rule(&state, &id).await {
        Ok(rule) => Ok(Json(SuccessResponse::new(rule))),
        Err(e) => {
            error!("Failed to get rule {}: {}", id, e);
            Err(RuleSrvError::RuleNotFound(id))
        },
    }
}

/// Update rule chain
#[utoipa::path(
    put,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule updated", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut rule): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    // Ensure the rule ID in JSON matches the path parameter
    rule["id"] = serde_json::json!(id.clone());

    if let Err(e) = rules_repository::upsert_rule(&state, &id, &rule).await {
        error!("Failed to update rule {} in SQLite: {}", id, e);
        return Err(RuleSrvError::InternalError(format!(
            "Failed to update rule in database: {}",
            e
        )));
    }

    info!("Rule {} updated successfully", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Delete rule chain
#[utoipa::path(
    delete,
    path = "/api/rules/{id}",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule deleted", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    if let Err(e) = rules_repository::delete_rule(&state, &id).await {
        error!("Failed to delete rule {}: {}", id, e);
        return Err(RuleSrvError::InternalError(
            "Failed to delete rule".to_string(),
        ));
    }

    info!("Deleted rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Enable rule chain
#[utoipa::path(
    post,
    path = "/api/rules/{id}/enable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule enabled", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn enable_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    if let Err(e) = rules_repository::set_rule_enabled(&state, &id, true).await {
        error!("Failed to enable rule {}: {}", id, e);
        return Err(RuleSrvError::InternalError(
            "Failed to enable rule".to_string(),
        ));
    }

    info!("Enabled rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Disable rule chain
#[utoipa::path(
    post,
    path = "/api/rules/{id}/disable",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule disabled", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn disable_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    if let Err(e) = rules_repository::set_rule_enabled(&state, &id, false).await {
        error!("Failed to disable rule {}: {}", id, e);
        return Err(RuleSrvError::InternalError(
            "Failed to disable rule".to_string(),
        ));
    }

    info!("Disabled rule: {}", id);
    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Execute rule chain immediately
#[utoipa::path(
    post,
    path = "/api/rules/{id}/execute",
    params(("id" = String, Path, description = "Rule identifier")),
    responses(
        (status = 200, description = "Rule execution result", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn execute_rule_now(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let execution_id = format!("manual-{}", uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now();

    // Try to find the rule chain from SQLite database first
    let chain = match rules_repository::get_rule_chain(&state, &id).await {
        Ok(c) => c,
        Err(RuleSrvError::RuleNotFound(_)) => {
            // Fallback: try chains_cache (for testing or in-memory rules)
            let cache = state.chains_cache.read().await;
            match cache.iter().find(|c| c.id == id) {
                Some(c) => c.clone(),
                None => return Err(RuleSrvError::RuleNotFound(id.clone())),
            }
        },
        Err(e) => {
            warn!("Failed to load chain from SQLite, trying cache: {}", e);
            let cache = state.chains_cache.read().await;
            match cache.iter().find(|c| c.id == id) {
                Some(c) => c.clone(),
                None => return Err(RuleSrvError::RuleNotFound(id.clone())),
            }
        },
    };

    // Check enabled flag for cache-loaded rules
    if !chain.enabled {
        return Ok(Json(SuccessResponse::new(json!({
            "result": "skipped",
            "reason": "rule is disabled",
            "chain_id": id,
            "execution_id": execution_id
        }))));
    }

    // Create ChainExecutor and execute
    let executor = ChainExecutor::new(state.rtdb.clone(), state.routing_cache.clone());
    let result = executor.execute(&chain).await?;

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
            "chain_id": result.chain_id,
            "execution_id": execution_id,
            "success": true,
            "actions_executed": action_results,
            "execution_path": result.execution_path,
            "timestamp": timestamp
        }))))
    } else {
        Ok(Json(SuccessResponse::new(json!({
            "result": "failed",
            "chain_id": result.chain_id,
            "execution_id": execution_id,
            "success": false,
            "error": result.error,
            "actions_executed": action_results,
            "execution_path": result.execution_path,
            "timestamp": timestamp
        }))))
    }
}
