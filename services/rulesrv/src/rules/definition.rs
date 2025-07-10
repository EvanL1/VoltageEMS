use crate::error::{Result, RulesrvError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

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
