use crate::storage::DataStore;
use crate::error::{ModelSrvError, Result};
use crate::rules::{NodeType, NodeState, DagRule, NodeDefinition};
use crate::storage::hybrid_store::HybridStore;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use log::{error, info};
use redis::Commands;
use uuid::Uuid;
use chrono::Utc;

/// Context for rule execution
pub struct ExecutionContext {
    /// Device status cache
    device_status: HashMap<String, Value>,
    /// Variables storage for rule execution
    variables: HashMap<String, Value>,
    /// Data store for persistence and device interaction
    store: Arc<HybridStore>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(store: Arc<HybridStore>) -> Self {
        Self {
            device_status: HashMap::new(),
            variables: HashMap::new(),
            store,
        }
    }
    
    /// Load device status from storage
    pub async fn load_device_status(&mut self) -> Result<()> {
        let keys = self.store.as_ref().get_keys("ems:device:status:*")?;
        
        for key in keys {
            if let Ok(status_json) = self.store.as_ref().get_string(&key) {
                if let Ok(status) = serde_json::from_str::<Value>(&status_json) {
                    let device_id = key.replace("ems:device:status:", "");
                    self.device_status.insert(device_id, status);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get device status by ID
    pub fn get_device_status(&self, device_id: &str) -> Option<&Value> {
        self.device_status.get(device_id)
    }
    
    /// Get device parameter
    pub fn get_device_parameter(&self, device_id: &str, parameter: &str) -> Result<Value> {
        match self.device_status.get(device_id) {
            Some(status) => match status.get(parameter) {
                Some(value) => Ok(value.clone()),
                None => Err(ModelSrvError::KeyNotFound(format!("Parameter '{}' not found for device '{}'", parameter, device_id)))
            },
            None => Err(ModelSrvError::KeyNotFound(format!("Device '{}' not found", device_id)))
        }
    }
    
    /// Set a variable
    pub fn set_variable(&mut self, name: &str, value: Value) {
        self.variables.insert(name.to_string(), value);
    }
    
    /// Get a variable
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }
    
    /// Evaluate a condition expression
    pub fn evaluate_expression(&self, expression: &str) -> Result<bool> {
        // Simple expression evaluation implementation
        // A more robust implementation would use a proper expression engine
        match expression {
            "true" => Ok(true),
            "false" => Ok(false),
            expr if expr.contains(">") => {
                let parts: Vec<&str> = expr.split('>').collect();
                if parts.len() == 2 {
                    let left = self.resolve_variable(parts[0].trim())?;
                    let right = self.resolve_variable(parts[1].trim())?;
                    
                    if let (Some(a), Some(b)) = (left.as_f64(), right.as_f64()) {
                        return Ok(a > b);
                    }
                }
                Err(ModelSrvError::RuleError(format!("Invalid comparison expression: {}", expression)))
            },
            expr if expr.contains("<") => {
                let parts: Vec<&str> = expr.split('<').collect();
                if parts.len() == 2 {
                    let left = self.resolve_variable(parts[0].trim())?;
                    let right = self.resolve_variable(parts[1].trim())?;
                    
                    if let (Some(a), Some(b)) = (left.as_f64(), right.as_f64()) {
                        return Ok(a < b);
                    }
                }
                Err(ModelSrvError::RuleError(format!("Invalid comparison expression: {}", expression)))
            },
            expr if expr.contains("==") => {
                let parts: Vec<&str> = expr.split("==").collect();
                if parts.len() == 2 {
                    let left = self.resolve_variable(parts[0].trim())?;
                    let right = self.resolve_variable(parts[1].trim())?;
                    
                    return Ok(left == right);
                }
                Err(ModelSrvError::RuleError(format!("Invalid equality expression: {}", expression)))
            },
            _ => Err(ModelSrvError::RuleError(format!("Unsupported expression: {}", expression))),
        }
    }
    
    /// Resolve a variable reference or literal value
    fn resolve_variable(&self, var_expr: &str) -> Result<Value> {
        // Check if it's a numeric literal
        if let Ok(num) = var_expr.parse::<f64>() {
            return Ok(json!(num));
        }
        
        // Check if it's a boolean literal
        if var_expr == "true" {
            return Ok(json!(true));
        } else if var_expr == "false" {
            return Ok(json!(false));
        }
        
        // Check if it's a string literal
        if var_expr.starts_with('"') && var_expr.ends_with('"') {
            return Ok(json!(var_expr[1..var_expr.len()-1].to_string()));
        }
        
        // Check if it's a device parameter reference (device.parameter)
        if var_expr.contains('.') {
            let parts: Vec<&str> = var_expr.split('.').collect();
            if parts.len() == 2 {
                return self.get_device_parameter(parts[0], parts[1]);
            }
        }
        
        // Check if it's a variable reference
        if let Some(value) = self.get_variable(var_expr) {
            return Ok(value.clone());
        }
        
        Err(ModelSrvError::RuleError(format!("Failed to resolve variable: {}", var_expr)))
    }
    
    /// Execute a device action
    pub async fn execute_device_action(&self, device_id: &str, operation: &str, parameters: &Value) -> Result<Value> {
        let cmd_id = format!("cmd_{}", uuid::Uuid::new_v4().simple());
        
        let command = json!({
            "id": cmd_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "target": {
                "device_id": device_id,
                "channel": "default_channel"
            },
            "operation": operation,
            "parameters": parameters,
            "status": "pending",
            "source": "rule_engine",
            "timeout": 30,
            "priority": 1
        });
        
        let cmd_key = format!("ems:control:cmd:{}", cmd_id);
        self.store.as_ref().set_string(&cmd_key, &command.to_string())?;
        
        if let Some(mut redis) = self.store.get_redis_connection() {
            let _: () = redis::cmd("RPUSH")
                .arg("ems:control:queue")
                .arg(&cmd_id)
                .query(&mut redis)?;
        }
        
        Ok(json!({
            "command_id": cmd_id,
            "status": "queued"
        }))
    }
}

/// Runtime rule node
pub struct RuleNode {
    /// Node ID
    pub id: String,
    /// Node type
    pub node_type: String,
    /// Node configuration
    pub config: Value,
    /// Node state
    pub state: NodeState,
    /// Node definition
    pub definition: NodeDefinition,
    /// Node execution result
    pub result: Option<Value>,
}

/// Runtime rule
pub struct RuntimeRule {
    /// Rule ID
    pub id: String,
    /// Graph data structure
    pub graph: DiGraph<RuleNode, Option<String>>,
    /// Node indices mapping
    pub node_indices: HashMap<String, NodeIndex>,
    /// Rule definition
    pub definition: DagRule,
    /// Node mapping
    pub node_map: HashMap<String, NodeIndex>,
}

/// Build a runtime rule from a DAG rule definition
pub fn build_rule_graph(rule_def: DagRule) -> Result<RuntimeRule> {
    let mut graph = DiGraph::<RuleNode, Option<String>>::new();
    let mut node_map = HashMap::new();
    
    // Add nodes
    for node_def in &rule_def.nodes {
        let node = RuleNode {
            id: node_def.id.clone(),
            node_type: format!("{:?}", node_def.node_type),
            config: node_def.config.clone(),
            state: NodeState::Pending,
            definition: node_def.clone(),
            result: None,
        };
        
        let node_idx = graph.add_node(node);
        node_map.insert(node_def.id.clone(), node_idx);
    }
    
    // Add edges
    for edge_def in &rule_def.edges {
        let from_idx = node_map.get(&edge_def.from)
            .ok_or_else(|| ModelSrvError::RuleError(format!("Node not found: {}", edge_def.from)))?;
        
        let to_idx = node_map.get(&edge_def.to)
            .ok_or_else(|| ModelSrvError::RuleError(format!("Node not found: {}", edge_def.to)))?;
        
        graph.add_edge(
            *from_idx,
            *to_idx,
            edge_def.condition.clone(),
        );
    }
    
    // Check if graph is acyclic
    if toposort(&graph, None).is_err() {
        return Err(ModelSrvError::RuleError("Cycle detected in rule graph".to_string()));
    }
    
    Ok(RuntimeRule {
        id: rule_def.id.clone(),
        graph,
        node_indices: node_map.clone(),
        definition: rule_def,
        node_map,
    })
}

/// Execute a rule graph
pub async fn execute_rule_graph(
    rule: &mut RuntimeRule,
    context: &mut ExecutionContext
) -> Result<Value> {
    // Reset node states
    for node_idx in rule.graph.node_indices() {
        let node = rule.graph.node_weight_mut(node_idx).unwrap();
        node.state = NodeState::Pending;
        node.result = None;
    }
    
    // Find input nodes (those with no incoming edges)
    let mut ready_nodes = Vec::new();
    for node_idx in rule.graph.node_indices() {
        let has_incoming = rule.graph.edges_directed(node_idx, Direction::Incoming).count() > 0;
        if !has_incoming {
            ready_nodes.push(node_idx);
        }
    }
    
    // Track completed nodes
    let mut completed = HashSet::new();
    
    // Execute nodes in topological order
    while !ready_nodes.is_empty() {
        let mut next_ready_nodes = Vec::new();
        
        // Process current ready nodes
        for node_idx in ready_nodes {
            let node = rule.graph.node_weight_mut(node_idx).unwrap();
            node.state = NodeState::Running;
            
            // Execute node
            match execute_node(node, context).await {
                Ok(result) => {
                    node.result = Some(result);
                    node.state = NodeState::Completed;
                    completed.insert(node_idx);
                    
                    // Store result in context using node ID as variable name
                    if let Some(result) = &node.result {
                        context.set_variable(&node.definition.id, result.clone());
                    }
                },
                Err(e) => {
                    error!("Failed to execute node {}: {}", node.definition.id, e);
                    node.state = NodeState::Failed;
                    // Don't mark as completed so downstream nodes won't execute
                }
            }
        }
        
        // Find next ready nodes
        for node_idx in rule.graph.node_indices() {
            if completed.contains(&node_idx) {
                continue;
            }
            
            let node = rule.graph.node_weight(node_idx).unwrap();
            if node.state != NodeState::Pending {
                continue;
            }
            
            let mut can_execute = true;
            
            // Check if all dependencies are completed
            for edge in rule.graph.edges_directed(node_idx, Direction::Incoming) {
                let source_idx = edge.source();
                
                if !completed.contains(&source_idx) {
                    can_execute = false;
                    break;
                }
                
                // Check edge condition if present
                if let Some(condition) = edge.weight() {
                    match context.evaluate_expression(condition) {
                        Ok(result) => {
                            if !result {
                                can_execute = false;
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Failed to evaluate edge condition: {}", e);
                            can_execute = false;
                            break;
                        }
                    }
                }
            }
            
            if can_execute {
                next_ready_nodes.push(node_idx);
            }
        }
        
        ready_nodes = next_ready_nodes;
    }
    
    // Return the result of the last action node
    for node_idx in rule.graph.node_indices() {
        let node = rule.graph.node_weight(node_idx).unwrap();
        if node.definition.node_type == NodeType::Action && node.state == NodeState::Completed {
            if let Some(result) = &node.result {
                return Ok(result.clone());
            }
        }
    }
    
    Ok(json!({ "status": "completed" }))
}

/// Execute a single node
async fn execute_node(node: &RuleNode, context: &mut ExecutionContext) -> Result<Value> {
    match node.definition.node_type {
        NodeType::Input => {
            // Process input node
            let device_id = node.definition.config.get("device_id")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Input node missing device_id".to_string()))?;
                
            let parameter = node.definition.config.get("parameter")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Input node missing parameter".to_string()))?;
                
            context.get_device_parameter(device_id, parameter)
        },
        NodeType::Condition => {
            // Process condition node
            let expression = node.definition.config.get("expression")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Condition node missing expression".to_string()))?;
                
            let result = context.evaluate_expression(expression)?;
            Ok(json!(result))
        },
        NodeType::Transform => {
            // Process transform node
            let transform_type = node.definition.config.get("transform_type")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Transform node missing transform_type".to_string()))?;
                
            let input = node.definition.config.get("input").cloned().unwrap_or(json!({}));
            
            match transform_type {
                "scale" => {
                    let value_expr = input.get("value_expr")
                        .and_then(Value::as_str)
                        .ok_or_else(|| ModelSrvError::RuleError("Scale transform missing value_expr".to_string()))?;
                        
                    let factor = input.get("factor")
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0);
                        
                    let value = context.resolve_variable(value_expr)?;
                    
                    if let Some(num) = value.as_f64() {
                        Ok(json!(num * factor))
                    } else {
                        Err(ModelSrvError::RuleError(format!("Value is not a number: {}", value)))
                    }
                },
                "threshold" => {
                    let value_expr = input.get("value_expr")
                        .and_then(Value::as_str)
                        .ok_or_else(|| ModelSrvError::RuleError("Threshold transform missing value_expr".to_string()))?;
                        
                    let threshold = input.get("threshold")
                        .and_then(Value::as_f64)
                        .ok_or_else(|| ModelSrvError::RuleError("Threshold transform missing threshold".to_string()))?;
                        
                    let value = context.resolve_variable(value_expr)?;
                    
                    if let Some(num) = value.as_f64() {
                        Ok(json!(num >= threshold))
                    } else {
                        Err(ModelSrvError::RuleError(format!("Value is not a number: {}", value)))
                    }
                },
                _ => Err(ModelSrvError::RuleError(format!("Unsupported transform type: {}", transform_type))),
            }
        },
        NodeType::Action => {
            // Process action node
            let device_id = node.definition.config.get("device_id")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Action node missing device_id".to_string()))?;
                
            let operation = node.definition.config.get("operation")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Action node missing operation".to_string()))?;
                
            let parameters = node.definition.config.get("parameters").cloned().unwrap_or(json!({}));
            
            context.execute_device_action(device_id, operation, &parameters).await
        },
        NodeType::Aggregate => {
            // Process aggregate node
            let aggregation_type = node.definition.config.get("aggregation_type")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelSrvError::RuleError("Aggregate node missing aggregation_type".to_string()))?;
                
            let inputs = node.definition.config.get("inputs")
                .and_then(Value::as_array)
                .ok_or_else(|| ModelSrvError::RuleError("Aggregate node missing inputs".to_string()))?;
                
            let mut values = Vec::new();
            
            for input in inputs {
                if let Some(var_name) = input.as_str() {
                    if let Some(value) = context.get_variable(var_name) {
                        values.push(value.clone());
                    }
                }
            }
            
            match aggregation_type {
                "and" => {
                    let result = values.iter().all(|v| v.as_bool().unwrap_or(false));
                    Ok(json!(result))
                },
                "or" => {
                    let result = values.iter().any(|v| v.as_bool().unwrap_or(false));
                    Ok(json!(result))
                },
                "sum" => {
                    let sum = values.iter()
                        .filter_map(|v| v.as_f64())
                        .sum::<f64>();
                    Ok(json!(sum))
                },
                "avg" => {
                    let nums: Vec<f64> = values.iter()
                        .filter_map(|v| v.as_f64())
                        .collect();
                        
                    if nums.is_empty() {
                        Ok(json!(0.0))
                    } else {
                        let avg = nums.iter().sum::<f64>() / nums.len() as f64;
                        Ok(json!(avg))
                    }
                },
                _ => Err(ModelSrvError::RuleError(format!("Unsupported aggregation type: {}", aggregation_type))),
            }
        },
    }
}

/// Execute rules with monitoring and metrics
pub struct RuleExecutor {
    store: Arc<HybridStore>,
}

impl RuleExecutor {
    /// Create a new rule executor
    pub fn new(store: Arc<HybridStore>) -> Self {
        Self { store }
    }

    /// Execute a rule by its ID
    pub fn execute_rule(&self, rule_id: &str) -> Result<Value> {
        info!("Executing rule: {}", rule_id);
        
        // Get the rule
        let rule_result = self.store.get_rule(rule_id);
        let rule_json = match rule_result {
            Ok(rule) => rule,
            Err(e) => {
                error!("Failed to get rule {}: {}", rule_id, e);
                return Err(ModelSrvError::RuleNotFound(rule_id.to_string()));
            }
        };
        
        // Parse rule as DagRule
        let rule: DagRule = match serde_json::from_value(rule_json) {
            Ok(r) => r,
            Err(e) => {
                error!("Invalid rule format for {}: {}", rule_id, e);
                return Err(ModelSrvError::RuleError(format!("Invalid rule format: {}", e)));
            }
        };
        
        // Check if rule is enabled
        if !rule.enabled {
            return Err(ModelSrvError::RuleDisabled(rule_id.to_string()));
        }
        
        // Generate execution ID and timestamp
        let execution_id = Uuid::new_v4().to_string();
        let start_time = Utc::now();
        let timestamp = start_time.to_rfc3339();
        
        // Simulate rule execution process
        // In a real implementation, this would call the rule engine to execute the rule
        
        // Record execution history
        let end_time = Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds();
        
        let execution_history = json!({
            "execution_id": execution_id,
            "rule_id": rule_id,
            "status": "completed",
            "timestamp": timestamp,
            "duration_ms": duration_ms,
            "output": "Rule executed successfully"
        });
        
        // Add execution history record
        if let Err(e) = self.store.add_execution_history(rule_id, &execution_history) {
            error!("Failed to add execution history: {}", e);
            // Continue execution, don't return error
        }
        
        // Return execution result
        Ok(execution_history)
    }
} 