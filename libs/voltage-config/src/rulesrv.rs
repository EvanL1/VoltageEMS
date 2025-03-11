//! Rulesrv service configuration structures

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
        workers: None,
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
#[cfg_attr(feature = "schema-macro", derive(Schema))]
#[cfg_attr(feature = "schema-macro", table(name = "service_config"))]
#[allow(dead_code)]
struct ServiceConfigRecord {
    #[cfg_attr(feature = "schema-macro", column(primary_key))]
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
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        type TEXT DEFAULT 'string',
        description TEXT,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
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

/// Rule execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ExecutionConfig {
    /// Rule execution interval in seconds
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,

    /// Number of rules to process in batch
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

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
fn default_interval() -> u64 {
    10 // Execute rules every 10 seconds by default
}

fn default_batch_size() -> usize {
    100 // Process 100 rules in batch by default
}

fn default_true() -> bool {
    true
}

// Default implementations
impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            interval_seconds: default_interval(),
            batch_size: default_batch_size(),
        }
    }
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
            workers: None,
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

        // Validate execution config
        self.execution.validate(&mut result);

        Ok(result)
    }

    fn validate_business(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Business);

        // Validate execution intervals
        if self.execution.interval_seconds > 3600 {
            result.add_warning(
                "Execution interval is greater than 1 hour, rules may not run frequently enough"
                    .to_string(),
            );
        } else if self.execution.interval_seconds < 1 {
            result.add_error("Execution interval must be at least 1 second".to_string());
        }

        if self.execution.batch_size > 1000 {
            result.add_warning("Large batch size may impact performance".to_string());
        }

        Ok(result)
    }

    fn validate_runtime(&self) -> Result<ValidationResult> {
        let mut result = ValidationResult::new(ValidationLevel::Runtime);

        // Port availability check
        self.api.validate_runtime(&mut result);

        Ok(result)
    }
}

impl ExecutionConfig {
    /// Validate execution configuration
    pub fn validate(&self, result: &mut ValidationResult) {
        if self.interval_seconds == 0 {
            result.add_error("Execution interval cannot be 0".to_string());
        }

        if self.batch_size == 0 {
            result.add_error("Batch size cannot be 0".to_string());
        }
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
