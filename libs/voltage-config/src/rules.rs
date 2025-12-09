//! Rules service configuration structures
//!
//! This module defines:
//! - Service configuration (RulesConfig)
//! - Rule chain structures for Vue Flow execution (RuleChain, FlowNode, etc.)
//! - SQLite table schemas

use crate::common::{ApiConfig, BaseServiceConfig, RedisConfig};
use serde::{Deserialize, Serialize};
use voltage_schema_macro::Schema;

#[cfg(feature = "schema")]
use schemars::JsonSchema;

/// Default API configuration for rules (port 6002, merged into modsrv)
fn default_rules_api() -> ApiConfig {
    ApiConfig {
        host: "0.0.0.0".to_string(),
        port: 6002,
    }
}

/// Rules service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RulesConfig {
    /// Base service configuration
    #[serde(flatten)]
    pub service: BaseServiceConfig,

    /// API configuration (has default value)
    #[serde(default = "default_rules_api")]
    pub api: ApiConfig,

    /// Redis configuration (simplified)
    #[serde(default)]
    pub redis: RedisConfig,

    /// Execution configuration
    #[serde(default)]
    pub execution: ExecutionConfig,
}

// ============================================================================
// Database Schema Definitions (re-exported from common)
// ============================================================================

/// Service configuration table SQL (from common)
pub use crate::common::SERVICE_CONFIG_TABLE;

/// Sync metadata table SQL (from common)
pub use crate::common::SYNC_METADATA_TABLE;

/// Default port for rules service (merged into modsrv)
pub const DEFAULT_PORT: u16 = 6002;

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
    pub id: i64,

    /// Rule name
    pub name: String,

    /// Rule description
    pub description: Option<String>,

    /// Whether the rule is enabled
    #[serde(default = "crate::serde_defaults::bool_true")]
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
    pub fn id(&self) -> i64 {
        self.core.id
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
/// Stores rule definitions as compact flow JSON (only execution-necessary data)
/// Variables are stored per-node, not globally
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "rules"))]
#[allow(dead_code)]
struct RuleRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key))]
    id: i64,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    name: String,

    description: Option<String>,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    nodes_json: String, // CompactFlow JSON stored as TEXT

    #[cfg_attr(feature = "schema-macro", column(default = "true"))]
    enabled: bool,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    priority: i32,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    cooldown_ms: i64,

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
    rule_id: i64,

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
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        nodes_json TEXT NOT NULL,
        enabled BOOLEAN DEFAULT TRUE,
        priority INTEGER DEFAULT 0,
        cooldown_ms INTEGER DEFAULT 0,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"#;

#[cfg(not(feature = "schema-macro"))]
pub const RULE_HISTORY_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS rule_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        rule_id INTEGER NOT NULL,
        triggered_at TIMESTAMP NOT NULL,
        execution_result TEXT,
        error TEXT,
        FOREIGN KEY (rule_id) REFERENCES rules(id)
    )
"#;

impl Default for RulesConfig {
    fn default() -> Self {
        let service = BaseServiceConfig {
            name: "rules".to_string(),
            ..Default::default()
        };

        let api = ApiConfig {
            host: "0.0.0.0".to_string(),
            port: 6002, // merged into modsrv
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
use anyhow::Result;

impl ConfigValidator for RulesConfig {
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
        if self.core.id <= 0 {
            result.add_error("Rule ID must be positive".to_string());
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

/// Type alias for backward compatibility - use GenericValidator directly for new code
pub type RulesValidator = crate::common::GenericValidator<RulesConfig>;

// ============================================================================
// Vue Flow Rule Structures (Parsed/Flattened)
// ============================================================================

/// Rule - execution structure with compact flow topology
/// This is the internal representation used by the execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Rule {
    /// Unique identifier
    pub id: i64,

    /// Rule name
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Whether the rule is enabled
    #[serde(default = "crate::serde_defaults::bool_true")]
    pub enabled: bool,

    /// Execution priority (higher = earlier)
    #[serde(default)]
    pub priority: u32,

    /// Cooldown period in milliseconds
    #[serde(default)]
    pub cooldown_ms: u64,

    /// Rule flow topology (nodes with local variables)
    pub flow: RuleFlow,
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
    id: i64,

    #[cfg_attr(feature = "schema-macro", column(not_null))]
    name: String,

    description: Option<String>,

    #[cfg_attr(feature = "schema-macro", column(default = "true"))]
    enabled: bool,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    priority: i32,

    #[cfg_attr(feature = "schema-macro", column(default = "0"))]
    cooldown_ms: i64,

    /// Parsed compact flow as JSON (for execution)
    #[cfg_attr(feature = "schema-macro", column(not_null))]
    nodes_json: String,

    /// Original Vue Flow JSON (for frontend editing)
    flow_json: Option<String>,

    /// Format type: "vue-flow" (default)
    #[cfg_attr(feature = "schema-macro", column(default = "'vue-flow'"))]
    format: String,

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
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        enabled BOOLEAN DEFAULT TRUE,
        priority INTEGER DEFAULT 0,
        cooldown_ms INTEGER DEFAULT 0,
        nodes_json TEXT NOT NULL,
        flow_json TEXT,
        format TEXT DEFAULT 'vue-flow',
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"#;

// ============================================================================
// Compact Flow Structures (Vue Flow → Simplified Topology)
// ============================================================================

use std::collections::HashMap;

/// Rule flow topology - simplified structure for execution
/// Extracted from full Vue Flow JSON, discarding UI-only information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleFlow {
    /// ID of the start node
    pub start_node: String,

    /// Nodes indexed by ID for O(1) lookup
    pub nodes: HashMap<String, RuleNode>,
}

/// Rule node - execution-only node structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type")]
pub enum RuleNode {
    /// Start node - entry point
    #[serde(rename = "start")]
    Start {
        /// Output wires
        wires: RuleWires,
    },

    /// End node - termination point
    #[serde(rename = "end")]
    End,

    /// Switch node - conditional branching (function-switch)
    #[serde(rename = "function-switch")]
    Switch {
        /// Node-local variable definitions
        variables: Vec<RuleVariable>,
        /// Condition rules
        rule: Vec<RuleSwitchBranch>,
        /// Output wires (keyed by output name, e.g., "out001")
        wires: HashMap<String, Vec<String>>,
    },

    /// Change value action node
    #[serde(rename = "action-changeValue")]
    ChangeValue {
        /// Node-local variable definitions (target points)
        variables: Vec<RuleVariable>,
        /// Value assignments
        rule: Vec<RuleValueAssignment>,
        /// Output wires
        wires: RuleWires,
    },

    /// Calculation action node - formula evaluation
    #[serde(rename = "action-calculation")]
    Calculation {
        /// Input variables (also serve as output targets)
        variables: Vec<RuleVariable>,
        /// Calculation rules with formulas
        rule: Vec<CalculationRule>,
        /// Output wires
        wires: RuleWires,
    },
}

/// Rule wires - output connections
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleWires {
    /// Default output connections
    #[serde(default)]
    pub default: Vec<String>,
}

/// Rule variable definition (matches Vue Flow format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleVariable {
    /// Variable name (e.g., "X1")
    pub name: String,

    /// Variable type: "single" or "combined"
    #[serde(rename = "type")]
    pub var_type: String,

    /// Instance name (for single type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    /// Point type: "measurement" or "action"
    #[serde(rename = "pointType", skip_serializing_if = "Option::is_none")]
    pub point_type: Option<String>,

    /// Point ID (numeric)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point: Option<u16>,

    /// Formula tokens (for combined type)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formula: Vec<serde_json::Value>,
}

/// Rule switch branch (condition branch)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleSwitchBranch {
    /// Output port name (e.g., "out001")
    pub name: String,

    /// Rule type (currently only "default")
    #[serde(rename = "type")]
    pub rule_type: String,

    /// Conditions
    pub rule: Vec<FlowCondition>,
}

/// Flow condition (used in RuleFlow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FlowCondition {
    /// Condition type: "variable" or "relation"
    #[serde(rename = "type")]
    pub cond_type: String,

    /// Variable name (for variable type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<String>,

    /// Comparison operator: "<=", ">=", "==", "!=", "<", ">"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// Comparison value (number or variable name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// Rule value assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RuleValueAssignment {
    /// Target variable name
    #[serde(rename = "Variables")]
    pub variables: String,

    /// Value to assign (number or variable name)
    pub value: serde_json::Value,
}

/// Calculation rule for formula evaluation
///
/// Used by `action-calculation` nodes to compute values using evalexpr expressions.
/// Supports arithmetic (+, -, *, /), comparison (>, <, >=, <=, ==), logical (&&, ||),
/// and conditional expressions (if(cond, then, else)).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CalculationRule {
    /// Output variable name (must reference a variable in the node)
    pub output: String,

    /// Formula expression (evalexpr syntax, e.g., "a + b * 2")
    pub formula: String,
}
