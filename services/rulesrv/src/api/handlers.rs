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
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::engine::{Rule, RuleEngine};
use crate::redis::RedisStore;

/// API state for the rule engine
pub struct ApiState {
    pub engine: Arc<RwLock<RuleEngine>>,
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
    pub enabled: Option<bool>,
    pub limit: Option<usize>,
}

/// List all rules
pub async fn list_rules(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ListRulesQuery>,
) -> impl IntoResponse {
    let result = async {
        let engine = state.engine.read().await;
        let mut rules = engine.list_rules().await?;

        // Filter by enabled status if specified
        if let Some(enabled) = query.enabled {
            rules.retain(|r| r.enabled == enabled);
        }

        // Apply limit if specified
        if let Some(limit) = query.limit {
            rules.truncate(limit);
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
        let rule_key = format!("rulesrv:rule:{}", rule_id);
        match state.store.get_string(&rule_key).await? {
            Some(rule_json) => {
                let rule: Rule = serde_json::from_str(&rule_json)?;
                Ok(rule)
            }
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

        if rule.name.is_empty() {
            return Err(anyhow::anyhow!("Rule name cannot be empty"));
        }

        // Check if rule already exists
        let rule_key = format!("rulesrv:rule:{}", rule.id);
        if state.store.get_string(&rule_key).await?.is_some() {
            return Err(anyhow::anyhow!("Rule already exists: {}", rule.id));
        }

        // Store rule
        let engine = state.engine.read().await;
        engine.store_rule(&rule).await?;

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
        let rule_key = format!("rulesrv:rule:{}", rule_id);
        if state.store.get_string(&rule_key).await?.is_none() {
            return Err(anyhow::anyhow!("Rule not found: {}", rule_id));
        }

        // Update rule
        let engine = state.engine.read().await;
        engine.store_rule(&rule).await?;

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
        let rule_key = format!("rulesrv:rule:{}", rule_id);

        // Check if rule exists
        if state.store.get_string(&rule_key).await?.is_none() {
            return Err(anyhow::anyhow!("Rule not found: {}", rule_id));
        }

        // Delete rule
        state.store.delete(&rule_key).await?;

        // Also delete any execution results and statistics
        let stats_key = format!("rulesrv:rule:{}:stats", rule_id);
        let _ = state.store.delete(&stats_key).await;

        info!("Deleted rule: {}", rule_id);
        Ok(json!({ "deleted": true }))
    }
    .await;

    handle_result(result)
}

/// Execute rule request
#[derive(Debug, Deserialize)]
pub struct ExecuteRuleRequest {
    /// Optional context data for rule evaluation
    #[allow(dead_code)]
    pub context: Option<Value>,
}

/// Execute a rule
pub async fn execute_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
    Json(_request): Json<ExecuteRuleRequest>,
) -> impl IntoResponse {
    let result = async {
        debug!("Executing rule: {}", rule_id);

        let mut engine = state.engine.write().await;
        let result = engine.execute_rule(&rule_id).await?;

        Ok(result)
    }
    .await;

    handle_result(result)
}

/// Test rule request
#[derive(Debug, Deserialize)]
pub struct TestRuleRequest {
    pub rule: Rule,
    #[allow(dead_code)]
    pub context: Option<Value>,
}

/// Test a rule without saving
pub async fn test_rule(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TestRuleRequest>,
) -> impl IntoResponse {
    let result = async {
        let rule = request.rule;

        debug!("Testing rule: {}", rule.name);

        // Create a temporary rule ID for testing
        let temp_id = format!("test_{}", uuid::Uuid::new_v4());
        let mut test_rule = rule.clone();
        test_rule.id = temp_id.clone();

        // Temporarily store the rule
        let engine = state.engine.read().await;
        engine.store_rule(&test_rule).await?;
        drop(engine);

        // Execute the test rule
        let mut engine = state.engine.write().await;
        let result = engine.execute_rule(&temp_id).await;
        drop(engine);

        // Clean up the temporary rule
        let temp_key = format!("rulesrv:rule:{}", temp_id);
        let _ = state.store.delete(&temp_key).await;

        result.map_err(|e| e.into())
    }
    .await;

    handle_result(result)
}

/// Get rule execution history
pub async fn get_rule_history(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
    Query(query): Query<ListHistoryQuery>,
) -> impl IntoResponse {
    let result = async {
        let _limit = query.limit.unwrap_or(100).min(1000);

        // TODO: Implement proper history retrieval
        // For now, return basic stats
        let stats_key = format!("rulesrv:rule:{}:stats", rule_id);
        match state.store.get_string(&stats_key).await? {
            Some(stats_json) => {
                let stats: Value = serde_json::from_str(&stats_json)?;
                Ok(json!({
                    "rule_id": rule_id,
                    "stats": stats,
                    "history": []
                }))
            }
            None => Ok(json!({
                "rule_id": rule_id,
                "stats": {},
                "history": []
            })),
        }
    }
    .await;

    handle_result(result)
}

/// List history query parameters
#[derive(Debug, Deserialize)]
pub struct ListHistoryQuery {
    pub limit: Option<usize>,
}

/// Get rule statistics
pub async fn get_rule_stats(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    let result = async {
        let stats_key = format!("rulesrv:rule:{}:stats", rule_id);
        match state.store.get_string(&stats_key).await? {
            Some(stats_json) => {
                let stats: Value = serde_json::from_str(&stats_json)?;
                Ok(stats)
            }
            None => Ok(json!({
                "rule_id": rule_id,
                "last_execution": null,
                "last_result": null,
                "conditions_met": null
            })),
        }
    }
    .await;

    handle_result(result)
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub engine: String,
    pub redis_connected: bool,
}

/// Health check endpoint for rule engine
pub async fn health_check(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let redis_connected = state
        .store
        .get_string("rulesrv:health_check")
        .await
        .map(|_| true)
        .unwrap_or(false);

    let response = HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        engine: "rule_engine".to_string(),
        redis_connected,
    };

    (StatusCode::OK, Json(response))
}

/// Example rules response for documentation
#[derive(Debug, Serialize)]
pub struct ExampleRulesResponse {
    pub examples: Vec<Rule>,
}

/// Get example rules for documentation/testing
pub async fn get_example_rules() -> impl IntoResponse {
    use crate::engine::{
        ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator,
        Rule, RuleAction,
    };

    let examples = vec![
        Rule {
            id: "battery_low_start_generator".to_string(),
            name: "Start Generator on Low Battery".to_string(),
            description: Some(
                "Start diesel generator when battery SOC drops below 20%".to_string(),
            ),
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "battery.soc".to_string(),
                    operator: ComparisonOperator::LessThanOrEqual,
                    value: json!(20.0),
                    description: Some("Battery SOC at or below 20%".to_string()),
                }],
            },
            actions: vec![
                RuleAction {
                    action_type: ActionType::DeviceControl,
                    config: ActionConfig::DeviceControl {
                        device_id: "generator_001".to_string(),
                        channel: "control".to_string(),
                        point: "start".to_string(),
                        value: json!(true),
                    },
                    description: Some("Start the diesel generator".to_string()),
                },
                RuleAction {
                    action_type: ActionType::Notify,
                    config: ActionConfig::Notify {
                        level: "info".to_string(),
                        message: "Diesel generator started due to low battery SOC".to_string(),
                        recipients: None,
                    },
                    description: Some("Send notification".to_string()),
                },
            ],
            enabled: true,
            priority: 1,
            cooldown_seconds: Some(300), // 5 minutes cooldown
        },
        Rule {
            id: "battery_high_stop_generator".to_string(),
            name: "Stop Generator on High Battery".to_string(),
            description: Some("Stop diesel generator when battery SOC reaches 80%".to_string()),
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "battery.soc".to_string(),
                    operator: ComparisonOperator::GreaterThanOrEqual,
                    value: json!(80.0),
                    description: Some("Battery SOC at or above 80%".to_string()),
                }],
            },
            actions: vec![
                RuleAction {
                    action_type: ActionType::DeviceControl,
                    config: ActionConfig::DeviceControl {
                        device_id: "generator_001".to_string(),
                        channel: "control".to_string(),
                        point: "stop".to_string(),
                        value: json!(true),
                    },
                    description: Some("Stop the diesel generator".to_string()),
                },
                RuleAction {
                    action_type: ActionType::Notify,
                    config: ActionConfig::Notify {
                        level: "info".to_string(),
                        message: "Diesel generator stopped - battery fully charged".to_string(),
                        recipients: None,
                    },
                    description: Some("Send notification".to_string()),
                },
            ],
            enabled: true,
            priority: 1,
            cooldown_seconds: Some(300), // 5 minutes cooldown
        },
        Rule {
            id: "voltage_monitoring".to_string(),
            name: "Voltage Monitoring Alert".to_string(),
            description: Some(
                "Alert when voltage drops below threshold or rises above limit".to_string(),
            ),
            conditions: ConditionGroup {
                operator: LogicOperator::Or,
                conditions: vec![
                    Condition {
                        source: "comsrv:1001:V".to_string(),
                        operator: ComparisonOperator::LessThan,
                        value: json!(220.0),
                        description: Some("Voltage below 220V".to_string()),
                    },
                    Condition {
                        source: "comsrv:1001:V".to_string(),
                        operator: ComparisonOperator::GreaterThan,
                        value: json!(250.0),
                        description: Some("Voltage above 250V".to_string()),
                    },
                ],
            },
            actions: vec![RuleAction {
                action_type: ActionType::Publish,
                config: ActionConfig::Publish {
                    channel: "ems:alerts".to_string(),
                    message: "Voltage out of normal range".to_string(),
                },
                description: Some("Send voltage alert".to_string()),
            }],
            enabled: true,
            priority: 2,
            cooldown_seconds: Some(60), // 1 minute cooldown
        },
    ];

    let response = ExampleRulesResponse { examples };
    (StatusCode::OK, Json(response))
}
