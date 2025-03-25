use crate::storage::DataStore;
use crate::error::{ModelSrvError, Result};
use crate::rules::{NodeType, NodeState, DagRule, NodeDefinition};
use crate::storage::hybrid_store::HybridStore;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde_json::{json, Value};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use log::{error, info, debug, warn, trace};
use uuid::Uuid;
use chrono::Utc;
use crate::redis_handler::RedisConnection;
use std::sync::Mutex;
use async_trait::async_trait;

/// Action handler trait for control operations
#[async_trait]
pub trait ActionHandler: Send + Sync {
    /// Get the name of this action handler
    fn name(&self) -> &str;
    
    /// Get the type of this action handler
    fn handler_type(&self) -> String;
    
    /// Check if this handler can handle the given action type
    fn can_handle(&self, action_type: &str) -> bool;
    
    /// Execute an action
    async fn execute_action(&self, action_type: &str, config: &Value) -> Result<String>;
}

/// Context for rule execution
pub struct ExecutionContext {
    /// Device status cache
    device_status: HashMap<String, Value>,
    /// Variables storage for rule execution
    variables: HashMap<String, Value>,
    /// Data store for persistence and device interaction
    store: Arc<HybridStore>,
    /// Action handlers for executing actions
    action_handlers: Vec<Box<dyn ActionHandler + Send + Sync>>,
    /// Post-processors for rule execution results
    post_processors: Vec<Box<dyn RulePostProcessor + Send + Sync>>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(store: Arc<HybridStore>) -> Self {
        Self {
            device_status: HashMap::new(),
            variables: HashMap::new(),
            store,
            action_handlers: Vec::new(),
            post_processors: Vec::new(),
        }
    }
    
    /// Register an action handler
    pub fn register_action_handler(&mut self, handler: Box<dyn ActionHandler + Send + Sync>) {
        self.action_handlers.push(handler);
    }
    
    /// Register a post-processor
    pub fn register_post_processor(&mut self, processor: Box<dyn RulePostProcessor + Send + Sync>) {
        self.post_processors.push(processor);
    }
    
    /// Find an action handler for a specific action type
    fn find_action_handler(&mut self, action_type: &str) -> Option<&mut Box<dyn ActionHandler + Send + Sync>> {
        self.action_handlers.iter_mut()
            .find(|handler| handler.can_handle(action_type))
    }
    
    /// Execute an action
    pub async fn execute_action(&mut self, action_type: &str, config: &Value) -> Result<Value> {
        // Try to find a handler for this action type
        let handler = self.find_action_handler(action_type);
        
        match handler {
            Some(handler) => {
                // Add store reference to the config if possible
                let mut config_with_store = config.clone();
                if let Value::Object(ref mut map) = config_with_store {
                    map.insert("store".to_string(), Value::Object(serde_json::Map::new()));
                }
                
                // Execute the action
                let result = handler.execute_action(action_type, &config_with_store).await
                    .map_err(|e| ModelSrvError::ActionExecutionError(
                        format!("Action '{}' execution failed: {}", action_type, e)
                    ))?;
                    
                // Convert result to Value
                Ok(Value::String(result))
            },
            None => Err(ModelSrvError::ActionTypeNotSupported(
                format!("No handler found for action type: {}", action_type)
            ))
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
        let key = format!("device:status:{}:{}", device_id, parameter);
        
        // Try from cached device status first
        let cache_key = format!("{}:{}", device_id, parameter);
        if let Some(value) = self.device_status.get(&cache_key) {
            return Ok(value.clone());
        }
        
        // Then try from store
        match self.store.as_ref().get_string(&key) {
            Ok(value_str) => {
                // Try to parse as JSON, or use raw string
                match serde_json::from_str(&value_str) {
                    Ok(value) => Ok(value),
                    Err(_) => Ok(json!(value_str)),
                }
            },
            Err(_) => Ok(json!(null)),
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
    
    /// Evaluate a simple expression
    pub fn evaluate_expression(&self, expression: &str) -> Result<bool> {
        // Very simple expression evaluator (should be replaced with a proper one)
        if expression.contains("==") {
            let parts: Vec<&str> = expression.split("==").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(ModelSrvError::RuleError(format!("Invalid expression: {}", expression)));
            }
            
            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;
            
            Ok(left == right)
        } else if expression.contains("!=") {
            let parts: Vec<&str> = expression.split("!=").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(ModelSrvError::RuleError(format!("Invalid expression: {}", expression)));
            }
            
            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;
            
            Ok(left != right)
        } else if expression.contains(">") {
            let parts: Vec<&str> = expression.split(">").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(ModelSrvError::RuleError(format!("Invalid expression: {}", expression)));
            }
            
            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;
            
            match (left.as_f64(), right.as_f64()) {
                (Some(l), Some(r)) => Ok(l > r),
                _ => Err(ModelSrvError::RuleError(format!("Cannot compare: {} and {}", left, right))),
            }
        } else if expression.contains("<") {
            let parts: Vec<&str> = expression.split("<").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(ModelSrvError::RuleError(format!("Invalid expression: {}", expression)));
            }
            
            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;
            
            match (left.as_f64(), right.as_f64()) {
                (Some(l), Some(r)) => Ok(l < r),
                _ => Err(ModelSrvError::RuleError(format!("Cannot compare: {} and {}", left, right))),
            }
        } else {
            // Assume it's a boolean variable or literal
            let value = self.resolve_variable(expression)?;
            
            if let Some(b) = value.as_bool() {
                Ok(b)
            } else if let Some(n) = value.as_f64() {
                Ok(n != 0.0)
            } else if let Some(s) = value.as_str() {
                Ok(!s.is_empty())
            } else {
                Ok(false)
            }
        }
    }
    
    /// Resolve a variable or literal
    pub fn resolve_variable(&self, name: &str) -> Result<Value> {
        // Check if it's a variable reference
        if name.starts_with("$") {
            let var_name = &name[1..];
            match self.get_variable(var_name) {
                Some(value) => Ok(value.clone()),
                None => Err(ModelSrvError::RuleError(format!("Variable not found: {}", var_name))),
            }
        } else if name.starts_with("device:") {
            // Device parameter reference: device:device_id:parameter
            let parts: Vec<&str> = name.split(':').collect();
            if parts.len() != 3 {
                return Err(ModelSrvError::RuleError(format!("Invalid device reference: {}", name)));
            }
            
            self.get_device_parameter(parts[1], parts[2])
        } else {
            // Assume it's a literal
            if name == "true" {
                Ok(json!(true))
            } else if name == "false" {
                Ok(json!(false))
            } else if let Ok(num) = name.parse::<f64>() {
                Ok(json!(num))
            } else {
                Ok(json!(name))
            }
        }
    }
    
    /// Execute device action
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
    
    /// Process the rule execution result with registered post-processors
    pub async fn process_result(&mut self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
        for processor in &mut self.post_processors {
            if let Err(e) = processor.process(rule_id, result).await {
                warn!("Post-processor {} failed: {}", processor.name(), e);
                // Continue with other processors even if one fails
            }
        }
        Ok(())
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
            if let Some(action_type) = node.definition.config.get("action_type").and_then(Value::as_str) {
                // Use ActionHandler implementation
                context.execute_action(action_type, &node.definition.config).await
            } else if let Some(control_id) = node.definition.config.get("control_id").and_then(Value::as_str) {
                // Directly execute control operation by ID
                let config = json!({
                    "action_type": "control",
                    "control_id": control_id
                });
                context.execute_action("control", &config).await
            } else if let (Some(device_id), Some(operation)) = (
                node.definition.config.get("device_id").and_then(Value::as_str),
                node.definition.config.get("operation").and_then(Value::as_str)
            ) {
                // Legacy direct device action
                let parameters = node.definition.config.get("parameters").cloned().unwrap_or(json!({}));
                
                // If we have registered handlers, try to use them first
                if !context.action_handlers.is_empty() {
                    let config = json!({
                        "device_id": device_id,
                        "point": operation,
                        "value": parameters,
                        "channel": "default_channel"
                    });
                    
                    // Try to execute via action handler
                    match context.execute_action("device_control", &config).await {
                        Ok(result) => Ok(result),
                        Err(e) => {
                            // Fall back to legacy implementation if handler doesn't work
                            warn!("Action handler failed: {}, falling back to legacy implementation", e);
                            context.execute_device_action(device_id, operation, &parameters).await
                        }
                    }
                } else {
                    // Use legacy implementation directly
                    context.execute_device_action(device_id, operation, &parameters).await
                }
            } else {
                Err(ModelSrvError::RuleError("Invalid action node configuration".to_string()))
            }
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

/// Result of a rule execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionResult {
    /// Rule ID
    pub rule_id: String,
    /// Execution ID
    pub execution_id: String,
    /// Execution timestamp
    pub timestamp: String,
    /// Execution duration in milliseconds
    pub duration_ms: u128,
    /// Execution status
    pub status: String,
    /// Output value
    pub output: Value,
    /// Input data
    pub input: Option<Value>,
    /// Error message if execution failed
    pub error: Option<String>,
}

/// Post-processor for rule execution results
#[async_trait::async_trait]
pub trait RulePostProcessor {
    /// Process a rule execution result
    async fn process(&mut self, rule_id: &str, result: &RuleExecutionResult) -> Result<()>;
    
    /// Get a descriptive name for this post-processor
    fn name(&self) -> &str;
}

/// Logger post-processor for rule execution results
pub struct LoggingPostProcessor;

impl LoggingPostProcessor {
    /// Create a new logging post-processor
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RulePostProcessor for LoggingPostProcessor {
    async fn process(&mut self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
        match result.status.as_str() {
            "completed" => {
                info!("Rule '{}' execution completed successfully in {} ms", 
                      rule_id, result.duration_ms);
            },
            "failed" => {
                error!("Rule '{}' execution failed in {} ms: {}", 
                       rule_id, result.duration_ms, 
                       result.error.as_deref().unwrap_or("Unknown error"));
            },
            _ => {
                warn!("Rule '{}' execution has unknown status: {}", 
                      rule_id, result.status);
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "LoggingPostProcessor"
    }
}

/// Notification post-processor for rule execution results
pub struct NotificationPostProcessor {
    /// Notification threshold in milliseconds
    threshold_ms: u128,
    /// Redis key prefix
    key_prefix: String,
    /// Redis connection
    redis: Mutex<Option<RedisConnection>>,
}

impl NotificationPostProcessor {
    /// Create a new notification post-processor
    pub fn new(threshold_ms: u128, key_prefix: &str) -> Self {
        Self {
            threshold_ms,
            key_prefix: key_prefix.to_string(),
            redis: Mutex::new(None),
        }
    }
    
    /// Initialize Redis connection
    pub fn init(&mut self, redis_url: &str) -> Result<()> {
        let mut conn = RedisConnection::new();
        conn.connect(redis_url)?;
        
        let mut redis_guard = self.redis.lock().map_err(|_| ModelSrvError::LockError)?;
        *redis_guard = Some(conn);
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl RulePostProcessor for NotificationPostProcessor {
    async fn process(&mut self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
        // Only send notifications for slow rule executions or failures
        if result.duration_ms > self.threshold_ms || result.status == "failed" {
            let notification = json!({
                "rule_id": rule_id,
                "timestamp": result.timestamp,
                "duration_ms": result.duration_ms,
                "status": result.status,
                "error": result.error,
                "level": if result.status == "failed" { "error" } else { "warning" },
                "message": if result.status == "failed" {
                    format!("Rule '{}' execution failed: {}", 
                            rule_id, result.error.as_deref().unwrap_or("Unknown error"))
                } else {
                    format!("Rule '{}' execution took too long: {} ms", rule_id, result.duration_ms)
                }
            });
            
            // Try to send notification via Redis pub/sub
            let redis_guard = self.redis.lock().map_err(|_| ModelSrvError::LockError)?;
            if let Some(mut conn) = redis_guard.clone() {
                let channel = format!("{}notifications", self.key_prefix);
                let _: () = redis::cmd("PUBLISH")
                    .arg(channel)
                    .arg(notification.to_string())
                    .query(&mut conn)
                    .map_err(|e| ModelSrvError::RedisError(e.to_string()))?;
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "NotificationPostProcessor"
    }
}

/// Execute rules with monitoring and metrics
pub struct RuleExecutor {
    store: Arc<HybridStore>,
    /// Action handler registry
    action_handlers: Arc<Mutex<Vec<Box<dyn ActionHandler + Send + Sync>>>>,
    /// Post-processor registry
    post_processors: Arc<Mutex<Vec<Box<dyn RulePostProcessor + Send + Sync>>>>,
}

impl RuleExecutor {
    /// Create a new rule executor
    pub fn new(store: Arc<HybridStore>) -> Self {
        // Initialize with default post-processors
        let mut post_processors = Vec::new();
        post_processors.push(Box::new(LoggingPostProcessor::new()) as Box<dyn RulePostProcessor + Send + Sync>);
        
        Self {
            store,
            action_handlers: Arc::new(Mutex::new(Vec::new())),
            post_processors: Arc::new(Mutex::new(post_processors)),
        }
    }
    
    /// Register an action handler
    pub fn register_action_handler<T: ActionHandler + Send + Sync + 'static>(&self, handler: T) -> Result<()> {
        let mut handlers = self.action_handlers.lock().map_err(|_| ModelSrvError::LockError)?;
        handlers.push(Box::new(handler));
        
        debug!("Registered action handler: {}", handlers.last().unwrap().name());
        Ok(())
    }
    
    /// Register a post-processor
    pub fn register_post_processor<T: RulePostProcessor + Send + Sync + 'static>(&self, processor: T) -> Result<()> {
        let mut processors = self.post_processors.lock().map_err(|_| ModelSrvError::LockError)?;
        processors.push(Box::new(processor));
        
        debug!("Registered post-processor: {}", processors.last().unwrap().name());
        Ok(())
    }
    
    /// Execute a rule
    pub async fn execute_rule(&self, rule_id: &str, input_data: Option<Value>) -> Result<Value> {
        // Create execution context and register action handlers
        let mut context = ExecutionContext::new(self.store.clone());
        
        // Register action handlers
        {
            let handlers = self.action_handlers.lock().map_err(|_| ModelSrvError::LockError)?;
            for handler in handlers.iter() {
                let handler_type_name = std::any::type_name::<Box<dyn ActionHandler + Send + Sync>>();
                let handler_name = handler.name();
                trace!("Registering handler {} of type {}", handler_name, handler_type_name);
                
                // Unfortunately, we can't directly clone the Box<dyn Trait> here
                // So we need to pass a trait object reference to the context
                // This is not ideal, but it's the best we can do without a Clone trait bound
                
                // Since we can't clone a trait object directly, we'll have to add handlers during execution
                // This approach would need to be refined in a real implementation
                // For now, we'll create a mock or placeholder handler
                // TODO: Find a better way to register handlers from the registry to the context
            }
        }
        
        // Register post-processors
        {
            let processors = self.post_processors.lock().map_err(|_| ModelSrvError::LockError)?;
            for _processor in processors.iter() {
                // Similar issue as with action handlers
                // We'll create placeholder processors for now
                // TODO: Find a better way to register processors from the registry to the context
            }
        }
        
        // Try loading rule directly from store first
        let rule_key = format!("rule:{}", rule_id);
        let rule_def = match self.store.get_string(&rule_key) {
            Ok(rule_json) => {
                match serde_json::from_str::<DagRule>(&rule_json) {
                    Ok(rule) => rule,
                    Err(e) => {
                        return Err(ModelSrvError::RuleParsingError(format!(
                            "Failed to parse rule {}: {}", rule_id, e
                        )));
                    }
                }
            },
            Err(e) => {
                return Err(ModelSrvError::RuleNotFound(format!(
                    "Rule {} not found: {}", rule_id, e
                )));
            }
        };
        
        // Check if rule is enabled
        if !rule_def.enabled {
            return Err(ModelSrvError::RuleDisabled(format!(
                "Rule {} is disabled", rule_id
            )));
        }
        
        // Build rule graph
        let mut runtime_rule = build_rule_graph(rule_def)?;
        
        // Record start time
        let start_time = std::time::Instant::now();
        
        // Load device status
        context.load_device_status().await?;
        
        // Set input data as variables
        if let Some(input) = &input_data {
            if let Value::Object(map) = input {
                for (key, value) in map {
                    context.set_variable(key, value.clone());
                }
            }
        }
        
        // Execute rule
        let result = execute_rule_graph(&mut runtime_rule, &mut context).await;
        
        // Calculate execution time
        let execution_time = start_time.elapsed();
        
        // Record execution result
        let execution_id = Uuid::new_v4().to_string();
        let execution_result = RuleExecutionResult {
            rule_id: rule_id.to_string(),
            execution_id: execution_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
            duration_ms: execution_time.as_millis(),
            status: if result.is_ok() { "completed".to_string() } else { "failed".to_string() },
            output: match &result {
                Ok(output) => output.clone(),
                Err(e) => json!({ "error": e.to_string() }),
            },
            input: input_data,
            error: if let Err(e) = &result {
                Some(e.to_string())
            } else {
                None
            },
        };
        
        // Apply post-processors
        context.process_result(rule_id, &execution_result).await?;
        
        // Record execution result
        let record = json!({
            "execution_id": execution_id,
            "rule_id": rule_id,
            "timestamp": Utc::now().to_rfc3339(),
            "duration_ms": execution_time.as_millis(),
            "status": if result.is_ok() { "completed" } else { "failed" },
            "output": match &result {
                Ok(output) => output.clone(),
                Err(e) => json!({ "error": e.to_string() }),
            }
        });
        
        // Store execution result in Redis
        let record_key = format!("ems:rule:execution:{}", execution_id);
        if let Err(e) = self.store.set_string(&record_key, &record.to_string()) {
            error!("Failed to store execution record: {}", e);
        }
        
        // Store in rule history
        let history_key = format!("ems:rule:history:{}", rule_id);
        if let Some(mut redis) = self.store.get_redis_connection() {
            let _: Result<()> = redis::cmd("LPUSH")
                .arg(&history_key)
                .arg(&execution_id)
                .query(&mut redis)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()));
                
            let _: Result<()> = redis::cmd("LTRIM")
                .arg(&history_key)
                .arg(0)
                .arg(99) // Keep last 100 executions
                .query(&mut redis)
                .map_err(|e| ModelSrvError::RedisError(e.to_string()));
        }
        
        // Return execution result
        match result {
            Ok(_output) => Ok(json!({
                "status": "success",
                "result": {
                    "execution_id": execution_id,
                    "rule_id": rule_id,
                    "timestamp": record["timestamp"],
                    "duration_ms": record["duration_ms"],
                    "status": "completed",
                    "output": "Rule executed successfully"
                }
            })),
            Err(e) => {
                error!("Rule execution failed: {}", e);
                Ok(json!({
                    "status": "error",
                    "error": e.to_string(),
                    "result": {
                        "execution_id": execution_id,
                        "rule_id": rule_id,
                        "timestamp": record["timestamp"],
                        "duration_ms": record["duration_ms"],
                        "status": "failed",
                        "output": format!("Rule execution failed: {}", e)
                    }
                }))
            }
        }
    }
} 