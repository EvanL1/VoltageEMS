use crate::error::{Result, RulesrvError};
use crate::redis::RedisStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Rule structure for practical automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Rule ID
    pub id: String,
    /// Rule name  
    pub name: String,
    /// Rule description
    pub description: Option<String>,
    /// Rule conditions (AND/OR logic)
    pub conditions: ConditionGroup,
    /// Actions to execute when conditions are met
    pub actions: Vec<RuleAction>,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Rule priority (higher values execute first)
    pub priority: i32,
    /// Cooldown period in seconds to prevent repeated execution
    pub cooldown_seconds: Option<u64>,
}

/// Condition group with AND/OR logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionGroup {
    /// Logic operator (AND/OR)
    pub operator: LogicOperator,
    /// Individual conditions
    pub conditions: Vec<Condition>,
}

/// Logic operators for combining conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicOperator {
    /// All conditions must be true
    #[serde(rename = "AND")]
    And,
    /// At least one condition must be true
    #[serde(rename = "OR")]
    Or,
}

/// Individual condition for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Data source key (e.g., "battery.soc", "comsrv:1001:V")
    pub source: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Value to compare against
    pub value: Value,
    /// Optional description
    pub description: Option<String>,
}

/// Comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Equal to
    #[serde(rename = "==")]
    Equals,
    /// Not equal to
    #[serde(rename = "!=")]
    NotEquals,
    /// Greater than
    #[serde(rename = ">")]
    GreaterThan,
    /// Greater than or equal to
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
    /// Less than
    #[serde(rename = "<")]
    LessThan,
    /// Less than or equal to
    #[serde(rename = "<=")]
    LessThanOrEqual,
    /// Contains (for string values)
    #[serde(rename = "contains")]
    Contains,
}

/// Action to execute when rule conditions are met
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    /// Action type
    pub action_type: ActionType,
    /// Action configuration
    pub config: ActionConfig,
    /// Optional description
    pub description: Option<String>,
}

/// Types of actions that can be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    /// Control a device
    #[serde(rename = "device_control")]
    DeviceControl,
    /// Publish a message
    #[serde(rename = "publish")]
    Publish,
    /// Set a value in Redis
    #[serde(rename = "set_value")]
    SetValue,
    /// Send notification/alarm
    #[serde(rename = "notify")]
    Notify,
}

/// Action configuration based on action type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionConfig {
    /// Device control configuration
    DeviceControl {
        device_id: String,
        channel: String,
        point: String,
        value: Value,
    },
    /// Publish configuration
    Publish { channel: String, message: String },
    /// Set value configuration
    SetValue {
        key: String,
        value: Value,
        ttl: Option<u64>,
    },
    /// Notification configuration
    Notify {
        level: String,
        message: String,
        recipients: Option<Vec<String>>,
    },
}

/// Rule execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionResult {
    pub rule_id: String,
    pub execution_id: String,
    pub timestamp: String,
    pub conditions_met: bool,
    pub actions_executed: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u128,
}

/// Rule engine for practical automation
pub struct RuleEngine {
    store: Arc<RedisStore>,
    /// Last execution timestamps for cooldown management
    last_execution: HashMap<String, std::time::Instant>,
}

impl RuleEngine {
    /// Create a new rule engine
    pub fn new(store: Arc<RedisStore>) -> Self {
        Self {
            store,
            last_execution: HashMap::new(),
        }
    }

    /// Execute a rule by ID
    pub async fn execute_rule(&mut self, rule_id: &str) -> Result<RuleExecutionResult> {
        let start_time = std::time::Instant::now();
        let execution_id = Uuid::new_v4().to_string();

        debug!("Executing rule: {}", rule_id);

        // Load rule from Redis
        let rule = self.load_rule(rule_id).await?;

        // Check if rule is enabled
        if !rule.enabled {
            return Ok(RuleExecutionResult {
                rule_id: rule_id.to_string(),
                execution_id,
                timestamp: chrono::Utc::now().to_rfc3339(),
                conditions_met: false,
                actions_executed: vec![],
                success: false,
                error: Some("Rule is disabled".to_string()),
                duration_ms: start_time.elapsed().as_millis(),
            });
        }

        // Check cooldown
        if let Some(cooldown) = rule.cooldown_seconds {
            if let Some(last_exec) = self.last_execution.get(rule_id) {
                let elapsed = last_exec.elapsed().as_secs();
                if elapsed < cooldown {
                    debug!(
                        "Rule {} still in cooldown, {} seconds remaining",
                        rule_id,
                        cooldown - elapsed
                    );
                    return Ok(RuleExecutionResult {
                        rule_id: rule_id.to_string(),
                        execution_id,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        conditions_met: false,
                        actions_executed: vec![],
                        success: true,
                        error: Some(format!(
                            "Rule in cooldown, {} seconds remaining",
                            cooldown - elapsed
                        )),
                        duration_ms: start_time.elapsed().as_millis(),
                    });
                }
            }
        }

        // Evaluate conditions
        let conditions_met = self.evaluate_conditions(&rule.conditions).await?;

        let mut actions_executed = vec![];
        let mut execution_error = None;

        if conditions_met {
            info!(
                "Rule '{}' conditions met, executing {} actions",
                rule.name,
                rule.actions.len()
            );

            // Execute actions
            for (idx, action) in rule.actions.iter().enumerate() {
                match self.execute_action(action).await {
                    Ok(action_result) => {
                        actions_executed.push(format!("action_{}: {}", idx, action_result));
                        debug!("Action {} executed successfully", idx);
                    }
                    Err(e) => {
                        let error_msg = format!("Action {} failed: {}", idx, e);
                        error!("{}", error_msg);
                        execution_error = Some(error_msg);
                        break; // Stop on first action failure
                    }
                }
            }

            // Update last execution time
            self.last_execution
                .insert(rule_id.to_string(), std::time::Instant::now());
        } else {
            debug!("Rule '{}' conditions not met", rule.name);
        }

        let result = RuleExecutionResult {
            rule_id: rule_id.to_string(),
            execution_id: execution_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            conditions_met,
            actions_executed,
            success: execution_error.is_none(),
            error: execution_error,
            duration_ms: start_time.elapsed().as_millis(),
        };

        // Store execution result in Redis
        self.store_execution_result(&result).await?;

        info!(
            "Rule '{}' execution completed in {}ms, conditions_met: {}",
            rule.name, result.duration_ms, conditions_met
        );

        Ok(result)
    }

    /// Load rule from Redis store
    async fn load_rule(&self, rule_id: &str) -> Result<Rule> {
        let rule_key = format!("rulesrv:rule:{}", rule_id);

        match self.store.get_string(&rule_key).await {
            Ok(Some(rule_json)) => serde_json::from_str(&rule_json).map_err(|e| {
                RulesrvError::RuleParsingError(format!("Failed to parse rule {}: {}", rule_id, e))
            }),
            Ok(None) => Err(RulesrvError::RuleNotFound(format!(
                "Rule {} not found",
                rule_id
            ))),
            Err(e) => Err(RulesrvError::RedisError(format!(
                "Failed to load rule {}: {}",
                rule_id, e
            ))),
        }
    }

    /// Evaluate condition group (AND/OR logic)
    async fn evaluate_conditions(&self, condition_group: &ConditionGroup) -> Result<bool> {
        let mut results = vec![];

        for condition in &condition_group.conditions {
            let result = self.evaluate_condition(condition).await?;
            results.push(result);

            // Short-circuit evaluation for performance
            match condition_group.operator {
                LogicOperator::And if !result => return Ok(false),
                LogicOperator::Or if result => return Ok(true),
                _ => {}
            }
        }

        let final_result = match condition_group.operator {
            LogicOperator::And => results.iter().all(|&r| r),
            LogicOperator::Or => results.iter().any(|&r| r),
        };

        debug!("Condition group evaluated to: {}", final_result);
        Ok(final_result)
    }

    /// Evaluate individual condition
    async fn evaluate_condition(&self, condition: &Condition) -> Result<bool> {
        // Get value from Redis
        let source_value = match self.get_source_value(&condition.source).await {
            Ok(Some(value)) => value,
            Ok(None) => {
                debug!(
                    "Source '{}' not found, condition evaluates to false",
                    condition.source
                );
                return Ok(false);
            }
            Err(e) => {
                warn!(
                    "Failed to get source value for '{}': {}",
                    condition.source, e
                );
                return Ok(false);
            }
        };

        let result = self.compare_values(&source_value, &condition.operator, &condition.value)?;

        debug!(
            "Condition '{}' {} '{}' = {}",
            source_value,
            self.operator_to_string(&condition.operator),
            condition.value,
            result
        );

        Ok(result)
    }

    /// Get value from a data source
    async fn get_source_value(&self, source: &str) -> Result<Option<Value>> {
        // Handle different source formats:
        // - Direct Redis key: "comsrv:1001:V"
        // - Hash field: "comsrv:1001:T.10001"
        // - Nested JSON path: "battery.soc"

        if source.contains('.') && !source.starts_with("comsrv:") {
            // Handle hash field format like "comsrv:1001:T.10001"
            let parts: Vec<&str> = source.splitn(2, '.').collect();
            if parts.len() == 2 {
                let hash_key = parts[0];
                let field = parts[1];

                match self.store.get_hash_field(hash_key, field).await {
                    Ok(Some(value_str)) => {
                        // Try to parse as number first, then as JSON, finally as string
                        if let Ok(num) = value_str.parse::<f64>() {
                            Ok(Some(json!(num)))
                        } else if let Ok(json_val) = serde_json::from_str(&value_str) {
                            Ok(Some(json_val))
                        } else {
                            Ok(Some(json!(value_str)))
                        }
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(RulesrvError::RedisError(e.to_string())),
                }
            } else {
                // Try as direct Redis key
                self.get_redis_key_value(source).await
            }
        } else {
            // Direct Redis key
            self.get_redis_key_value(source).await
        }
    }

    /// Get value from direct Redis key
    async fn get_redis_key_value(&self, key: &str) -> Result<Option<Value>> {
        match self.store.get_string(key).await {
            Ok(Some(value_str)) => {
                // Handle comsrv format "value:timestamp"
                if key.starts_with("comsrv:") {
                    if let Some(val_part) = value_str.split(':').next() {
                        if let Ok(num) = val_part.parse::<f64>() {
                            return Ok(Some(json!(num)));
                        }
                    }
                }

                // Try to parse as number, then JSON, then string
                if let Ok(num) = value_str.parse::<f64>() {
                    Ok(Some(json!(num)))
                } else if let Ok(json_val) = serde_json::from_str(&value_str) {
                    Ok(Some(json_val))
                } else {
                    Ok(Some(json!(value_str)))
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(RulesrvError::RedisError(e.to_string())),
        }
    }

    /// Compare two values using the specified operator
    fn compare_values(
        &self,
        left: &Value,
        operator: &ComparisonOperator,
        right: &Value,
    ) -> Result<bool> {
        match operator {
            ComparisonOperator::Equals => Ok(left == right),
            ComparisonOperator::NotEquals => Ok(left != right),
            ComparisonOperator::Contains => {
                let left_string = left
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| left.to_string());
                let right_string = right
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| right.to_string());
                Ok(left_string.contains(&right_string))
            }
            _ => {
                // Numeric comparisons
                let left_num = self.extract_number(left)?;
                let right_num = self.extract_number(right)?;

                match operator {
                    ComparisonOperator::GreaterThan => Ok(left_num > right_num),
                    ComparisonOperator::GreaterThanOrEqual => Ok(left_num >= right_num),
                    ComparisonOperator::LessThan => Ok(left_num < right_num),
                    ComparisonOperator::LessThanOrEqual => Ok(left_num <= right_num),
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Extract numeric value from JSON Value
    fn extract_number(&self, value: &Value) -> Result<f64> {
        match value {
            Value::Number(n) => n
                .as_f64()
                .ok_or_else(|| RulesrvError::RuleError("Invalid number".to_string())),
            Value::String(s) => s
                .parse::<f64>()
                .map_err(|_| RulesrvError::RuleError(format!("Cannot parse '{}' as number", s))),
            _ => Err(RulesrvError::RuleError(format!(
                "Value {:?} is not a number",
                value
            ))),
        }
    }

    /// Convert operator to string for logging
    fn operator_to_string(&self, operator: &ComparisonOperator) -> &'static str {
        match operator {
            ComparisonOperator::Equals => "==",
            ComparisonOperator::NotEquals => "!=",
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::GreaterThanOrEqual => ">=",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::LessThanOrEqual => "<=",
            ComparisonOperator::Contains => "contains",
        }
    }

    /// Execute an action
    async fn execute_action(&self, action: &RuleAction) -> Result<String> {
        match &action.action_type {
            ActionType::DeviceControl => {
                if let ActionConfig::DeviceControl {
                    device_id,
                    channel,
                    point,
                    value,
                } = &action.config
                {
                    self.execute_device_control(device_id, channel, point, value)
                        .await
                } else {
                    Err(RulesrvError::ActionExecutionError(
                        "Invalid device control configuration".to_string(),
                    ))
                }
            }
            ActionType::Publish => {
                if let ActionConfig::Publish { channel, message } = &action.config {
                    self.execute_publish(channel, message).await
                } else {
                    Err(RulesrvError::ActionExecutionError(
                        "Invalid publish configuration".to_string(),
                    ))
                }
            }
            ActionType::SetValue => {
                if let ActionConfig::SetValue { key, value, ttl } = &action.config {
                    self.execute_set_value(key, value, *ttl).await
                } else {
                    Err(RulesrvError::ActionExecutionError(
                        "Invalid set value configuration".to_string(),
                    ))
                }
            }
            ActionType::Notify => {
                if let ActionConfig::Notify {
                    level,
                    message,
                    recipients: _,
                } = &action.config
                {
                    self.execute_notify(level, message).await
                } else {
                    Err(RulesrvError::ActionExecutionError(
                        "Invalid notify configuration".to_string(),
                    ))
                }
            }
        }
    }

    /// Execute device control action
    async fn execute_device_control(
        &self,
        device_id: &str,
        channel: &str,
        point: &str,
        value: &Value,
    ) -> Result<String> {
        let cmd_id = format!("cmd_{}", Uuid::new_v4().simple());

        let command = json!({
            "id": cmd_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "target": {
                "device_id": device_id,
                "channel": channel
            },
            "operation": "write",
            "parameters": {
                "point": point,
                "value": value
            },
            "status": "pending",
            "source": "rule_engine",
            "timeout": 30,
            "priority": 1
        });

        let cmd_key = format!("ems:control:cmd:{}", cmd_id);
        self.store
            .set_string(&cmd_key, &command.to_string())
            .await
            .map_err(|e| {
                RulesrvError::ActionExecutionError(format!(
                    "Failed to queue device control command: {}",
                    e
                ))
            })?;

        // Publish to control queue (if implemented)
        let queue_key = "ems:control:queue";
        if let Err(e) = self.store.publish(queue_key, &cmd_id).await {
            warn!("Failed to publish to control queue: {}", e);
        }

        Ok(format!("Device control command queued: {}", cmd_id))
    }

    /// Execute publish action
    async fn execute_publish(&self, channel: &str, message: &str) -> Result<String> {
        self.store.publish(channel, message).await.map_err(|e| {
            RulesrvError::ActionExecutionError(format!("Failed to publish message: {}", e))
        })?;

        Ok(format!("Published to channel: {}", channel))
    }

    /// Execute set value action
    async fn execute_set_value(
        &self,
        key: &str,
        value: &Value,
        ttl: Option<u64>,
    ) -> Result<String> {
        let value_str = serde_json::to_string(value).map_err(|e| {
            RulesrvError::ActionExecutionError(format!("Failed to serialize value: {}", e))
        })?;

        self.store.set_string(key, &value_str).await.map_err(|e| {
            RulesrvError::ActionExecutionError(format!("Failed to set value: {}", e))
        })?;

        // TODO: Implement TTL when RedisStore supports EXPIRE
        if ttl.is_some() {
            warn!("TTL not implemented in RedisStore");
        }

        Ok(format!("Set value: {} = {}", key, value_str))
    }

    /// Execute notify action
    async fn execute_notify(&self, level: &str, message: &str) -> Result<String> {
        let notification = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": level,
            "message": message,
            "source": "rule_engine"
        });

        let notification_str = notification.to_string();

        // Publish to notifications channel
        let channel = "ems:notifications";
        self.store
            .publish(channel, &notification_str)
            .await
            .map_err(|e| {
                RulesrvError::ActionExecutionError(format!("Failed to publish notification: {}", e))
            })?;

        Ok(format!("Notification sent: {}", message))
    }

    /// Store execution result in Redis
    async fn store_execution_result(&self, result: &RuleExecutionResult) -> Result<()> {
        let result_key = format!("rulesrv:execution:{}", result.execution_id);
        let result_json = serde_json::to_string(result).map_err(|e| {
            RulesrvError::RuleError(format!("Failed to serialize execution result: {}", e))
        })?;

        self.store
            .set_string(&result_key, &result_json)
            .await
            .map_err(|e| {
                RulesrvError::RedisError(format!("Failed to store execution result: {}", e))
            })?;

        // Also update rule statistics
        let stats_key = format!("rulesrv:rule:{}:stats", result.rule_id);
        let stats = json!({
            "last_execution": result.timestamp,
            "last_result": result.success,
            "conditions_met": result.conditions_met
        });

        if let Err(e) = self.store.set_string(&stats_key, &stats.to_string()).await {
            warn!("Failed to update rule statistics: {}", e);
        }

        Ok(())
    }

    /// List all rules
    pub async fn list_rules(&self) -> Result<Vec<Rule>> {
        // Use Redis Functions to query rules efficiently
        match self
            .store
            .call_function("query_rules", &[], &[r#"{"enabled": true}"#])
            .await
        {
            Ok(result) => {
                let query_result: Value = serde_json::from_str(&result).map_err(|e| {
                    RulesrvError::RuleParsingError(format!("Failed to parse query result: {}", e))
                })?;

                if let Some(rules_array) = query_result.get("data").and_then(|d| d.as_array()) {
                    let mut rules = vec![];
                    for rule_value in rules_array {
                        match serde_json::from_value::<Rule>(rule_value.clone()) {
                            Ok(rule) => rules.push(rule),
                            Err(e) => warn!("Failed to parse rule: {}", e),
                        }
                    }
                    Ok(rules)
                } else {
                    Ok(vec![])
                }
            }
            Err(_) => {
                // Fallback to direct Redis query if Redis Functions not available
                warn!("Redis Functions not available, using fallback query");
                self.list_rules_fallback().await
            }
        }
    }

    /// Fallback method to list rules without Redis Functions
    async fn list_rules_fallback(&self) -> Result<Vec<Rule>> {
        // This is a simplified fallback - in production you might want to
        // implement pattern matching for rule keys
        warn!("Fallback rule listing not fully implemented");
        Ok(vec![])
    }

    /// Store a rule
    pub async fn store_rule(&self, rule: &Rule) -> Result<()> {
        let rule_key = format!("rulesrv:rule:{}", rule.id);
        let rule_json = serde_json::to_string(rule).map_err(|e| {
            RulesrvError::RuleParsingError(format!("Failed to serialize rule: {}", e))
        })?;

        self.store
            .set_string(&rule_key, &rule_json)
            .await
            .map_err(|e| RulesrvError::RedisError(format!("Failed to store rule: {}", e)))?;

        info!("Stored rule: {} ({})", rule.id, rule.name);
        Ok(())
    }
}
