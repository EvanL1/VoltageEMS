//! Rulesrv service configuration structures
//!
//! This module defines:
//! - Service configuration (RulesrvConfig)
//! - Rule chain structures for Vue Flow execution (RuleChain, FlowNode, etc.)
//! - SQLite table schemas

use crate::common::{ApiConfig, BaseServiceConfig, RedisConfig};
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema")]
use schemars::JsonSchema;

#[cfg(feature = "schema-macro")]
use voltage_schema_macro::Schema;

/// Default API configuration for rulesrv (port 6003)
fn default_rulesrv_api() -> ApiConfig {
    ApiConfig {
        host: "0.0.0.0".to_string(),
        port: 6003,
    }
}

/// Rulesrv service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RulesrvConfig {
    /// Base service configuration
    #[serde(flatten)]
    pub service: BaseServiceConfig,

    /// API configuration (has default value)
    #[serde(default = "default_rulesrv_api")]
    pub api: ApiConfig,

    /// Redis configuration (simplified)
    #[serde(default)]
    pub redis: RedisConfig,

    /// Execution configuration
    #[serde(default)]
    pub execution: ExecutionConfig,
}

// ============================================================================
// Database Schema Definitions
// ============================================================================

/// Service configuration table record
/// Maps to RulesrvConfig for service-level settings
/// Supports both global and service-specific configuration with composite primary key
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "service_config"))]
#[allow(dead_code)]
struct ServiceConfigRecord {
    #[cfg_attr(feature = "schema-macro", column(not_null, primary_key))]
    service_name: String,

    #[cfg_attr(feature = "schema-macro", column(not_null, primary_key))]
    key: String,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    value: String,

    #[cfg_attr(feature = "schema-macro", column(default = "string"))]
    r#type: String,

    description: Option<String>,

    #[cfg_attr(feature = "schema-macro", column(default = "CURRENT_TIMESTAMP"))]
    updated_at: String, // TIMESTAMP
}

/// Sync metadata table record
/// Tracks configuration synchronization status
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "sync_metadata"))]
#[allow(dead_code)]
struct SyncMetadataRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key))]
    service: String,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    last_sync: String, // TIMESTAMP

    version: Option<String>,
}

// Generate table SQL from Schema structs
#[cfg(feature = "schema-macro")]
pub const SERVICE_CONFIG_TABLE: &str = ServiceConfigRecord::CREATE_TABLE_SQL;
#[cfg(feature = "schema-macro")]
pub const SYNC_METADATA_TABLE: &str = SyncMetadataRecord::CREATE_TABLE_SQL;

// Fallback for non-schema-macro builds
#[cfg(not(feature = "schema-macro"))]
pub const SERVICE_CONFIG_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS service_config (
        service_name TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        type TEXT DEFAULT 'string',
        description TEXT,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (service_name, key)
    )
"#;

#[cfg(not(feature = "schema-macro"))]
pub const SYNC_METADATA_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS sync_metadata (
        service TEXT PRIMARY KEY,
        last_sync TIMESTAMP NOT NULL,
        version TEXT
    )
"#;

/// Default port for rulesrv service
pub const DEFAULT_PORT: u16 = 6003;

/// Rule execution configuration (reserved for future use)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ExecutionConfig {}

/// Rule core fields (shared between Config and API responses)
/// These fields represent the essential rule identity and state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleCore {
    /// Rule ID
    pub id: String,

    /// Rule name
    pub name: String,

    /// Rule description
    pub description: Option<String>,

    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Priority (higher number = higher priority)
    #[serde(default)]
    pub priority: u32,
}

/// Individual rule configuration for vue-flow/node-red
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleConfig {
    /// Core rule fields
    #[serde(flatten)]
    pub core: RuleCore,

    /// Complete flow graph JSON (nodes, edges, viewport, etc.)
    pub flow_json: serde_json::Value,
}

impl RuleConfig {
    /// Convenient accessor for rule ID
    pub fn id(&self) -> &str {
        &self.core.id
    }

    /// Convenient accessor for rule name
    pub fn name(&self) -> &str {
        &self.core.name
    }

    /// Convenient accessor for enabled status
    pub fn is_enabled(&self) -> bool {
        self.core.enabled
    }

    /// Convenient accessor for priority
    pub fn priority(&self) -> u32 {
        self.core.priority
    }
}

/// Rules table record
/// Stores rule definitions as JSON for vue-flow/node-red
/// Each rule is a complete flow graph (nodes, edges, viewport, etc.)
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "rules"))]
#[allow(dead_code)]
struct RuleRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key))]
    id: String,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    name: String,

    description: Option<String>,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    flow_json: String, // JSON stored as TEXT

    #[cfg_attr(feature = "schema-macro", column(default = "true"))]
    enabled: bool,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    priority: i32,

    #[cfg_attr(feature = "schema-macro", column(default = "CURRENT_TIMESTAMP"))]
    created_at: String, // TIMESTAMP

    #[cfg_attr(feature = "schema-macro", column(default = "CURRENT_TIMESTAMP"))]
    updated_at: String, // TIMESTAMP
}

/// Rule history table record
/// Stores rule execution history - tracks when and how rules were executed
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "rule_history"))]
#[allow(dead_code)]
struct RuleHistoryRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key, autoincrement))]
    id: i64,

    #[cfg_attr(feature = "schema-macro", column(not_null, references = "rules(id)"))]
    rule_id: String,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    triggered_at: String, // TIMESTAMP

    execution_result: Option<String>,

    error: Option<String>,
}

// Generate table SQL from Schema structs
#[cfg(feature = "schema-macro")]
pub const RULES_TABLE: &str = RuleRecord::CREATE_TABLE_SQL;
#[cfg(feature = "schema-macro")]
pub const RULE_HISTORY_TABLE: &str = RuleHistoryRecord::CREATE_TABLE_SQL;

// Fallback for non-schema-macro builds
#[cfg(not(feature = "schema-macro"))]
pub const RULES_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rules (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        flow_json TEXT NOT NULL,
        enabled BOOLEAN DEFAULT TRUE,
        priority INTEGER DEFAULT 0,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"#;

#[cfg(not(feature = "schema-macro"))]
pub const RULE_HISTORY_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rule_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        rule_id TEXT NOT NULL,
        triggered_at TIMESTAMP NOT NULL,
        execution_result TEXT,
        error TEXT,
        FOREIGN KEY (rule_id) REFERENCES rules(id)
    )
"#;

// Default value functions
fn default_true() -> bool {
    true
}

impl Default for RulesrvConfig {
    fn default() -> Self {
        let service = BaseServiceConfig {
            name: "rulesrv".to_string(),
            ..Default::default()
        };

        let api = ApiConfig {
            host: "0.0.0.0".to_string(),
            port: 6003, // rulesrv default port
        };

        Self {
            service,
            api,
            redis: RedisConfig::default(),
            execution: ExecutionConfig::default(),
        }
    }
}

// ============================================================================
// Validation implementations
// ============================================================================

use crate::common::{ConfigValidator, ValidationLevel, ValidationResult};
use anyhow::{Context, Result};

impl ConfigValidator for RulesrvConfig {
    fn validate_syntax(&self) -> Result<ValidationResult> {
        Ok(ValidationResult::new(ValidationLevel::Syntax))
    }

    fn validate_schema(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Schema);

        // Validate common components
        self.service.validate(&mut result);
        self.api.validate(&mut result);
        self.redis.validate(&mut result);

        Ok(result)
    }

    fn validate_business(&self) -> Result<ValidationResult> {
        let result = ValidationResult::new(ValidationLevel::Business);
        Ok(result)
    }

    fn validate_runtime(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Runtime);

        // Port availability check
        self.api.validate_runtime(&mut result);

        Ok(result)
    }
}

impl RuleConfig {
    /// Validate individual rule configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        if self.core.id.is_empty() {
            result.add_error("Rule ID cannot be empty".to_string());
        }

        if self.core.name.is_empty() {
            result.add_error("Rule name cannot be empty".to_string());
        }

        // Validate flow_json is not null
        if self.flow_json.is_null() {
            result.add_error(format!("Rule {} has null flow_json", self.core.name));
        }

        // Priority warnings
        if self.core.priority > 1000 {
            result.add_warning(format!(
                "Rule {} has unusually high priority: {}",
                self.core.name, self.core.priority
            ));
        }
    }
}

/// Helper validator for backward compatibility
pub struct RulesrvValidator {
    config: Option<RulesrvConfig>,
    raw_yaml: Option<serde_yaml::Value>,
}

impl RulesrvValidator {
    pub fn from_yaml(yaml: serde_yaml::Value) -> Self {
        let config = serde_yaml::from_value(yaml.clone()).ok();
        Self {
            config,
            raw_yaml: Some(yaml),
        }
    }

    pub fn from_config(config: RulesrvConfig) -> Self {
        Self {
            config: Some(config),
            raw_yaml: None,
        }
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Deserialize directly from string to capture line/column information
        let config = serde_yaml::from_str::<RulesrvConfig>(&content).map_err(|e| {
            if let Some(location) = e.location() {
                anyhow::anyhow!(
                    "Configuration error in {}:{}:{}\n  {}",
                    path.display(),
                    location.line(),
                    location.column(),
                    e
                )
            } else {
                anyhow::anyhow!("Configuration error in {}\n  {}", path.display(), e)
            }
        })?;

        // Also parse as YAML Value for raw_yaml field
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        Ok(Self {
            config: Some(config),
            raw_yaml: Some(yaml),
        })
    }
}

impl ConfigValidator for RulesrvValidator {
    fn validate_syntax(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Syntax);

        if self.config.is_none() {
            if let Some(yaml) = &self.raw_yaml {
                match serde_yaml::from_value::<RulesrvConfig>(yaml.clone()) {
                    Ok(_) => {
                        result.add_warning("Configuration parsed but not stored".to_string());
                    },
                    Err(e) => {
                        result.add_error(format!("Invalid YAML syntax: {}", e));
                    },
                }
            } else {
                result.add_error("No configuration data available".to_string());
            }
        }

        Ok(result)
    }

    fn validate_schema(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_schema(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Schema);
                result.add_error("Configuration parsing failed".to_string());
                Ok(result)
            },
        }
    }

    fn validate_business(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_business(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Business);
                result.add_error("Configuration not available".to_string());
                Ok(result)
            },
        }
    }

    fn validate_runtime(&self) -> Result<ValidationResult> {
        match &self.config {
            Some(config) => config.validate_runtime(),
            None => {
                let mut result = ValidationResult::new(ValidationLevel::Runtime);
                result.add_error("Configuration not available".to_string());
                Ok(result)
            },
        }
    }
}

// ============================================================================
// Vue Flow Rule Structures (Parsed/Flattened)
// ============================================================================

/// Rule - parsed and flattened structure for execution
/// This is the internal representation used by the execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Rule {
    /// Unique identifier
    pub id: String,

    /// Rule name
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Execution priority (higher = earlier)
    #[serde(default)]
    pub priority: u32,

    /// Cooldown period in milliseconds
    #[serde(default)]
    pub cooldown_ms: u64,

    /// Variable definitions (data sources)
    pub variables: Vec<Variable>,

    /// Parsed flow nodes
    pub nodes: Vec<FlowNode>,

    /// Node ID to start execution from
    pub start_node_id: String,

    /// Original flow_json for frontend editing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_json: Option<serde_json::Value>,
}

/// Instance point type (M/A)
/// Used for reading from modsrv instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum InstancePointType {
    /// M - Measurement (测量点) - Read from inst:{id}:M
    #[serde(rename = "M")]
    Measurement,
    /// A - Action (动作点) - Read from inst:{id}:A
    #[serde(rename = "A")]
    Action,
}

/// Re-export PointType as ChannelPointType for clarity
/// T = Telemetry, S = Signal, C = Control, A = Adjustment
pub use crate::protocols::PointType as ChannelPointType;

/// Variable definition for data acquisition
///
/// Variables define where to read data from:
/// - Instance: Read from modsrv instance points (inst:{id}:M or inst:{id}:A)
/// - Channel: Read directly from comsrv channel points (comsrv:{id}:T/S/C/A)
/// - Combined: Calculate from other variables using a formula
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Variable {
    /// Instance point - read from modsrv instance (recommended)
    /// Redis key: inst:{instance_id}:M or inst:{instance_id}:A
    Instance {
        /// Variable name (e.g., "pv_voltage")
        name: String,
        /// Instance ID (numeric)
        instance_id: u16,
        /// Point type: M (Measurement) or A (Action)
        point_type: InstancePointType,
        /// Point ID (hash field in Redis)
        point_id: String,
    },
    /// Channel point - read directly from comsrv channel
    /// Redis key: comsrv:{channel_id}:T/S/C/A
    Channel {
        /// Variable name (e.g., "raw_temp")
        name: String,
        /// Channel ID
        channel_id: u16,
        /// Point type: T/S/C/A
        point_type: ChannelPointType,
        /// Point ID (hash field in Redis)
        point_id: String,
    },
    /// Combined value calculated from formula
    Combined {
        /// Variable name (e.g., "power_sum")
        name: String,
        /// Formula tokens for calculation
        formula: Vec<FormulaToken>,
    },
}

/// Formula token for expression evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FormulaToken {
    /// Variable reference
    Var { name: String },
    /// Numeric constant
    Num { value: f64 },
    /// Arithmetic operator
    Op { op: ArithmeticOp },
}

/// Arithmetic operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ArithmeticOp {
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "/")]
    Div,
}

/// Flow node types (parsed from Vue Flow nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowNode {
    /// Start node - entry point
    Start { id: String, next: String },
    /// End node - termination point
    End { id: String },
    /// Switch node - conditional branching
    Switch { id: String, rules: Vec<SwitchRule> },
    /// Change value action
    ChangeValue {
        id: String,
        changes: Vec<ValueChange>,
        next: String,
    },
}

impl FlowNode {
    /// Get node ID
    pub fn id(&self) -> &str {
        match self {
            FlowNode::Start { id, .. } => id,
            FlowNode::End { id } => id,
            FlowNode::Switch { id, .. } => id,
            FlowNode::ChangeValue { id, .. } => id,
        }
    }
}

/// Switch rule - one branch in a switch node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SwitchRule {
    /// Output port name (e.g., "out001")
    pub name: String,

    /// Conditions to evaluate
    pub conditions: Vec<RuleCondition>,

    /// Next node ID if conditions match
    pub next_node: String,
}

/// Single condition in a rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleCondition {
    /// Left operand (variable name or literal)
    pub left: String,

    /// Comparison operator
    pub operator: CompareOp,

    /// Right operand (variable name or literal)
    pub right: String,

    /// Logical relation to next condition (AND/OR)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<LogicOp>,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum CompareOp {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = "<=")]
    Lte,
}

/// Logical operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum LogicOp {
    #[serde(rename = "&&")]
    And,
    #[serde(rename = "||")]
    Or,
}

/// Value change action - defines where to write data
///
/// Supports two target types:
/// - Instance: Write to modsrv instance action point (triggers M2C routing)
/// - Channel: Write directly to comsrv channel point (bypasses routing, use with caution)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ValueChange {
    /// Write to instance action point (recommended)
    /// Uses M2C routing: inst:{id}:A → route:m2c → comsrv:{ch}:A:TODO
    Instance {
        /// Instance ID
        instance_id: u16,
        /// Action point ID
        point_id: String,
        /// New value (can reference variable names like "X1")
        value: String,
    },
    /// Write directly to channel point (bypasses routing, use with caution)
    /// Typically used for C (Control) or A (Adjustment) point types
    Channel {
        /// Channel ID
        channel_id: u16,
        /// Point type (usually C or A)
        point_type: ChannelPointType,
        /// Point ID
        point_id: String,
        /// New value (can reference variable names)
        value: String,
    },
}

// ============================================================================
// SQLite Table for Rule Chains
// ============================================================================

/// Rule chains table record
/// Stores parsed rule chains with both flattened structure and original JSON
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "rules"))]
#[allow(dead_code)]
struct RuleChainRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key))]
    id: String,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    name: String,

    description: Option<String>,

    #[cfg_attr(feature = "schema-macro", column(default = "true"))]
    enabled: bool,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    priority: i32,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    cooldown_ms: i64,

    /// Parsed variables as JSON
    #[cfg_attr(feature = "schema-macro", column(not_null))]
    variables_json: String,

    /// Parsed nodes as JSON
    #[cfg_attr(feature = "schema-macro", column(not_null))]
    nodes_json: String,

    /// Start node ID
    #[cfg_attr(feature = "schema-macro", column(not_null))]
    start_node_id: String,

    /// Original flow_json for frontend editing
    #[cfg_attr(feature = "schema-macro", column(not_null))]
    flow_json: String,

    #[cfg_attr(feature = "schema-macro", column(default = "CURRENT_TIMESTAMP"))]
    created_at: String,

    #[cfg_attr(feature = "schema-macro", column(default = "CURRENT_TIMESTAMP"))]
    updated_at: String,
}

#[cfg(feature = "schema-macro")]
pub const RULE_CHAINS_TABLE: &str = RuleChainRecord::CREATE_TABLE_SQL;

#[cfg(not(feature = "schema-macro"))]
pub const RULE_CHAINS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rules (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        enabled BOOLEAN DEFAULT TRUE,
        priority INTEGER DEFAULT 0,
        cooldown_ms INTEGER DEFAULT 0,
        variables_json TEXT NOT NULL,
        nodes_json TEXT NOT NULL,
        start_node_id TEXT NOT NULL,
        flow_json TEXT NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"#;
