use crate::error::{ModelSrvError, Result};
use crate::rules::{DagRule, NodeType, NodeState, RuntimeRule, RuleNode};
use crate::storage::DataStore;
use crate::storage::hybrid_store::HybridStore;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use log::{debug, error, info, warn};
use redis::Commands;
use serde::{Serialize, Deserialize};

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

/// Build a runtime rule from a DAG rule definition
pub fn build_rule_graph(rule_def: DagRule) -> Result<RuntimeRule> {
    let mut graph = DiGraph::<RuleNode, Option<String>>::new();
    let mut node_map = HashMap::new();
    
    // Add nodes
    for node_def in &rule_def.nodes {
        let node = RuleNode {
            definition: node_def.clone(),
            state: NodeState::Pending,
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
        definition: rule_def,
        graph,
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

/// Rule executor for executing rules
pub struct RuleExecutor {
    /// Data store
    store: Arc<HybridStore>,
}

impl RuleExecutor {
    /// Create a new rule executor
    pub fn new(store: Arc<HybridStore>) -> Self {
        Self {
            store,
        }
    }
    
    /// Execute a rule
    pub async fn execute_rule(&self, rule: DagRule, context: Option<Value>) -> Result<Value> {
        // Build rule graph
        let mut runtime_rule = self.build_rule_graph(rule)?;
        
        // Create execution context
        let mut exec_context = match context {
            Some(ctx) => {
                let mut context = ExecutionContext::new(self.store.clone());
                for (k, v) in ctx.as_object().unwrap() {
                    context.set_variable(k, v.clone());
                }
                context
            }
            None => ExecutionContext::new(self.store.clone())
        };
        
        // Load device statuses
        exec_context.load_device_status().await?;
        
        // Execute rule graph
        self.execute_graph(&mut runtime_rule, &mut exec_context).await
    }
    
    /// Build a rule graph from a rule definition
    fn build_rule_graph(&self, rule_def: DagRule) -> Result<RuntimeRule> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();
        
        // Create nodes
        for node_def in &rule_def.nodes {
            let rule_node = RuleNode {
                definition: node_def.clone(),
                state: NodeState::Pending,
                result: None,
            };
            
            let node_idx = graph.add_node(rule_node);
            node_map.insert(node_def.id.clone(), node_idx);
        }
        
        // Create edges
        for edge_def in &rule_def.edges {
            let from_idx = node_map.get(&edge_def.from).ok_or_else(|| {
                ModelSrvError::InvalidOperation(format!("Node {} not found", edge_def.from))
            })?;
            
            let to_idx = node_map.get(&edge_def.to).ok_or_else(|| {
                ModelSrvError::InvalidOperation(format!("Node {} not found", edge_def.to))
            })?;
            
            graph.add_edge(*from_idx, *to_idx, edge_def.condition.clone());
        }
        
        Ok(RuntimeRule {
            definition: rule_def,
            graph,
            node_map,
        })
    }
    
    /// Execute a rule graph
    async fn execute_graph(&self, rule: &mut RuntimeRule, context: &mut ExecutionContext) -> Result<Value> {
        // Find root nodes (nodes with no incoming edges)
        let mut root_nodes = Vec::new();
        for node_idx in rule.graph.node_indices() {
            if rule.graph.neighbors_directed(node_idx, Direction::Incoming).count() == 0 {
                root_nodes.push(node_idx);
            }
        }
        
        if root_nodes.is_empty() {
            return Err(ModelSrvError::InvalidOperation("Rule has no root nodes".to_string()));
        }
        
        // Create a queue for execution
        let mut queue = VecDeque::new();
        for node_idx in root_nodes {
            queue.push_back(node_idx);
        }
        
        // Track visited nodes to handle possible cycles
        let mut visited = HashSet::new();
        
        // Execute nodes in topological order
        while let Some(node_idx) = queue.pop_front() {
            if visited.contains(&node_idx) {
                continue;
            }
            
            // Check if all dependencies are satisfied
            let mut dependencies_satisfied = true;
            for dep_idx in rule.graph.neighbors_directed(node_idx, Direction::Incoming) {
                if !visited.contains(&dep_idx) {
                    dependencies_satisfied = false;
                    break;
                }
            }
            
            if !dependencies_satisfied {
                // Put back in queue to process later
                queue.push_back(node_idx);
                continue;
            }
            
            // Execute the node
            let node = &rule.graph[node_idx];
            let node_id = node.definition.id.clone();
            let node_type = node.definition.node_type.clone();
            
            // Set node state to Running
            rule.graph[node_idx].state = NodeState::Running;
            
            // Execute node based on type
            let result = match node_type {
                NodeType::Input => self.execute_input_node(&rule.graph[node_idx], context).await?,
                NodeType::Condition => self.execute_condition_node(&rule.graph[node_idx], context).await?,
                NodeType::Transform => self.execute_transform_node(&rule.graph[node_idx], context).await?,
                NodeType::Action => self.execute_action_node(&rule.graph[node_idx], context).await?,
                NodeType::Aggregate => self.execute_aggregate_node(&rule.graph[node_idx], context, &rule.graph, &visited).await?,
            };
            
            // Store the result
            rule.graph[node_idx].result = Some(result.clone());
            
            // Mark node as completed
            rule.graph[node_idx].state = NodeState::Completed;
            
            // Add node to visited set
            visited.insert(node_idx);
            
            // Store the result in execution context
            context.set_variable(&format!("node.{}.result", node_id), result);
            
            // Add successors to queue
            for succ_idx in rule.graph.neighbors_directed(node_idx, Direction::Outgoing) {
                queue.push_back(succ_idx);
            }
        }
        
        // Collect results from all terminal nodes (nodes with no outgoing edges)
        let mut results = Vec::new();
        for node_idx in rule.graph.node_indices() {
            if rule.graph.neighbors_directed(node_idx, Direction::Outgoing).count() == 0 
               && rule.graph[node_idx].state == NodeState::Completed {
                if let Some(result) = &rule.graph[node_idx].result {
                    results.push(result.clone());
                }
            }
        }
        
        // Return combined results
        if results.is_empty() {
            Ok(json!({"status": "executed", "results": []}))
        } else if results.len() == 1 {
            Ok(json!({"status": "executed", "result": results[0]}))
        } else {
            Ok(json!({"status": "executed", "results": results}))
        }
    }
    
    /// Execute an input node
    async fn execute_input_node(&self, node: &RuleNode, context: &mut ExecutionContext) -> Result<Value> {
        let config = &node.definition.config;
        
        // Get device ID
        let device_id = config["device_id"].as_str()
            .ok_or_else(|| ModelSrvError::InvalidOperation("Input node missing device_id".to_string()))?;
            
        // Get requested data points
        if let Some(data_points) = config.get("data_points") {
            if let Some(points) = data_points.as_array() {
                let mut result = json!({});
                
                for point in points {
                    if let Some(param) = point.as_str() {
                        // Get device parameter
                        let value = context.get_device_parameter(device_id, param)?;
                        result[param] = value;
                    }
                }
                
                return Ok(result);
            }
        }
        
        // Default: return device status
        let device_status = context.get_device_parameter(device_id, "status")?;
        
        Ok(device_status)
    }
    
    /// Execute a condition node
    async fn execute_condition_node(&self, node: &RuleNode, context: &mut ExecutionContext) -> Result<Value> {
        let config = &node.definition.config;
        
        // Get condition expression
        let expression = config["expression"].as_str()
            .ok_or_else(|| ModelSrvError::InvalidOperation("Condition node missing expression".to_string()))?;
            
        // Split expression into left and right parts
        let parts: Vec<&str> = expression.split("==").collect();
        if parts.len() != 2 {
            return Err(ModelSrvError::InvalidOperation("Invalid condition format".to_string()));
        }
        
        let left = parts[0].trim();
        let right = parts[1].trim();
        
        // Get left operand
        let left_value = if left.starts_with("$") {
            // Variable reference
            context.resolve_variable(&left[1..])?.clone()
        } else {
            // Direct reference
            context.get_variable(left).map(|v| v.clone()).unwrap_or(Value::Null)
        };
        
        // Get right operand
        let right_value = if right.starts_with("$") {
            // Variable reference
            context.resolve_variable(&right[1..])?.clone()
        } else if let Ok(num) = right.parse::<i64>() {
            // Integer literal
            Value::Number(num.into())
        } else if let Ok(num) = right.parse::<f64>() {
            // Float literal
            serde_json::Number::from_f64(num)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        } else {
            // Try as variable
            context.get_variable(right).map(|v| v.clone()).unwrap_or(Value::Null)
        };
        
        // Return result wrapped in Result
        Ok(json!({"result": left_value == right_value}))
    }
    
    /// Execute a transform node
    async fn execute_transform_node(&self, node: &RuleNode, context: &mut ExecutionContext) -> Result<Value> {
        let config = &node.definition.config;
        
        // Get transform type
        let transform_type = config["type"].as_str()
            .ok_or_else(|| ModelSrvError::InvalidOperation("Transform node missing type".to_string()))?;
            
        // Execute transform based on type
        match transform_type {
            "map" => {
                // Get input variable
                let input_var = config["input_var"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Map transform missing input_var".to_string()))?;
                    
                // Get mapping rules
                let mappings = config["mappings"].as_object()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Map transform missing mappings".to_string()))?;
                    
                // Get input value
                let input_value = context.get_variable(input_var)
                    .ok_or_else(|| ModelSrvError::InvalidOperation(format!("Variable {} not found", input_var)))?;
                    
                // Apply mapping
                let input_str = input_value.as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Input value must be a string".to_string()))?;
                    
                if let Some(mapped_value) = mappings.get(input_str) {
                    Ok(json!({"result": mapped_value}))
                } else {
                    // Use default if provided
                    if let Some(default) = config["default"].as_str() {
                        Ok(json!({"result": default}))
                    } else {
                        Ok(json!({"result": null, "error": "No mapping found for value"}))
                    }
                }
            },
            "calculate" => {
                // Get formula
                let formula = config["formula"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Calculate transform missing formula".to_string()))?;
                    
                // In a real implementation, we would parse and evaluate the formula
                // For now, we'll just return a dummy result
                Ok(json!({"result": 42, "formula": formula}))
            },
            _ => Err(ModelSrvError::InvalidOperation(format!("Unsupported transform type: {}", transform_type))),
        }
    }
    
    /// Execute an action node
    async fn execute_action_node(&self, node: &RuleNode, context: &mut ExecutionContext) -> Result<Value> {
        let config = &node.definition.config;
        
        // Get action type
        let action_type = config["type"].as_str()
            .ok_or_else(|| ModelSrvError::InvalidOperation("Action node missing type".to_string()))?;
            
        // Execute action based on type
        match action_type {
            "device_control" => {
                // Get device ID
                let device_id = config["device_id"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Device control action missing device_id".to_string()))?;
                    
                // Get operation
                let operation = config["operation"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Device control action missing operation".to_string()))?;
                    
                // Get parameters
                let parameters = config.get("parameters").cloned().unwrap_or(json!({}));
                
                // In a real implementation, we would send the control command to the device
                // For now, we'll just log the action and return success
                info!("Executing device control: {} on device {} with parameters {:?}", 
                      operation, device_id, parameters);
                      
                Ok(json!({
                    "action": "device_control",
                    "device_id": device_id,
                    "operation": operation,
                    "parameters": parameters,
                    "status": "success"
                }))
            },
            "notify" => {
                // Get notification target
                let target = config["target"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Notify action missing target".to_string()))?;
                    
                // Get message
                let message = config["message"].as_str()
                    .ok_or_else(|| ModelSrvError::InvalidOperation("Notify action missing message".to_string()))?;
                    
                // In a real implementation, we would send the notification
                // For now, we'll just log it and return success
                info!("Sending notification to {}: {}", target, message);
                
                Ok(json!({
                    "action": "notify",
                    "target": target,
                    "message": message,
                    "status": "success"
                }))
            },
            _ => Err(ModelSrvError::InvalidOperation(format!("Unsupported action type: {}", action_type))),
        }
    }
    
    /// Execute an aggregate node
    async fn execute_aggregate_node(
        &self, 
        node: &RuleNode, 
        context: &mut ExecutionContext,
        graph: &DiGraph<RuleNode, Option<String>>,
        visited: &HashSet<NodeIndex>,
    ) -> Result<Value> {
        let config = &node.definition.config;
        
        // Get aggregation type
        let agg_type = config["type"].as_str()
            .ok_or_else(|| ModelSrvError::InvalidOperation("Aggregate node missing type".to_string()))?;
            
        // Get node index for the current node
        // We need to find the node index in the graph by searching for matching node ID
        let node_id = &node.definition.id;
        let mut node_idx = None;
        
        // Find the node index in the graph
        for idx in graph.node_indices() {
            if &graph[idx].definition.id == node_id {
                node_idx = Some(idx);
                break;
            }
        }
        
        let node_idx = node_idx.ok_or_else(|| 
            ModelSrvError::InvalidOperation(format!("Node index not found for {}", node_id)))?;
        
        let mut inputs = Vec::new();
        for in_idx in graph.neighbors_directed(node_idx, Direction::Incoming) {
            if !visited.contains(&in_idx) {
                continue;
            }
            
            if let Some(result) = &graph[in_idx].result {
                inputs.push(result.clone());
            }
        }
        
        // Execute aggregation based on type
        match agg_type {
            "all" => {
                // Check if all inputs are true
                let all_true = inputs.iter().all(|input| {
                    if let Some(result) = input.get("result") {
                        result.as_bool().unwrap_or(false)
                    } else {
                        false
                    }
                });
                
                Ok(json!({"result": all_true}))
            },
            "any" => {
                // Check if any input is true
                let any_true = inputs.iter().any(|input| {
                    if let Some(result) = input.get("result") {
                        result.as_bool().unwrap_or(false)
                    } else {
                        false
                    }
                });
                
                Ok(json!({"result": any_true}))
            },
            "count" => {
                // Count inputs that are true
                let count = inputs.iter().filter(|input| {
                    if let Some(result) = input.get("result") {
                        result.as_bool().unwrap_or(false)
                    } else {
                        false
                    }
                }).count();
                
                Ok(json!({"result": count}))
            },
            _ => Err(ModelSrvError::InvalidOperation(format!("Unsupported aggregation type: {}", agg_type))),
        }
    }
} 