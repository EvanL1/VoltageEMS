use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};

use crate::engine::RuleExecutor;
use crate::redis::RedisStore;
use crate::rules::{Rule, RuleGroup};

/// API state shared across handlers
pub struct ApiState {
    pub executor: Arc<RuleExecutor>,
    pub store: Arc<RedisStore>,
}

/// API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": self.code,
                    "message": self.message,
                }
            })),
        )
            .into_response()
    }
}

/// Convert Result to API response
pub fn handle_result<T: Serialize>(result: Result<T>) -> impl IntoResponse {
    match result {
        Ok(data) => (StatusCode::OK, Json(json!({ "data": data }))).into_response(),
        Err(e) => ApiError {
            code: "INTERNAL_ERROR".to_string(),
            message: e.to_string(),
        }
        .into_response(),
    }
}

/// List rules query parameters
#[derive(Debug, Deserialize)]
pub struct ListRulesQuery {
    pub group_id: Option<String>,
    pub enabled: Option<bool>,
}

/// List all rules
pub async fn list_rules(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ListRulesQuery>,
) -> impl IntoResponse {
    let result = async {
        let mut rules = state.store.list_rules().await?;

        // Filter by group if specified
        if let Some(group_id) = query.group_id {
            rules.retain(|r| r.group_id.as_ref() == Some(&group_id));
        }

        // Filter by enabled status if specified
        if let Some(enabled) = query.enabled {
            rules.retain(|r| r.enabled == enabled);
        }

        Ok::<Vec<Rule>, anyhow::Error>(rules)
    }
    .await;

    handle_result(result)
}

/// Get a specific rule
pub async fn get_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    let result = async {
        match state.store.get_rule(&rule_id).await? {
            Some(rule) => Ok(rule),
            None => Err(anyhow::anyhow!("Rule not found: {}", rule_id)),
        }
    }
    .await;

    handle_result(result)
}

/// Create rule request
#[derive(Debug, Deserialize)]
pub struct CreateRuleRequest {
    pub rule: Rule,
}

/// Create a new rule
pub async fn create_rule(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    let result = async {
        let rule = request.rule;

        // Validate rule
        if rule.id.is_empty() {
            return Err(anyhow::anyhow!("Rule ID cannot be empty"));
        }

        // Check if rule already exists
        if state.store.get_rule(&rule.id).await?.is_some() {
            return Err(anyhow::anyhow!("Rule already exists: {}", rule.id));
        }

        // Save rule
        state.store.save_rule(&rule).await?;

        info!("Created rule: {} ({})", rule.name, rule.id);
        Ok(rule)
    }
    .await;

    handle_result(result)
}

/// Update rule request
#[derive(Debug, Deserialize)]
pub struct UpdateRuleRequest {
    pub rule: Rule,
}

/// Update an existing rule
pub async fn update_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
    Json(request): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    let result = async {
        let rule = request.rule;

        // Ensure rule ID matches path
        if rule.id != rule_id {
            return Err(anyhow::anyhow!("Rule ID mismatch"));
        }

        // Check if rule exists
        if state.store.get_rule(&rule_id).await?.is_none() {
            return Err(anyhow::anyhow!("Rule not found: {}", rule_id));
        }

        // Update rule
        state.store.save_rule(&rule).await?;

        info!("Updated rule: {} ({})", rule.name, rule.id);
        Ok(rule)
    }
    .await;

    handle_result(result)
}

/// Delete a rule
pub async fn delete_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    let result = async {
        let deleted = state.store.delete_rule(&rule_id).await?;

        if deleted {
            info!("Deleted rule: {}", rule_id);
            Ok(json!({ "deleted": true }))
        } else {
            Err(anyhow::anyhow!("Rule not found: {}", rule_id))
        }
    }
    .await;

    handle_result(result)
}

/// Execute rule request
#[derive(Debug, Deserialize)]
pub struct ExecuteRuleRequest {
    pub input: Option<Value>,
}

/// Execute a rule
pub async fn execute_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
    Json(request): Json<ExecuteRuleRequest>,
) -> impl IntoResponse {
    let result = async {
        debug!(
            "Executing rule: {} with input: {:?}",
            rule_id, request.input
        );

        let result = state.executor.execute_rule(&rule_id, request.input).await?;

        Ok(result)
    }
    .await;

    handle_result(result)
}

/// Test rule request
#[derive(Debug, Deserialize)]
pub struct TestRuleRequest {
    pub rule: Rule,
    pub input: Option<Value>,
}

/// Test a rule without saving
pub async fn test_rule(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TestRuleRequest>,
) -> impl IntoResponse {
    let rule = request.rule;
    let input = request.input;

    debug!("Testing rule: {} with input: {:?}", rule.name, input);

    // Temporarily save rule for testing
    let temp_id = format!("test_{}", uuid::Uuid::new_v4());
    let mut test_rule = rule.clone();
    test_rule.id = temp_id.clone();

    let save_result = state.store.save_rule(&test_rule).await;
    if let Err(e) = save_result {
        return handle_result::<Value>(Err(e));
    }

    // Execute test
    let result = state.executor.execute_rule(&temp_id, input).await;

    // Clean up
    let _ = state.store.delete_rule(&temp_id).await;

    handle_result(result.map_err(|e| e.into()))
}

/// Get rule execution history
pub async fn get_rule_history(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
    Query(query): Query<ListHistoryQuery>,
) -> impl IntoResponse {
    let result = async {
        let limit = query.limit.unwrap_or(100).min(1000);
        let history = state.store.get_execution_history(&rule_id, limit).await?;

        Ok(history)
    }
    .await;

    handle_result(result)
}

/// List history query parameters
#[derive(Debug, Deserialize)]
pub struct ListHistoryQuery {
    pub limit: Option<usize>,
}

/// List all rule groups
pub async fn list_rule_groups(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let result = state.store.list_rule_groups().await;
    handle_result(result)
}

/// Get a specific rule group
pub async fn get_rule_group(
    State(state): State<Arc<ApiState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let result = async {
        match state.store.get_rule_group(&group_id).await? {
            Some(group) => Ok(group),
            None => Err(anyhow::anyhow!("Rule group not found: {}", group_id)),
        }
    }
    .await;

    handle_result(result)
}

/// Create rule group request
#[derive(Debug, Deserialize)]
pub struct CreateRuleGroupRequest {
    pub group: RuleGroup,
}

/// Create a new rule group
pub async fn create_rule_group(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateRuleGroupRequest>,
) -> impl IntoResponse {
    let result = async {
        let group = request.group;

        // Validate group
        if group.id.is_empty() {
            return Err(anyhow::anyhow!("Group ID cannot be empty"));
        }

        // Check if group already exists
        if state.store.get_rule_group(&group.id).await?.is_some() {
            return Err(anyhow::anyhow!("Rule group already exists: {}", group.id));
        }

        // Save group
        state.store.save_rule_group(&group).await?;

        info!("Created rule group: {} ({})", group.name, group.id);
        Ok(group)
    }
    .await;

    handle_result(result)
}

/// Delete a rule group
pub async fn delete_rule_group(
    State(state): State<Arc<ApiState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let result = async {
        let deleted = state.store.delete_rule_group(&group_id).await?;

        if deleted {
            info!("Deleted rule group: {}", group_id);
            Ok(json!({ "deleted": true }))
        } else {
            Err(anyhow::anyhow!("Rule group not found: {}", group_id))
        }
    }
    .await;

    handle_result(result)
}

/// Get rules in a group
pub async fn get_group_rules(
    State(state): State<Arc<ApiState>>,
    Path(group_id): Path<String>,
) -> impl IntoResponse {
    let result = state.store.get_group_rules(&group_id).await;
    handle_result(result)
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub redis_connected: bool,
}

/// Health check endpoint
pub async fn health_check(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let redis_connected = state
        .store
        .list_rules()
        .await
        .map(|_| true)
        .unwrap_or(false);

    let response = HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        redis_connected,
    };

    (StatusCode::OK, Json(response))
}
