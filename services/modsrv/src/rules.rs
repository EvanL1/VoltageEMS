use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::sync::Arc;
use log::info;

/// Rule node types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Input nodes collect data from devices
    Input,
    /// Condition nodes evaluate conditions
    Condition,
    /// Transform nodes transform data
    Transform,
    /// Action nodes execute device actions
    Action,
    /// Aggregate nodes combine results from other nodes
    Aggregate,
}

/// Node execution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// Node is waiting to be processed
    Pending,
    /// Node is currently being processed
    Running,
    /// Node has been processed successfully
    Completed,
    /// Node processing failed
    Failed,
    /// Node was skipped (e.g., due to edge condition)
    Skipped,
}

/// Graph node definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// Unique node ID
    pub id: String,
    /// Node name
    pub name: String,
    /// Node type
    #[serde(rename = "type")]
    pub node_type: NodeType,
    /// Node configuration
    #[serde(default)]
    pub config: Value,
}

/// Graph edge definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDefinition {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Edge condition (optional)
    #[serde(default)]
    pub condition: Option<String>,
}

/// Rule definition using DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    #[serde(default)]
    pub description: String,
    /// Rule nodes
    pub nodes: Vec<NodeDefinition>,
    /// Rule edges
    pub edges: Vec<EdgeDefinition>,
    /// Whether the rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Rule priority (higher values = higher priority)
    #[serde(default)]
    pub priority: i32,
}

fn default_true() -> bool {
    true
}

/// Rule engine for processing rules
pub struct RuleEngine {
    /// Redis connection for storage
    redis: Arc<RedisConnection>,
}

impl RuleEngine {
    /// Create a new rule engine
    pub fn new(redis: Arc<RedisConnection>) -> Self {
        Self {
            redis,
        }
    }
    
    /// Get mutable connection
    fn get_mutable_connection(&self) -> Result<RedisConnection> {
        self.redis.clone().duplicate()
    }
    
    /// Execute a rule
    pub async fn execute_rule(&self, rule_id: &str) -> Result<Value> {
        info!("Executing rule: {}", rule_id);
        
        // Get rule from Redis
        let mut redis = self.get_mutable_connection()?;
        let key = format!("rule:{}", rule_id);
        
        if !redis.exists(&key)? {
            return Err(ModelSrvError::RuleNotFound(rule_id.to_string()));
        }
        
        let rule_json = redis.get_string(&key)?;
        let rule: DagRule = serde_json::from_str(&rule_json)?;
        
        if !rule.enabled {
            return Err(ModelSrvError::RuleDisabled(rule_id.to_string()));
        }
        
        // TODO: Implement rule execution logic
        
        Ok(json!({
            "status": "executed",
            "rule_id": rule_id,
            "message": "Rule executed successfully"
        }))
    }
} 