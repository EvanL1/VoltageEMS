//! Rule Engine Core Module
//!
//! This module provides the core rule engine functionality for VoltageEMS.
//! It supports configuration-driven rules with conditions and actions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

// Re-export ComparisonOperator from common for use in other modules
pub use voltage_config::common::ComparisonOperator;

/// Rule definition with conditions and actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique rule identifier
    pub id: String,

    /// Human-readable rule name
    pub name: String,

    /// Rule category for grouping
    #[serde(default)]
    pub category: String,

    /// Rule description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Execution priority (lower number = higher priority)
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Whether the rule is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Trigger conditions for the rule
    #[serde(default)]
    pub triggers: Vec<Trigger>,

    /// Conditions that must be met for the rule to execute
    pub conditions: ConditionGroup,

    /// Actions to execute when conditions are met
    pub actions: Vec<Action>,

    /// Metadata for rule configuration
    #[serde(default)]
    pub metadata: RuleMetadata,
}

fn default_priority() -> i32 {
    100
}

fn default_enabled() -> bool {
    true
}

/// Rule trigger types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// Periodic interval trigger
    Interval {
        /// Interval in milliseconds
        interval: u64,
    },

    /// Event-based trigger
    Event {
        /// Event name to listen for
        event: String,
    },

    /// Data change trigger
    DataChange {
        /// Data field to monitor
        field: String,

        /// Optional threshold for change detection
        #[serde(skip_serializing_if = "Option::is_none")]
        threshold: Option<f64>,
    },

    /// Schedule-based trigger (cron expression)
    Schedule {
        /// Cron expression
        cron: String,
    },
}

/// Condition group with logical operators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConditionGroup {
    /// Single condition
    Single(Condition),

    /// Group of conditions with logical operator
    Group {
        /// Logical operator: AND, OR
        logic: LogicalOperator,

        /// List of conditions or nested groups
        rules: Vec<ConditionGroup>,
    },
}

/// Logical operators for combining conditions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogicalOperator {
    And,
    Or,
}

/// Individual condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Field to evaluate - supports both legacy and modsrv syntax
    /// Legacy: "energy:pv_power"
    /// Modsrv: "pv_inverter_01.M.3" (instance.type.point)
    /// Aggregate: "SUM(pv_inverter_*.M.3)"
    pub field: String,

    /// Comparison operator
    pub operator: ComparisonOperator,

    /// Static value to compare against
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,

    /// Reference to another field for dynamic comparison
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_ref: Option<String>,
}

// Note: The ComparisonOperator from common uses different serde names than before:
// - "eq" instead of "=="
// - "ne" instead of "!="
// - "gt" instead of ">"
// - "gte" instead of ">="
// - "lt" instead of "<"
// - "lte" instead of "<="
// The "in", "not_in", "contains", and "matches" remain the same.

/// Actions to execute when rule conditions are met
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Set a system mode
    SetMode {
        /// Mode parameters
        params: HashMap<String, serde_json::Value>,
    },

    /// Send control command to a device/channel
    SendControl {
        /// Control parameters
        params: HashMap<String, serde_json::Value>,
    },

    /// Trigger an alarm
    TriggerAlarm {
        /// Alarm parameters
        params: HashMap<String, serde_json::Value>,
    },

    /// Publish message to a channel
    Publish {
        /// Publish parameters
        params: HashMap<String, serde_json::Value>,
    },

    /// Set a value in Redis
    SetValue {
        /// Target key
        target: String,

        /// Value to set
        value: serde_json::Value,
    },

    /// Invoke a model calculation
    InvokeModel {
        /// Model ID
        model_id: String,

        /// Model parameters
        params: HashMap<String, serde_json::Value>,
    },

    /// Execute a Lua script
    ExecuteScript {
        /// Script name
        script: String,

        /// Script arguments
        args: Vec<serde_json::Value>,
    },

    /// Send HTTP request
    HttpRequest {
        /// Request URL
        url: String,

        /// HTTP method
        method: String,

        /// Request headers
        #[serde(default)]
        headers: HashMap<String, String>,

        /// Request body
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<serde_json::Value>,
    },

    /// Send command to modsrv instance
    ModsrvCommand {
        /// Target instance name
        instance_name: String,

        /// Point type ("M" for measurement, "A" for action)
        point_type: String,

        /// Point ID
        point_id: u32,

        /// Value to set
        value: serde_json::Value,
    },

    /// Batch modsrv commands
    ModsrvBatch {
        /// Multiple commands to execute
        commands: Vec<ModsrvCommandData>,
    },
}

/// Modsrv command data for batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModsrvCommandData {
    /// Target instance name
    pub instance_name: String,

    /// Point type ("M" for measurement, "A" for action)
    pub point_type: String,

    /// Point ID
    pub point_id: u32,

    /// Value to set
    pub value: serde_json::Value,
}

/// Rule metadata for execution control
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleMetadata {
    /// Cooldown period in milliseconds to prevent rapid re-triggering
    #[serde(default = "default_cooldown")]
    pub cooldown: u64,

    /// Maximum retry attempts for failed actions
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Timeout for rule execution in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Tags for categorization and filtering
    #[serde(default)]
    pub tags: Vec<String>,

    /// Additional custom metadata
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

fn default_cooldown() -> u64 {
    5000 // 5 seconds
}

fn default_max_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    10000 // 10 seconds
}

/// Rule group for organizing related rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroup {
    /// Group identifier
    pub id: String,

    /// Group name
    pub name: String,

    /// Group description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Rule IDs in this group
    pub rules: Vec<String>,

    /// Execution order strategy
    #[serde(default)]
    pub execution_order: ExecutionOrder,
}

/// Execution order strategies for rule groups
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOrder {
    /// Execute by priority
    #[default]
    Priority,

    /// Execute sequentially in order
    Sequential,

    /// Execute in parallel
    Parallel,
}

/// Rule configuration containing all rules and settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    /// Configuration version
    #[serde(default = "default_version")]
    pub version: String,

    /// Namespace for rule isolation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// List of rules
    pub rules: Vec<Rule>,

    /// Rule groups
    #[serde(default)]
    pub rule_groups: Vec<RuleGroup>,

    /// Global settings
    #[serde(default)]
    pub settings: RuleSettings,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Global rule engine settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleSettings {
    /// Engine configuration
    #[serde(default)]
    pub engine: EngineSettings,

    /// Persistence configuration
    #[serde(default)]
    pub persistence: PersistenceSettings,

    /// Monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringSettings,
}

/// Engine execution settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    /// Execution mode: sync or async
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,

    /// Batch size for rule processing
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Number of worker threads
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            execution_mode: default_execution_mode(),
            batch_size: default_batch_size(),
            worker_threads: default_worker_threads(),
        }
    }
}

fn default_execution_mode() -> String {
    "async".to_string()
}

fn default_batch_size() -> usize {
    100
}

fn default_worker_threads() -> usize {
    4
}

/// Persistence settings for rule execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceSettings {
    /// Whether to save execution history
    #[serde(default = "default_save_history")]
    pub save_history: bool,

    /// History retention in days
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            save_history: default_save_history(),
            retention_days: default_retention_days(),
        }
    }
}

fn default_save_history() -> bool {
    true
}

fn default_retention_days() -> u32 {
    30
}

/// Monitoring settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    /// Enable metrics collection
    #[serde(default)]
    pub metrics_enabled: bool,

    /// Enable tracing
    #[serde(default)]
    pub trace_enabled: bool,
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            trace_enabled: false,
        }
    }
}

/// Historical data for change detection
#[derive(Debug, Clone)]
pub struct DataHistory {
    /// Previous value
    pub value: f64,

    /// Timestamp of the previous value
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Rule execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Current timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Execution ID for tracing
    #[allow(dead_code)]
    pub execution_id: String,

    /// Data values for condition evaluation
    pub data: HashMap<String, serde_json::Value>,

    /// Previous execution results
    pub history: Vec<ExecutionResult>,

    /// Historical data for change detection (field -> last value and timestamp)
    pub data_history: HashMap<String, DataHistory>,
}

/// Result of rule execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Rule ID
    pub rule_id: String,

    /// Execution timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Whether conditions were met
    pub conditions_met: bool,

    /// Actions executed
    pub actions_executed: Vec<ActionResult>,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Error if execution failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of action execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Action type
    pub action_type: String,

    /// Success status
    pub success: bool,

    /// Result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Rule {
    /// Check if rule should be triggered based on triggers
    pub fn should_trigger(&self, context: &ExecutionContext) -> bool {
        if self.triggers.is_empty() {
            // No triggers means always evaluate
            return true;
        }

        // Check if any trigger condition is met
        self.triggers.iter().any(|trigger| {
            match trigger {
                Trigger::Interval { .. } => {
                    // Handled by scheduler
                    true
                },
                Trigger::Event { event } => {
                    // Check if event is in context
                    context.data.contains_key(&format!("event:{}", event))
                },
                Trigger::DataChange { field, threshold } => {
                    // Check if field has changed beyond threshold
                    if let Some(current_value_json) = context.data.get(field) {
                        // Try to convert current value to f64
                        let current_value = match current_value_json {
                            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                            serde_json::Value::String(s) => {
                                match s.parse::<f64>() {
                                    Ok(v) => v,
                                    Err(_) => {
                                        debug!(
                                            "DataChange trigger: field '{}' contains non-numeric string '{}', skipping detection",
                                            field, s
                                        );
                                        return false;
                                    }
                                }
                            }
                            _ => {
                                debug!(
                                    "DataChange trigger: field '{}' has unsupported type for numeric comparison, skipping detection",
                                    field
                                );
                                return false;
                            }
                        };

                        if let Some(thresh) = threshold {
                            // Check if we have previous value
                            if let Some(history) = context.data_history.get(field) {
                                let prev_value = history.value;
                                let change = (current_value - prev_value).abs();

                                // Support two threshold modes:
                                // - Positive threshold: absolute value change
                                // - Negative threshold: percentage change (e.g., -10.0 means 10% change)
                                let threshold_met = if *thresh < 0.0 {
                                    // Percentage threshold
                                    let percentage_threshold = -thresh;
                                    if prev_value.abs() < 1e-10 {
                                        // Avoid division by zero
                                        change > 1e-10
                                    } else {
                                        let percentage_change = (change / prev_value.abs()) * 100.0;
                                        percentage_change >= percentage_threshold
                                    }
                                } else {
                                    // Absolute value threshold
                                    change >= *thresh
                                };

                                debug!(
                                    "DataChange check: field={}, current={}, prev={}, change={}, threshold={}, met={}",
                                    field, current_value, prev_value, change, thresh, threshold_met
                                );

                                threshold_met
                            } else {
                                // No previous value, first time seeing this field
                                debug!("DataChange: No previous value for field {}, triggering", field);
                                true
                            }
                        } else {
                            // No threshold specified, any change triggers
                            if let Some(history) = context.data_history.get(field) {
                                let changed = (current_value - history.value).abs() > 1e-10;
                                debug!(
                                    "DataChange (any change): field={}, current={}, prev={}, changed={}",
                                    field, current_value, history.value, changed
                                );
                                changed
                            } else {
                                // First value, always trigger
                                debug!("DataChange (any change): No previous value for field {}, triggering", field);
                                true
                            }
                        }
                    } else {
                        debug!("DataChange: Field {} not found in context data", field);
                        false
                    }
                },
                Trigger::Schedule { .. } => {
                    // Handled by scheduler
                    true
                },
            }
        })
    }

    /// Check if rule is in cooldown period
    pub fn is_in_cooldown(&self, context: &ExecutionContext) -> bool {
        if self.metadata.cooldown == 0 {
            return false;
        }

        // Check last execution time from history
        for result in &context.history {
            if result.rule_id == self.id {
                let elapsed = context.timestamp.signed_duration_since(result.timestamp);
                let cooldown_ms = chrono::Duration::milliseconds(self.metadata.cooldown as i64);

                if elapsed < cooldown_ms {
                    debug!(
                        "Rule {} is in cooldown for {} more ms",
                        self.id,
                        (cooldown_ms - elapsed).num_milliseconds()
                    );
                    return true;
                }
                break; // Only check the most recent execution
            }
        }

        false
    }
}

/// Modsrv field reference parser
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModsrvField {
    /// Instance name (e.g., "pv_inverter_01")
    pub instance_name: String,

    /// Point type ("M" for measurement, "A" for action)
    pub point_type: String,

    /// Point ID
    pub point_id: u32,
}

impl ModsrvField {
    /// Parse a modsrv field reference string
    /// Format: "instance_name.type.point_id" (e.g., "pv_inverter_01.M.3")
    pub fn parse(field: &str) -> Option<Self> {
        let parts: Vec<&str> = field.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let instance_name = parts[0].to_string();
        let point_type = parts[1].to_string();
        let point_id = parts[2].parse().ok()?;

        // Validate point type
        if point_type != "M" && point_type != "A" {
            return None;
        }

        Some(ModsrvField {
            instance_name,
            point_type,
            point_id,
        })
    }

    /// Convert to Redis key format
    pub fn to_redis_key(&self) -> String {
        format!(
            "modsrv:{}:{}:{}",
            self.instance_name, self.point_type, self.point_id
        )
    }
}

/// Aggregate function types for modsrv data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateFunction {
    Sum,
    Average,
    Max,
    Min,
    Count,
}

impl AggregateFunction {
    /// Parse from string (e.g., "SUM", "AVG", "MAX", "MIN", "COUNT")
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "SUM" => Some(AggregateFunction::Sum),
            "AVG" | "AVERAGE" => Some(AggregateFunction::Average),
            "MAX" => Some(AggregateFunction::Max),
            "MIN" => Some(AggregateFunction::Min),
            "COUNT" => Some(AggregateFunction::Count),
            _ => None,
        }
    }
}

/// Parsed aggregate field expression
#[derive(Debug, Clone)]
pub struct AggregateField {
    /// Aggregate function to apply
    pub function: AggregateFunction,

    /// Instance pattern (supports wildcards)
    pub instance_pattern: String,

    /// Point type
    pub point_type: String,

    /// Point ID
    pub point_id: u32,
}

impl AggregateField {
    /// Parse aggregate field expression
    /// Format: "FUNC(instance_pattern.type.point_id)"
    /// Example: "SUM(pv_inverter_*.M.3)"
    pub fn parse(field: &str) -> Option<Self> {
        // Check for function call syntax
        let open_paren = field.find('(')?;
        let close_paren = field.rfind(')')?;

        if close_paren <= open_paren {
            return None;
        }

        let func_name = &field[..open_paren];
        let field_expr = &field[open_paren + 1..close_paren];

        let function = AggregateFunction::parse(func_name)?;

        // Parse the field expression
        let parts: Vec<&str> = field_expr.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        Some(AggregateField {
            function,
            instance_pattern: parts[0].to_string(),
            point_type: parts[1].to_string(),
            point_id: parts[2].parse().ok()?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_rule_deserialization() {
        let json = r#"{
            "id": "test_rule",
            "name": "Test Rule",
            "priority": 10,
            "enabled": true,
            "conditions": {
                "logic": "AND",
                "rules": [
                    {
                        "field": "temperature",
                        "operator": "gt",
                        "value": 80
                    },
                    {
                        "field": "pressure",
                        "operator": "lt",
                        "value": 100
                    }
                ]
            },
            "actions": [
                {
                    "type": "set_mode",
                    "params": {
                        "mode": "cooling"
                    }
                }
            ]
        }"#;

        let rule: Rule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.id, "test_rule");
        assert_eq!(rule.priority, 10);
        assert!(rule.enabled);
    }

    #[test]
    fn test_condition_operators() {
        // Test with new serde names from common.rs
        let op: ComparisonOperator = serde_json::from_str(r#""gte""#).unwrap();
        assert_eq!(op, ComparisonOperator::GreaterThanOrEqual);

        let op: ComparisonOperator = serde_json::from_str(r#""in""#).unwrap();
        assert_eq!(op, ComparisonOperator::InRange);

        // Test other operators
        let op: ComparisonOperator = serde_json::from_str(r#""eq""#).unwrap();
        assert_eq!(op, ComparisonOperator::Equal);

        let op: ComparisonOperator = serde_json::from_str(r#""contains""#).unwrap();
        assert_eq!(op, ComparisonOperator::Contains);
    }

    #[test]
    fn test_modsrv_field_parsing() {
        // Valid modsrv field
        let field = ModsrvField::parse("pv_inverter_01.M.3").unwrap();
        assert_eq!(field.instance_name, "pv_inverter_01");
        assert_eq!(field.point_type, "M");
        assert_eq!(field.point_id, 3);
        assert_eq!(field.to_redis_key(), "modsrv:pv_inverter_01:M:3");

        // Action point
        let field = ModsrvField::parse("pcs_01.A.1").unwrap();
        assert_eq!(field.instance_name, "pcs_01");
        assert_eq!(field.point_type, "A");
        assert_eq!(field.point_id, 1);

        // Invalid formats
        assert!(ModsrvField::parse("invalid").is_none());
        assert!(ModsrvField::parse("too.many.parts.here").is_none());
        assert!(ModsrvField::parse("instance.X.1").is_none()); // Invalid type
        assert!(ModsrvField::parse("instance.M.abc").is_none()); // Invalid point ID
    }

    #[test]
    fn test_aggregate_field_parsing() {
        // SUM function
        let field = AggregateField::parse("SUM(pv_inverter_*.M.3)").unwrap();
        assert!(matches!(field.function, AggregateFunction::Sum));
        assert_eq!(field.instance_pattern, "pv_inverter_*");
        assert_eq!(field.point_type, "M");
        assert_eq!(field.point_id, 3);

        // AVG function
        let field = AggregateField::parse("AVG(pcs_*.M.7)").unwrap();
        assert!(matches!(field.function, AggregateFunction::Average));
        assert_eq!(field.instance_pattern, "pcs_*");

        // Invalid formats
        assert!(AggregateField::parse("SUM").is_none()); // No parentheses
        assert!(AggregateField::parse("INVALID(test.M.1)").is_none()); // Invalid function
        assert!(AggregateField::parse("SUM(test)").is_none()); // Invalid field format
    }

    #[test]
    fn test_datachange_trigger_string_handling() {
        use std::collections::HashMap;

        // Create a rule with DataChange trigger
        let rule = Rule {
            id: "test_rule".to_string(),
            name: "Test DataChange".to_string(),
            category: String::new(),
            description: None,
            enabled: true,
            priority: 5,
            triggers: vec![Trigger::DataChange {
                field: "test_field".to_string(),
                threshold: Some(5.0),
            }],
            conditions: ConditionGroup::Single(Condition {
                field: "always_true".to_string(),
                operator: ComparisonOperator::Equal,
                value: Some(serde_json::json!(true)),
                value_ref: None,
            }),
            actions: vec![],
            metadata: RuleMetadata {
                cooldown: 0,
                max_retries: 3,
                timeout: 10000,
                tags: vec![],
                custom: HashMap::new(),
            },
        };

        // Test 1: Numeric string should work
        let mut data = HashMap::new();
        data.insert("test_field".to_string(), serde_json::json!("123.45"));
        let context = ExecutionContext {
            timestamp: chrono::Utc::now(),
            execution_id: "test_exec_1".to_string(),
            data: data.clone(),
            history: vec![],
            data_history: HashMap::new(),
        };
        assert!(rule.should_trigger(&context)); // Should trigger (first observation)

        // Test 2: Non-numeric string should be rejected
        data.insert("test_field".to_string(), serde_json::json!("RUNNING"));
        let context = ExecutionContext {
            timestamp: chrono::Utc::now(),
            execution_id: "test_exec_2".to_string(),
            data: data.clone(),
            history: vec![],
            data_history: HashMap::new(),
        };
        assert!(!rule.should_trigger(&context)); // Should NOT trigger (non-numeric string)

        // Test 3: Numeric value should work
        data.insert("test_field".to_string(), serde_json::json!(42.0));
        let context = ExecutionContext {
            timestamp: chrono::Utc::now(),
            execution_id: "test_exec_3".to_string(),
            data,
            history: vec![],
            data_history: HashMap::new(),
        };
        assert!(rule.should_trigger(&context)); // Should trigger (valid numeric value)
    }
}
