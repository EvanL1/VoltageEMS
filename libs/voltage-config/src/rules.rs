//! Rules service configuration structures
//!
//! This module defines:
//! - Service configuration (RulesConfig)
//! - Rule chain structures for Vue Flow execution (RuleChain, FlowNode, etc.)
//! - SQLite table schemas

use crate::common::{ApiConfig, BaseServiceConfig, RedisConfig};
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema")]
use schemars::JsonSchema;

#[cfg(feature = "schema-macro")]
use voltage_schema_macro::Schema;

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
// Database Schema Definitions
// ============================================================================

/// Service configuration table record
/// Maps to RulesConfig for service-level settings
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
/// Stores rule definitions as compact flow JSON (only execution-necessary data)
/// Variables are stored per-node, not globally
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
use anyhow::{Context, Result};

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
pub struct RulesValidator {
    config: Option<RulesConfig>,
    raw_yaml: Option<serde_yaml::Value>,
}

impl RulesValidator {
    pub fn from_yaml(yaml: serde_yaml::Value) -> Self {
        let config = serde_yaml::from_value(yaml.clone()).ok();
        Self {
            config,
            raw_yaml: Some(yaml),
        }
    }

    pub fn from_config(config: RulesConfig) -> Self {
        Self {
            config: Some(config),
            raw_yaml: None,
        }
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Deserialize directly from string to capture line/column information
        let config = serde_yaml::from_str::<RulesConfig>(&content).map_err(|e| {
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

impl ConfigValidator for RulesValidator {
    fn validate_syntax(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Syntax);

        if self.config.is_none() {
            if let Some(yaml) = &self.raw_yaml {
                match serde_yaml::from_value::<RulesConfig>(yaml.clone()) {
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

/// Rule - execution structure with compact flow topology
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
        id TEXT PRIMARY KEY,
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
