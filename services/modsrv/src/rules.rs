use crate::error::{ModelSrvError, Result};
use crate::storage::DataStore;
use crate::storage::hybrid_store::HybridStore;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::visit::EdgeRef;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use log::{debug, error, info, warn};
use serde_json::{json, Value};

// 导出rules子模块
pub mod engine;

/// Rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Rule priority (lower number = higher priority)
    pub priority: i32,
}

/// Node type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeType {
    /// Input node fetches data from devices
    Input,
    /// Condition node evaluates conditions
    Condition,
    /// Transform node transforms data
    Transform,
    /// Action node executes actions
    Action,
    /// Aggregate node combines multiple inputs
    Aggregate,
}

/// Node definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// Node ID
    pub id: String,
    /// Node name
    pub name: String,
    /// Node type
    pub node_type: NodeType,
    /// Node configuration
    pub config: Value,
}

/// Edge definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDefinition {
    /// Edge ID
    pub id: String,
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Optional condition for this edge
    pub condition: Option<String>,
}

/// DAG rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagRule {
    /// Rule base information
    pub rule: Rule,
    /// Nodes in the rule graph
    pub nodes: Vec<NodeDefinition>,
    /// Edges in the rule graph
    pub edges: Vec<EdgeDefinition>,
}

/// Node execution state
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// Node is pending execution
    Pending,
    /// Node is currently running
    Running,
    /// Node has completed execution
    Completed,
    /// Node execution failed
    Failed,
}

/// Runtime rule node with state
#[derive(Debug)]
pub struct RuleNode {
    /// Node definition
    pub definition: NodeDefinition,
    /// Current execution state
    pub state: NodeState,
    /// Execution result
    pub result: Option<Value>,
}

/// Runtime rule
#[derive(Debug)]
pub struct RuntimeRule {
    /// Original rule definition
    pub definition: DagRule,
    /// Rule graph
    pub graph: DiGraph<RuleNode, Option<String>>,
    /// Map from node ID to graph index
    pub node_map: HashMap<String, NodeIndex>,
}

/// Rule Engine
#[derive(Debug)]
pub struct RuleEngine {
    /// Rules storage
    rules: HashMap<String, RuntimeRule>,
    /// Data store
    store: std::sync::Arc<crate::storage::hybrid_store::HybridStore>,
}

impl RuleEngine {
    /// Create a new rule engine
    pub fn new(store: Arc<HybridStore>) -> Self {
        Self {
            rules: HashMap::new(),
            store,
        }
    }
    
    /// Load rules from a JSON file
    pub fn load_rules_from_file(&mut self, file_path: &str) -> Result<()> {
        // Implementation will be added later
        Ok(())
    }
    
    /// Build a rule graph from a rule definition
    fn build_rule_graph(&self, rule_def: DagRule) -> Result<RuntimeRule> {
        // Implementation will be added later
        unimplemented!()
    }
    
    /// Execute a rule
    pub async fn execute_rule(&self, rule_id: &str) -> Result<Value> {
        // Implementation will be added later
        unimplemented!()
    }
    
    /// Execute all rules
    pub async fn execute_all_rules(&self) -> Result<HashMap<String, Value>> {
        // Implementation will be added later
        unimplemented!()
    }
}

// 重新导出engine模块中的函数
pub use engine::{build_rule_graph, execute_rule_graph, ExecutionContext}; 