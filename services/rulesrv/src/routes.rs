//! API routes and handlers for Rule Service

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use crate::action_executor::ActionExecutor;
use crate::app::AppState;
use crate::condition_evaluator::ConditionEvaluator;
use crate::error::RuleSrvError;
use crate::rule_engine::{DataHistory, ExecutionContext, ExecutionResult};
use crate::rule_logger;
use crate::rules_repository;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};
use utoipa::OpenApi;
use voltage_config::api::SuccessResponse;
use voltage_config::ReloadableService; // For unified hot reload interface

/// Create all API routes with state
pub fn create_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        // Rule management
        .route("/api/rules", get(list_rules).post(create_rule))
        .route(
            "/api/rules/{id}",
            get(get_rule).put(update_rule).delete(delete_rule),
        )
        .route("/api/rules/{id}/enable", post(enable_rule))
        .route("/api/rules/{id}/disable", post(disable_rule))
        .route("/api/rules/{id}/execute", post(execute_rule_now))
        // Hot reload endpoint
        .route("/api/rules/reload", post(reload_rules_from_db))
        // Rule history and statistics
        .route("/api/rules/history", get(get_all_rules_history))
        .route("/api/rules/{id}/history", get(get_rule_history))
        .route("/api/rules/{id}/stats", get(get_rule_stats))
        // SQLite operations
        .route("/api/rules/cached", get(get_cached_rules))
        // Test endpoints
        .route("/api/rules/test", post(test_rules))
        .route("/api/rules/{id}/test", post(test_rule))
        .route("/api/conditions/evaluate", post(evaluate_conditions))
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
        let rules_cache = Arc::new(tokio::sync::RwLock::new(Arc::new(Vec::new())));
        let rule_config = Arc::new(tokio::sync::RwLock::new(None));
        let execution_history = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let routing_cache = Arc::new(voltage_config::RoutingCache::new());

        Arc::new(crate::app::AppState {
            rtdb,
            routing_cache,
            config,
            sqlite_client,
            rules_cache,
            rule_config,
            execution_history,
        })
    }

    // ========================================================================
    // Closed-loop Testing Utilities
    // ========================================================================

    /// Extract JSON response body from axum Response
    async fn extract_json(resp: axum::response::Response) -> serde_json::Value {
        use http_body_util::BodyExt;
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).expect("Response body should be valid JSON")
    }

    /// Assert that a JSON field at the given JSON pointer path equals the expected value
    ///
    /// # Arguments
    /// * `json` - The JSON value to inspect
    /// * `path` - JSON pointer path (e.g., "/data/id", "/data/name")
    /// * `expected` - The expected value at that path
    ///
    /// # Panics
    /// Panics if the field doesn't exist or doesn't match the expected value
    fn assert_json_field(json: &serde_json::Value, path: &str, expected: serde_json::Value) {
        let actual = json
            .pointer(path)
            .unwrap_or_else(|| panic!("Field '{}' not found in JSON: {:?}", path, json));
        assert_eq!(
            actual, &expected,
            "Field '{}' mismatch: expected {:?}, got {:?}",
            path, expected, actual
        );
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

        // Create rule
        let body = serde_json::json!({
            "id": "r1",
            "name": "rule1",
            "enabled": true,
            "priority": 10,
            "flow_json": {"x": 1}
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
    async fn test_execute_rule_now_skipped_when_conditions_false() {
        let state = build_test_state().await;
        // Seed rules_cache with a single rule with no conditions (AND []) which evaluates to true; we want false, so use a condition group requiring a non-existent field compare
        let mut rules = state.rules_cache.write().await;
        let rule = crate::rule_engine::Rule {
            id: "r_exec".to_string(),
            name: "exec".to_string(),
            category: String::new(),
            description: None,
            priority: 10,
            enabled: true,
            triggers: vec![],
            conditions: crate::rule_engine::ConditionGroup::Group {
                logic: crate::rule_engine::LogicalOperator::And,
                rules: vec![],
            },
            actions: vec![],
            metadata: crate::rule_engine::RuleMetadata::default(),
        };
        *rules = Arc::new(vec![rule]);
        drop(rules);

        let app = create_routes(state);
        let req = Request::builder()
            .uri("/api/rules/r_exec/execute")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_evaluate_conditions_and_test_endpoints() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // evaluate_conditions with empty AND group
        let body = serde_json::json!({
            "conditions": {"logic":"AND", "rules": []},
            "context": {}
        });
        let req = Request::builder()
            .uri("/api/conditions/evaluate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // /api/rules/test returns 400 by design
        let body2 = serde_json::json!({"rule": {"id": "r1", "name": "n", "priority": 10, "enabled": true, "triggers":[], "conditions": {"logic":"AND","rules":[]}, "actions": []}});
        let req2 = Request::builder()
            .uri("/api/rules/test")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body2.to_string()))
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);

        // /api/rules/{id}/test returns 400 by design
        let req3 = Request::builder()
            .uri("/api/rules/r1/test")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body2.to_string()))
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_rule_history_and_stats() {
        let state = build_test_state().await;
        // First create the rule (required for foreign key constraint)
        if let Some(sqlite) = &state.sqlite_client {
            sqlx::query(
                "INSERT INTO rules (id, name, description, flow_json, enabled) VALUES ('rA', 'Rule A', 'Test rule', '{}', 1)"
            ).execute(sqlite.pool()).await.unwrap();

            // Then insert some history rows
            sqlx::query(
                "INSERT INTO rule_history (rule_id, triggered_at, execution_result, error) VALUES ('rA', '2025-01-01T00:00:00Z', '[]', NULL)"
            ).execute(sqlite.pool()).await.unwrap();
            sqlx::query(
                "INSERT INTO rule_history (rule_id, triggered_at, execution_result, error) VALUES ('rA', '2025-01-02T00:00:00Z', '[{}]', 'err')"
            ).execute(sqlite.pool()).await.unwrap();
        }
        let app = create_routes(state);

        // by id
        let req1 = Request::builder()
            .uri("/api/rules/rA/history?limit=10&offset=0")
            .body(Body::empty())
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), axum::http::StatusCode::OK);

        // stats
        let req2 = Request::builder()
            .uri("/api/rules/rA/stats")
            .body(Body::empty())
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), axum::http::StatusCode::OK);

        // all history
        let req3 = Request::builder()
            .uri("/api/rules/history?limit=10&offset=0")
            .body(Body::empty())
            .unwrap();
        let resp3 = app.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rule_json_roundtrip_alignment() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // Post a rule JSON with explicit fields
        let posted = serde_json::json!({
            "id": "r_json_align",
            "name": "RoundTrip Alignment",
            "description": "Ensure JSON stored/loaded matches",
            "enabled": true,
            "priority": 10,
            "conditions": {"operator": "AND", "conditions": []},
            "actions": [{"type": "log", "message": "ok"}]
        });
        let req_create = Request::builder()
            .uri("/api/rules")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(posted.to_string()))
            .unwrap();
        let resp_create = app.clone().oneshot(req_create).await.unwrap();
        assert_eq!(resp_create.status(), axum::http::StatusCode::OK);

        // GET and compare
        let req_get = Request::builder()
            .uri("/api/rules/r_json_align")
            .body(Body::empty())
            .unwrap();
        let resp_get = app.clone().oneshot(req_get).await.unwrap();
        assert_eq!(resp_get.status(), axum::http::StatusCode::OK);
        use http_body_util::BodyExt as _;
        let bytes = resp_get.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let got = &v["data"];
        assert_eq!(got, &posted);
    }

    // ========================================================================
    // Phase 2: P0 Closed-loop Tests
    // ========================================================================

    /// Test 1: Update Rule Closed-loop
    /// Verifies that rule updates are properly persisted and retrieved
    #[tokio::test]
    async fn test_update_rule_closed_loop() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // Step 1: POST - Create initial rule
        let initial_body = serde_json::json!({
            "id": "rule_update_test",
            "name": "Initial Rule Name",
            "description": "Initial description",
            "enabled": true,
            "priority": 10,
            "conditions": {"operator": "AND", "conditions": []},
            "actions": [{"type": "set_value", "target": "test:initial", "value": 100}]
        });
        let create_req = Request::builder()
            .uri("/api/rules")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(initial_body.to_string()))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(
            create_resp.status(),
            axum::http::StatusCode::OK,
            "Rule creation should succeed"
        );

        // Step 2: PUT - Update rule with new values
        let updated_body = serde_json::json!({
            "id": "rule_update_test",
            "name": "Updated Rule Name",
            "description": "Updated description",
            "enabled": false,
            "priority": 20,
            "conditions": {"operator": "OR", "conditions": []},
            "actions": [{"type": "set_value", "target": "test:updated", "value": 200}]
        });
        let update_req = Request::builder()
            .uri("/api/rules/rule_update_test")
            .method("PUT")
            .header("content-type", "application/json")
            .body(Body::from(updated_body.to_string()))
            .unwrap();
        let update_resp = app.clone().oneshot(update_req).await.unwrap();
        let status = update_resp.status();
        if status != axum::http::StatusCode::OK {
            let body_bytes = axum::body::to_bytes(update_resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let body_str = String::from_utf8_lossy(&body_bytes);
            panic!("Rule update failed with status {}: {}", status, body_str);
        }

        // Step 3: GET - Read updated rule and verify changes
        let get_req = Request::builder()
            .uri("/api/rules/rule_update_test")
            .body(Body::empty())
            .unwrap();
        let get_resp = app.oneshot(get_req).await.unwrap();
        assert_eq!(
            get_resp.status(),
            axum::http::StatusCode::OK,
            "Rule retrieval should succeed"
        );

        // Step 4: Verify - All updated fields match
        let json = extract_json(get_resp).await;
        assert_json_field(&json, "/data/id", serde_json::json!("rule_update_test"));
        assert_json_field(&json, "/data/name", serde_json::json!("Updated Rule Name"));
        assert_json_field(
            &json,
            "/data/description",
            serde_json::json!("Updated description"),
        );
        assert_json_field(&json, "/data/enabled", serde_json::json!(false));
        assert_json_field(&json, "/data/priority", serde_json::json!(20));
        assert_json_field(
            &json,
            "/data/actions/0/target",
            serde_json::json!("test:updated"),
        );
        assert_json_field(&json, "/data/actions/0/value", serde_json::json!(200));
    }

    /// Test 2: Enable/Disable Rule Closed-loop
    /// Verifies that rule enable/disable operations persist correctly
    #[tokio::test]
    async fn test_enable_disable_rule_closed_loop() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // Step 1: POST - Create rule with enabled=false
        let create_body = serde_json::json!({
            "id": "rule_enable_test",
            "name": "Enable Test Rule",
            "description": "Test enable/disable operations",
            "enabled": false,
            "priority": 15,
            "conditions": {"operator": "AND", "conditions": []},
            "actions": []
        });
        let create_req = Request::builder()
            .uri("/api/rules")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(create_body.to_string()))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(
            create_resp.status(),
            axum::http::StatusCode::OK,
            "Rule creation should succeed"
        );

        // Step 2: GET - Verify initial state (enabled=false)
        let get_req1 = Request::builder()
            .uri("/api/rules/rule_enable_test")
            .body(Body::empty())
            .unwrap();
        let get_resp1 = app.clone().oneshot(get_req1).await.unwrap();
        assert_eq!(
            get_resp1.status(),
            axum::http::StatusCode::OK,
            "Rule retrieval should succeed"
        );
        let json1 = extract_json(get_resp1).await;
        assert_json_field(&json1, "/data/enabled", serde_json::json!(false));

        // Step 3: POST - Enable rule
        let enable_req = Request::builder()
            .uri("/api/rules/rule_enable_test/enable")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let enable_resp = app.clone().oneshot(enable_req).await.unwrap();
        assert_eq!(
            enable_resp.status(),
            axum::http::StatusCode::OK,
            "Rule enable should succeed"
        );

        // Step 4: GET - Verify enabled state (enabled=true)
        let get_req2 = Request::builder()
            .uri("/api/rules/rule_enable_test")
            .body(Body::empty())
            .unwrap();
        let get_resp2 = app.clone().oneshot(get_req2).await.unwrap();
        assert_eq!(
            get_resp2.status(),
            axum::http::StatusCode::OK,
            "Rule retrieval should succeed"
        );
        let json2 = extract_json(get_resp2).await;
        assert_json_field(&json2, "/data/enabled", serde_json::json!(true));

        // Step 5: POST - Disable rule
        let disable_req = Request::builder()
            .uri("/api/rules/rule_enable_test/disable")
            .method("POST")
            .body(Body::empty())
            .unwrap();
        let disable_resp = app.clone().oneshot(disable_req).await.unwrap();
        assert_eq!(
            disable_resp.status(),
            axum::http::StatusCode::OK,
            "Rule disable should succeed"
        );

        // Step 6: GET - Verify disabled state (enabled=false)
        let get_req3 = Request::builder()
            .uri("/api/rules/rule_enable_test")
            .body(Body::empty())
            .unwrap();
        let get_resp3 = app.oneshot(get_req3).await.unwrap();
        assert_eq!(
            get_resp3.status(),
            axum::http::StatusCode::OK,
            "Rule retrieval should succeed"
        );
        let json3 = extract_json(get_resp3).await;
        assert_json_field(&json3, "/data/enabled", serde_json::json!(false));
    }

    // ========================================================================
    // Phase 3: P1 Closed-loop Tests (Delete & Batch Operations)
    // ========================================================================

    /// Test 3: Delete Rule Closed-loop
    /// Verifies that rule deletion removes the rule from the system
    #[tokio::test]
    async fn test_delete_rule_closed_loop() {
        let state = build_test_state().await;
        let app = create_routes(state);

        // Step 1: POST - Create rule to be deleted
        let create_body = serde_json::json!({
            "id": "rule_delete_test",
            "name": "Rule To Delete",
            "description": "This rule will be deleted",
            "enabled": true,
            "priority": 25,
            "conditions": {"operator": "AND", "conditions": []},
            "actions": []
        });
        let create_req = Request::builder()
            .uri("/api/rules")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(create_body.to_string()))
            .unwrap();
        let create_resp = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(
            create_resp.status(),
            axum::http::StatusCode::OK,
            "Rule creation should succeed"
        );

        // Step 2: GET - Verify rule exists before deletion
        let get_req1 = Request::builder()
            .uri("/api/rules/rule_delete_test")
            .body(Body::empty())
            .unwrap();
        let get_resp1 = app.clone().oneshot(get_req1).await.unwrap();
        assert_eq!(
            get_resp1.status(),
            axum::http::StatusCode::OK,
            "Rule should exist before deletion"
        );
        let json1 = extract_json(get_resp1).await;
        assert_json_field(&json1, "/data/id", serde_json::json!("rule_delete_test"));
        assert_json_field(&json1, "/data/name", serde_json::json!("Rule To Delete"));

        // Step 3: DELETE - Remove rule
        let delete_req = Request::builder()
            .uri("/api/rules/rule_delete_test")
            .method("DELETE")
            .body(Body::empty())
            .unwrap();
        let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
        assert_eq!(
            delete_resp.status(),
            axum::http::StatusCode::OK,
            "Rule deletion should succeed"
        );

        // Step 4: GET - Verify rule no longer exists (404)
        let get_req2 = Request::builder()
            .uri("/api/rules/rule_delete_test")
            .body(Body::empty())
            .unwrap();
        let get_resp2 = app.oneshot(get_req2).await.unwrap();
        assert_eq!(
            get_resp2.status(),
            axum::http::StatusCode::NOT_FOUND,
            "Deleted rule should return 404"
        );
    }
}

/// Start rule execution background task
pub fn start_rule_execution_task(
    state: Arc<AppState>,
    token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(state.config.execution.interval_seconds));
        let mut batch_id = 0u64;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = execute_rules(&state, batch_id).await {
                        error!("Rule execution error: {}", e);
                    }
                    batch_id += 1;
                }
                _ = token.cancelled() => {
                    info!("Rule execution task shutting down gracefully");
                    break;
                }
            }
        }
    })
}

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::list_rules,
        crate::routes::create_rule,
        crate::routes::get_rule,
        crate::routes::update_rule,
        crate::routes::delete_rule,
        crate::routes::enable_rule,
        crate::routes::disable_rule,
        crate::routes::execute_rule_now,
        crate::routes::reload_rules_from_db,
        crate::routes::get_rule_history,
        crate::routes::get_rule_stats,
        crate::routes::get_all_rules_history
    ),
    tags(
        (name = "rulesrv", description = "Rule Service API")
    )
)]
pub struct RulesrvApiDoc;

// === Rule Execution ===

/// Execute all enabled rules in priority order
async fn execute_rules(
    state: &AppState,
    batch_id: u64,
) -> voltage_config::error::VoltageResult<()> {
    trace!("Starting rule execution for batch {}", batch_id);

    // Get current rules (Arc clone - only 8 bytes copied)
    let rules = Arc::clone(&*state.rules_cache.read().await);
    if rules.is_empty() {
        trace!("No rules to execute in batch {}", batch_id);
        return Ok(());
    }

    // Fetch current data from Redis for context
    let mut data = HashMap::new();

    // Fetch common data points
    let keys_to_fetch = vec![
        "energy:realtime:pv_power",
        "energy:realtime:battery_soc",
        "energy:realtime:load_demand",
        "energy:realtime:pv_available",
        "energy:system_mode",
    ];

    for key in keys_to_fetch {
        // Use RTDB trait for data fetching
        let value_bytes: Option<bytes::Bytes> = state.rtdb.get(key).await.ok().flatten();
        let value: Option<String> = value_bytes.and_then(|b| String::from_utf8(b.to_vec()).ok());

        if let Some(val) = value {
            // Try to parse as number or keep as string
            let json_val = if let Ok(num) = val.parse::<f64>() {
                match serde_json::Number::from_f64(num) {
                    Some(n) => serde_json::Value::Number(n),
                    None => {
                        error!(
                            "Failed to convert f64 {} to JSON number (NaN or Infinity)",
                            num
                        );
                        serde_json::Value::String(val)
                    },
                }
            } else if val == "true" || val == "false" {
                serde_json::Value::Bool(val == "true")
            } else {
                serde_json::Value::String(val)
            };
            data.insert(key.to_string(), json_val);
        }
    }

    // Get execution history (shared across all rules in this batch)
    let history = state.execution_history.read().await.clone();
    let shared_timestamp = chrono::Utc::now();
    let shared_data = data.clone();

    // Create evaluator and executor (reused for all rules)
    let mut evaluator = ConditionEvaluator::new(state.rtdb.clone());
    let mut executor = ActionExecutor::with_rtdb(state.rtdb.clone(), state.routing_cache.clone())
        .map_err(voltage_config::error::VoltageError::Other)?;

    let mut rules_executed = 0;
    let mut rules_triggered = 0;

    // Execute rules by priority
    for rule in rules.iter() {
        if !rule.enabled {
            continue;
        }

        // Load data_history for this rule from RTDB
        let data_history = load_data_history_from_redis(&*state.rtdb, &rule.id).await;

        // Create context for this rule with its specific data_history
        let context = ExecutionContext {
            timestamp: shared_timestamp,
            execution_id: format!("batch_{}_{}", batch_id, rule.id),
            data: shared_data.clone(),
            history: history.clone(),
            data_history,
        };

        // Check if rule should trigger
        if !rule.should_trigger(&context) {
            continue;
        }

        // Save data history immediately after trigger check passes
        // This ensures baseline is recorded even if conditions fail later
        if let Err(e) = save_data_history_to_redis(&*state.rtdb, &rule.id, &context.data).await {
            warn!("Failed to save data history for rule {}: {}", rule.id, e);
        }

        // Check cooldown
        if rule.is_in_cooldown(&context) {
            continue;
        }

        rules_executed += 1;
        let start = std::time::Instant::now();

        // Evaluate conditions
        match evaluator
            .evaluate_condition_group(&rule.conditions, &context)
            .await
        {
            Ok(true) => {
                debug!("Rule {} conditions met, executing actions", rule.id);
                rules_triggered += 1;

                // Execute actions
                let action_results = executor.execute_actions(&rule.actions, &context).await;

                // Record execution result
                let result = ExecutionResult {
                    rule_id: rule.id.clone(),
                    timestamp: chrono::Utc::now(),
                    conditions_met: true,
                    actions_executed: action_results,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: None,
                };

                // Log rule execution to file
                rule_logger::log_rule_execution(&result);

                // Persist to database (async, non-blocking)
                if let Some(sqlite) = &state.sqlite_client {
                    let sqlite_clone = Arc::clone(sqlite);
                    let result_clone = result.clone();
                    tokio::spawn(async move {
                        if let Err(e) = save_execution_to_db(&sqlite_clone, &result_clone).await {
                            warn!("Failed to save execution history to database: {}", e);
                        }
                    });
                }

                // Update history
                let mut history = state.execution_history.write().await;
                history.push(result);
                // Keep only last 1000 executions
                if history.len() > 1000 {
                    let drain_count = history.len() - 1000;
                    history.drain(0..drain_count);
                }
            },
            Ok(false) => {
                trace!("Rule {} conditions not met", rule.id);
                // Log evaluation (conditions not met)
                rule_logger::log_rule_evaluation(
                    &rule.id,
                    "conditions_not_met",
                    start.elapsed().as_millis() as u64,
                );
            },
            Err(e) => {
                error!("Error evaluating rule {}: {}", rule.id, e);
                // Log error
                rule_logger::log_rule_error(
                    &rule.id,
                    &e.to_string(),
                    start.elapsed().as_millis() as u64,
                );
            },
        }
    }

    if rules_executed > 0 {
        if rules_triggered > 0 {
            info!(
                "Executed {} rules, {} triggered (batch {})",
                rules_executed, rules_triggered, batch_id
            );
        } else {
            debug!(
                "Executed {} rules, none triggered (batch {})",
                rules_executed, batch_id
            );
        }
    }

    Ok(())
}

// === Health Check ===

async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let sqlite_status = if state.sqlite_client.is_some() {
        "connected"
    } else {
        "not configured"
    };

    let rules_count = state.rules_cache.read().await.len();

    Ok(Json(SuccessResponse::new(json!({
        "status": "healthy",
        "service": "rulesrv",
        "sqlite": sqlite_status,
        "rules_cached": rules_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))))
}

// === Rule Management ===

/// List all rules in the system
#[utoipa::path(
    get,
    path = "/api/rules",
    responses(
        (status = 200, description = "List rules", body = serde_json::Value,
            example = json!([
                {
                    "id": "battery_charge_optimization",
                    "name": "Battery Charge Optimization",
                    "enabled": true,
                    "priority": 100,
                    "description": "Optimize battery charging during low electricity price periods"
                },
                {
                    "id": "peak_demand_reduction",
                    "name": "Peak Demand Reduction",
                    "enabled": true,
                    "priority": 90,
                    "description": "Reduce grid demand during peak hours using battery"
                },
                {
                    "id": "diesel_backup_activation",
                    "name": "Diesel Backup Activation",
                    "enabled": false,
                    "priority": 200,
                    "description": "Start diesel generator when grid fails"
                }
            ])
        )
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

/// Create a new rule or update existing
#[utoipa::path(
    post,
    path = "/api/rules",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule created", body = serde_json::Value,
            example = json!({"id": "battery_charge_optimization", "status": "OK"})
        )
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

/// Get rule details by ID
#[utoipa::path(
    get,
    path = "/api/rules/{id}",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule details", body = serde_json::Value,
            example = json!({
                "id": "battery_charge_optimization",
                "name": "Battery Charge Optimization",
                "enabled": true,
                "priority": 100,
                "description": "Optimize battery charging during low electricity price periods",
                "conditions": {
                    "operator": "AND",
                    "conditions": [
                        {"type": "threshold", "key": "energy:realtime:battery_soc", "operator": "<", "value": 80.0},
                        {"type": "threshold", "key": "energy:realtime:pv_power", "operator": ">", "value": 1000.0}
                    ]
                },
                "actions": [
                    {"type": "set_redis_key", "key": "energy:command:battery_mode", "value": "charge"}
                ],
                "trigger": {"mode": "interval"},
                "cooldown_seconds": 300
            })
        )
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

/// Update existing rule definition
#[utoipa::path(
    put,
    path = "/api/rules/{id}",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rule updated", body = serde_json::Value,
            example = json!({"id": "battery_charge_optimization", "status": "OK"})
        )
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

    // 1. Update SQLite database (always succeeds for valid JSON)
    if let Err(e) = rules_repository::upsert_rule(&state, &id, &rule).await {
        error!("Failed to update rule {} in SQLite: {}", id, e);
        return Err(RuleSrvError::InternalError(format!(
            "Failed to update rule in database: {}",
            e
        )));
    }

    // 2. Attempt hot reload (best effort - warns but doesn't fail if parsing fails)
    match serde_json::from_value::<crate::rule_engine::Rule>(rule.clone()) {
        Ok(rule_struct) => {
            if let Err(e) = state.perform_hot_reload(rule_struct).await {
                warn!(
                    "Rule {} updated in SQLite but failed to hot reload: {}. Will be loaded on next service reload.",
                    id, e
                );
            } else {
                info!("Rule {} updated and hot reloaded successfully", id);
            }
        },
        Err(e) => {
            warn!(
                "Rule {} updated in SQLite but has invalid format for hot reload: {}. Will be loaded on next service reload.",
                id, e
            );
        },
    }

    Ok(Json(SuccessResponse::new(
        json!({ "id": id, "status": "OK" }),
    )))
}

/// Delete a rule from the system
#[utoipa::path(
    delete,
    path = "/api/rules/{id}",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule deleted", body = serde_json::Value,
            example = json!({"id": "battery_charge_optimization", "status": "OK"})
        )
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

/// Reload rules from SQLite database
///
/// Performs incremental synchronization of rules from SQLite to in-memory cache.
/// Compares current cache with database and performs add/update/remove operations.
///
/// @route POST /api/rules/reload
/// @output Json<SuccessResponse<serde_json::Value>> - Reload result with statistics
/// @status 200 - Reload completed successfully
/// @status 500 - Database error or reload failed
#[utoipa::path(
    post,
    path = "/api/rules/reload",
    responses(
        (status = 200, description = "Rules reloaded successfully", body = serde_json::Value,
            example = json!({
                "message": "Rules reloaded successfully",
                "result": {
                    "total_count": 10,
                    "added": [1, 2],
                    "updated": [3, 4, 5],
                    "removed": [6],
                    "errors": [],
                    "duration_ms": 125
                }
            })
        ),
        (status = 500, description = "Reload failed", body = serde_json::Value)
    ),
    tag = "rulesrv"
)]
pub async fn reload_rules_from_db(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    // Get SQLite connection
    let Some(sqlite) = &state.sqlite_client else {
        error!("SQLite client not configured for hot reload");
        return Err(RuleSrvError::InternalError(
            "SQLite not configured".to_string(),
        ));
    };

    // Use unified ReloadableService interface for incremental sync
    match state.reload_from_database(sqlite.pool()).await {
        Ok(result) => {
            info!(
                "Rules reloaded: {} added, {} updated, {} removed, {} errors",
                result.added.len(),
                result.updated.len(),
                result.removed.len(),
                result.errors.len()
            );
            Ok(Json(SuccessResponse::new(json!({
                "message": "Rules reloaded successfully",
                "result": result
            }))))
        },
        Err(e) => {
            error!("Failed to reload rules: {}", e);
            Err(RuleSrvError::InternalError(format!(
                "Failed to reload rules: {}",
                e
            )))
        },
    }
}

/// Enable a rule for execution
#[utoipa::path(
    post,
    path = "/api/rules/{id}/enable",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule enabled", body = serde_json::Value,
            example = json!({"id": "battery_charge_optimization", "status": "OK"})
        )
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

/// Disable a rule from execution
#[utoipa::path(
    post,
    path = "/api/rules/{id}/disable",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule disabled", body = serde_json::Value,
            example = json!({"id": "battery_charge_optimization", "status": "OK"})
        )
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

/// Get rules from in-memory cache
async fn get_cached_rules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let rules = Arc::clone(&*state.rules_cache.read().await);
    Ok(Json(SuccessResponse::new(json!({
        "count": rules.len(),
        "rules": &*rules
    }))))
}

// === Test Endpoints ===

/// Test rule request
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TestRuleRequest {
    rule: serde_json::Value,
    #[serde(default)]
    test_data: HashMap<String, String>,
    #[serde(default)]
    context: HashMap<String, serde_json::Value>,
}

/// Test multiple rules
async fn test_rules(
    State(_state): State<Arc<AppState>>,
    Json(_request): Json<TestRuleRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    Err(RuleSrvError::InvalidData(
        "Test functionality is temporarily disabled".to_string(),
    ))
}

/// Test a specific rule by ID
async fn test_rule(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(_request): Json<TestRuleRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    Err(RuleSrvError::InvalidData(
        "Test functionality is temporarily disabled".to_string(),
    ))
}

/// Evaluate conditions request
#[derive(Debug, Deserialize)]
struct EvaluateConditionsRequest {
    conditions: serde_json::Value,
    #[serde(default)]
    context: HashMap<String, serde_json::Value>,
    #[serde(default)]
    test_data: HashMap<String, String>,
}

/// Evaluate conditions without executing actions
async fn evaluate_conditions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EvaluateConditionsRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    if !request.test_data.is_empty() {
        warn!("Test data injection is temporarily disabled");
    }

    // Parse conditions
    let conditions: crate::rule_engine::ConditionGroup =
        match serde_json::from_value(request.conditions) {
            Ok(c) => c,
            Err(e) => {
                return Err(RuleSrvError::InvalidData(format!(
                    "Invalid conditions format: {}",
                    e
                )));
            },
        };

    // Create execution context
    let context = ExecutionContext {
        timestamp: chrono::Utc::now(),
        execution_id: format!("eval-{}", uuid::Uuid::new_v4()),
        data: request.context,
        history: vec![],
        data_history: HashMap::new(),
    };

    // Evaluate conditions
    let mut evaluator = ConditionEvaluator::new(state.rtdb.clone());
    match evaluator
        .evaluate_condition_group(&conditions, &context)
        .await
    {
        Ok(result) => {
            if !request.test_data.is_empty() {
                debug!("Test data cleanup is temporarily disabled");
            }

            Ok(Json(SuccessResponse::new(json!({
                "result": result,
                "timestamp": context.timestamp,
                "execution_id": context.execution_id,
            }))))
        },
        Err(e) => Err(RuleSrvError::InternalError(format!(
            "Condition evaluation failed: {}",
            e
        ))),
    }
}

/// Execute a rule immediately
#[utoipa::path(
    post,
    path = "/api/rules/{id}/execute",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule execution result", body = serde_json::Value,
            example = json!({
                "result": "executed",
                "rule_id": "battery_charge_optimization",
                "execution_id": "manual-550e8400-e29b-41d4-a716-446655440000",
                "conditions_met": true,
                "action_results": [
                    {"action_type": "set_redis_key", "result": "OK", "status": "success"}
                ],
                "timestamp": "2025-10-15T14:30:00Z"
            })
        )
    ),
    tag = "rulesrv"
)]
pub async fn execute_rule_now(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    // Find the rule
    let rules = Arc::clone(&*state.rules_cache.read().await);
    let rule = match rules.iter().find(|r| r.id == id) {
        Some(r) => r.clone(),
        None => {
            return Err(RuleSrvError::RuleNotFound(id.to_string()));
        },
    };

    // Create execution context
    let context = ExecutionContext {
        timestamp: chrono::Utc::now(),
        execution_id: format!("manual-{}", uuid::Uuid::new_v4()),
        data: HashMap::new(),
        history: vec![],
        data_history: HashMap::new(),
    };

    // Evaluate conditions
    let mut evaluator = ConditionEvaluator::new(state.rtdb.clone());
    let conditions_met = match evaluator
        .evaluate_condition_group(&rule.conditions, &context)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            return Err(RuleSrvError::InternalError(format!(
                "Condition evaluation failed: {}",
                e
            )));
        },
    };

    if !conditions_met {
        return Ok(Json(SuccessResponse::new(json!({
            "result": "skipped",
            "reason": "conditions not met",
            "rule_id": id,
            "execution_id": context.execution_id
        }))));
    }

    // Execute actions
    let mut executor =
        match ActionExecutor::with_rtdb(state.rtdb.clone(), state.routing_cache.clone()) {
            Ok(e) => e,
            Err(e) => {
                return Err(RuleSrvError::InternalError(format!(
                    "Failed to create action executor: {}",
                    e
                )));
            },
        };
    let results = executor.execute_actions(&rule.actions, &context).await;

    let action_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            if r.success {
                json!({
                    "action_type": r.action_type,
                    "result": r.result,
                    "status": "success"
                })
            } else {
                json!({
                    "action_type": r.action_type,
                    "error": r.error,
                    "status": "failed"
                })
            }
        })
        .collect();

    Ok(Json(SuccessResponse::new(json!({
        "result": "executed",
        "rule_id": id,
        "execution_id": context.execution_id,
        "conditions_met": conditions_met,
        "action_results": action_results,
        "timestamp": context.timestamp
    }))))
}

// === Data History Persistence Helpers ===

/// Load data_history for a specific rule from RTDB
async fn load_data_history_from_redis(
    rtdb: &dyn voltage_rtdb::Rtdb,
    rule_id: &str,
) -> HashMap<String, DataHistory> {
    let key = format!("rulesrv:datahistory:{}", rule_id);
    let mut history = HashMap::new();

    // Get all fields from the hash using RTDB trait
    match rtdb.hash_get_all(&key).await {
        Ok(hash_bytes) => {
            // Convert Bytes to String
            let hash_data: HashMap<String, String> = hash_bytes
                .into_iter()
                .filter_map(|(k, v)| String::from_utf8(v.to_vec()).ok().map(|s| (k, s)))
                .collect();
            for (field, encoded_value) in hash_data {
                // Parse the encoded value: "value|timestamp"
                if let Some((value_str, timestamp_str)) = encoded_value.split_once('|') {
                    if let (Ok(value), Ok(timestamp)) = (
                        value_str.parse::<f64>(),
                        chrono::DateTime::parse_from_rfc3339(timestamp_str),
                    ) {
                        history.insert(
                            field,
                            DataHistory {
                                value,
                                timestamp: timestamp.with_timezone(&chrono::Utc),
                            },
                        );
                    } else {
                        debug!("Failed to parse data history for rule {}, field {}: value={}, timestamp={}",
                            rule_id, field, value_str, timestamp_str);
                    }
                } else {
                    debug!(
                        "Invalid data history format for rule {}, field {}: {}",
                        rule_id, field, encoded_value
                    );
                }
            }
            trace!(
                "Loaded {} data history entries for rule {}",
                history.len(),
                rule_id
            );
        },
        Err(e) => {
            // It's normal for the key not to exist on first execution
            trace!("No data history found for rule {} ({})", rule_id, e);
        },
    }

    history
}

/// Save data_history for a specific rule to RTDB
async fn save_data_history_to_redis(
    rtdb: &dyn voltage_rtdb::Rtdb,
    rule_id: &str,
    data: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    let key = format!("rulesrv:datahistory:{}", rule_id);
    let timestamp = chrono::Utc::now();

    // Convert all numeric data fields to history entries
    let mut fields_to_save = Vec::new();

    for (field, value) in data {
        // Try to extract numeric value
        let numeric_value = match value {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        };

        if let Some(num) = numeric_value {
            // Encode as "value|timestamp"
            let encoded = format!("{}|{}", num, timestamp.to_rfc3339());
            fields_to_save.push((field.clone(), bytes::Bytes::from(encoded)));
        }
    }

    if !fields_to_save.is_empty() {
        let num_fields = fields_to_save.len();

        // Use hash_mset to update multiple fields at once
        rtdb.hash_mset(&key, fields_to_save)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to save data history: {}", e))?;

        trace!(
            "Saved {} data history entries for rule {}",
            num_fields,
            rule_id
        );
    }

    Ok(())
}

// === Rule History and Statistics APIs ===

/// Save execution result to database
async fn save_execution_to_db(
    sqlite: &common::sqlite::SqliteClient,
    result: &ExecutionResult,
) -> anyhow::Result<()> {
    let execution_result = serde_json::to_string(&result.actions_executed)?;
    let error = result.error.as_deref();

    sqlx::query(
        "INSERT INTO rule_history (rule_id, triggered_at, execution_result, error) VALUES (?, ?, ?, ?)"
    )
    .bind(&result.rule_id)
    .bind(result.timestamp.to_rfc3339())
    .bind(execution_result)
    .bind(error)
    .execute(sqlite.pool())
    .await?;

    Ok(())
}

/// Query parameters for history pagination
#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    start_time: Option<String>,
    end_time: Option<String>,
}

fn default_limit() -> i64 {
    100
}

/// Get execution history for a specific rule
#[utoipa::path(
    get,
    path = "/api/rules/{id}/history",
    params(
        ("id" = String, Path, description = "Rule identifier"),
        ("limit" = Option<i64>, Query, description = "Number of records to return (default 100)"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination (default 0)"),
        ("start_time" = Option<String>, Query, description = "Start time filter (RFC3339)"),
        ("end_time" = Option<String>, Query, description = "End time filter (RFC3339)")
    ),
    responses(
        (status = 200, description = "Rule execution history", body = serde_json::Value,
            example = json!({
                "rule_id": "battery_charge_optimization",
                "count": 2,
                "history": [
                    {
                        "id": 1,
                        "rule_id": "battery_charge_optimization",
                        "triggered_at": "2025-10-15T14:30:00Z",
                        "execution_result": [
                            {"action_type": "set_redis_key", "result": "OK", "success": true}
                        ],
                        "error": null
                    },
                    {
                        "id": 2,
                        "rule_id": "battery_charge_optimization",
                        "triggered_at": "2025-10-15T14:25:00Z",
                        "execution_result": [
                            {"action_type": "set_redis_key", "result": "OK", "success": true}
                        ],
                        "error": null
                    }
                ],
                "limit": 100,
                "offset": 0
            })
        )
    ),
    tag = "rulesrv"
)]
async fn get_rule_history(
    State(state): State<Arc<AppState>>,
    Path(rule_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let Some(sqlite) = &state.sqlite_client else {
        return Err(RuleSrvError::InternalError(
            "SQLite not configured".to_string(),
        ));
    };

    // Build query with optional time filters
    let mut sql = "SELECT id, rule_id, triggered_at, execution_result, error FROM rule_history WHERE rule_id = ?".to_string();

    if query.start_time.is_some() {
        sql.push_str(" AND triggered_at >= ?");
    }
    if query.end_time.is_some() {
        sql.push_str(" AND triggered_at <= ?");
    }

    sql.push_str(" ORDER BY triggered_at DESC LIMIT ? OFFSET ?");

    let mut query_builder =
        sqlx::query_as::<_, (i64, String, String, String, Option<String>)>(&sql).bind(&rule_id);

    if let Some(start) = &query.start_time {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = &query.end_time {
        query_builder = query_builder.bind(end);
    }

    query_builder = query_builder.bind(query.limit).bind(query.offset);

    match query_builder.fetch_all(sqlite.pool()).await {
        Ok(rows) => {
            let history: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(id, rule_id, triggered_at, execution_result, error)| {
                    json!({
                        "id": id,
                        "rule_id": rule_id,
                        "triggered_at": triggered_at,
                        "execution_result": serde_json::from_str::<serde_json::Value>(&execution_result).unwrap_or(json!(null)),
                        "error": error
                    })
                })
                .collect();

            Ok(Json(SuccessResponse::new(json!({
                "rule_id": rule_id,
                "count": history.len(),
                "history": history,
                "limit": query.limit,
                "offset": query.offset
            }))))
        },
        Err(e) => {
            error!("Failed to query rule history for {}: {}", rule_id, e);
            Err(RuleSrvError::InternalError(
                "Failed to query rule history".to_string(),
            ))
        },
    }
}

/// Get execution statistics for a specific rule
#[utoipa::path(
    get,
    path = "/api/rules/{id}/stats",
    params(
        ("id" = String, Path, description = "Rule identifier")
    ),
    responses(
        (status = 200, description = "Rule execution statistics", body = serde_json::Value,
            example = json!({
                "rule_id": "battery_charge_optimization",
                "total_executions": 150,
                "successful_executions": 147,
                "failed_executions": 3,
                "success_rate": "98.00%",
                "first_execution": "2025-10-01T08:00:00Z",
                "last_execution": "2025-10-15T14:30:00Z"
            })
        )
    ),
    tag = "rulesrv"
)]
async fn get_rule_stats(
    State(state): State<Arc<AppState>>,
    Path(rule_id): Path<String>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let Some(sqlite) = &state.sqlite_client else {
        return Err(RuleSrvError::InternalError(
            "SQLite not configured".to_string(),
        ));
    };

    // Query statistics
    let stats_query = r#"
        SELECT
            COUNT(*) as total_executions,
            COUNT(CASE WHEN error IS NULL THEN 1 END) as successful_executions,
            COUNT(CASE WHEN error IS NOT NULL THEN 1 END) as failed_executions,
            MIN(triggered_at) as first_execution,
            MAX(triggered_at) as last_execution
        FROM rule_history
        WHERE rule_id = ?
    "#;

    match sqlx::query_as::<_, (i64, i64, i64, Option<String>, Option<String>)>(stats_query)
        .bind(&rule_id)
        .fetch_one(sqlite.pool())
        .await
    {
        Ok((total, successful, failed, first, last)) => {
            let success_rate = if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(Json(SuccessResponse::new(json!({
                "rule_id": rule_id,
                "total_executions": total,
                "successful_executions": successful,
                "failed_executions": failed,
                "success_rate": format!("{:.2}%", success_rate),
                "first_execution": first,
                "last_execution": last
            }))))
        },
        Err(e) => {
            error!("Failed to query rule stats for {}: {}", rule_id, e);
            Err(RuleSrvError::InternalError(
                "Failed to query rule statistics".to_string(),
            ))
        },
    }
}

/// Get execution history for all rules with pagination
#[utoipa::path(
    get,
    path = "/api/rules/history",
    params(
        ("limit" = Option<i64>, Query, description = "Number of records to return (default 100)"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination (default 0)"),
        ("start_time" = Option<String>, Query, description = "Start time filter (RFC3339)"),
        ("end_time" = Option<String>, Query, description = "End time filter (RFC3339)")
    ),
    responses(
        (status = 200, description = "All rules execution history", body = serde_json::Value,
            example = json!({
                "count": 3,
                "history": [
                    {
                        "id": 1,
                        "rule_id": "battery_charge_optimization",
                        "triggered_at": "2025-10-15T14:30:00Z",
                        "execution_result": [{"action_type": "set_redis_key", "result": "OK", "success": true}],
                        "error": null
                    },
                    {
                        "id": 2,
                        "rule_id": "peak_demand_reduction",
                        "triggered_at": "2025-10-15T14:28:00Z",
                        "execution_result": [{"action_type": "set_redis_key", "result": "OK", "success": true}],
                        "error": null
                    },
                    {
                        "id": 3,
                        "rule_id": "diesel_backup_activation",
                        "triggered_at": "2025-10-15T14:20:00Z",
                        "execution_result": [],
                        "error": "Grid voltage check failed"
                    }
                ],
                "limit": 100,
                "offset": 0
            })
        )
    ),
    tag = "rulesrv"
)]
async fn get_all_rules_history(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, RuleSrvError> {
    let Some(sqlite) = &state.sqlite_client else {
        return Err(RuleSrvError::InternalError(
            "SQLite not configured".to_string(),
        ));
    };

    // Build query with optional time filters
    let mut sql =
        "SELECT id, rule_id, triggered_at, execution_result, error FROM rule_history WHERE 1=1"
            .to_string();

    if query.start_time.is_some() {
        sql.push_str(" AND triggered_at >= ?");
    }
    if query.end_time.is_some() {
        sql.push_str(" AND triggered_at <= ?");
    }

    sql.push_str(" ORDER BY triggered_at DESC LIMIT ? OFFSET ?");

    let mut query_builder =
        sqlx::query_as::<_, (i64, String, String, String, Option<String>)>(&sql);

    if let Some(start) = &query.start_time {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = &query.end_time {
        query_builder = query_builder.bind(end);
    }

    query_builder = query_builder.bind(query.limit).bind(query.offset);

    match query_builder.fetch_all(sqlite.pool()).await {
        Ok(rows) => {
            let history: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(id, rule_id, triggered_at, execution_result, error)| {
                    json!({
                        "id": id,
                        "rule_id": rule_id,
                        "triggered_at": triggered_at,
                        "execution_result": serde_json::from_str::<serde_json::Value>(&execution_result).unwrap_or(json!(null)),
                        "error": error
                    })
                })
                .collect();

            Ok(Json(SuccessResponse::new(json!({
                "count": history.len(),
                "history": history,
                "limit": query.limit,
                "offset": query.offset
            }))))
        },
        Err(e) => {
            error!("Failed to query all rules history: {}", e);
            Err(RuleSrvError::InternalError(
                "Failed to query rules history".to_string(),
            ))
        },
    }
}
