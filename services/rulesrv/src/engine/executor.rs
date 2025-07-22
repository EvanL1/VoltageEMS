use crate::error::{Result, RulesrvError};
use crate::redis::RedisStore;
use crate::rules::{DagRule, NodeDefinition, NodeState, NodeType};
use async_trait::async_trait;
use chrono::Utc;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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
    store: Arc<RedisStore>,
    /// Action handlers for executing actions
    action_handlers: Vec<Arc<dyn ActionHandler + Send + Sync>>,
    /// Post-processors for rule execution results
    post_processors: Vec<Arc<dyn RulePostProcessor + Send + Sync>>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(store: Arc<RedisStore>) -> Self {
        Self {
            device_status: HashMap::new(),
            variables: HashMap::new(),
            store,
            action_handlers: Vec::new(),
            post_processors: Vec::new(),
        }
    }

    /// Register an action handler
    pub fn register_action_handler(&mut self, handler: Arc<dyn ActionHandler + Send + Sync>) {
        self.action_handlers.push(handler);
    }

    /// Register a post-processor
    pub fn register_post_processor(&mut self, processor: Arc<dyn RulePostProcessor + Send + Sync>) {
        self.post_processors.push(processor);
    }

    /// Find an action handler for a specific action type
    fn find_action_handler(
        &self,
        action_type: &str,
    ) -> Option<&Arc<dyn ActionHandler + Send + Sync>> {
        self.action_handlers
            .iter()
            .find(|handler| handler.can_handle(action_type))
    }

    /// Execute an action
    pub async fn execute_action(&self, action_type: &str, config: &Value) -> Result<Value> {
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
                let result = handler
                    .execute_action(action_type, &config_with_store)
                    .await
                    .map_err(|e| {
                        RulesrvError::ActionExecutionError(format!(
                            "Action '{}' execution failed: {}",
                            action_type, e
                        ))
                    })?;

                // Convert result to Value
                Ok(Value::String(result))
            }
            None => Err(RulesrvError::ActionTypeNotSupported(format!(
                "No handler found for action type: {}",
                action_type
            ))),
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

    /// Resolve a variable or literal with external variables
    pub fn resolve_variable_with_vars(
        &self,
        name: &str,
        variables: &HashMap<String, Value>,
    ) -> Result<Value> {
        // Check if it's a variable reference
        if name.starts_with("$") {
            let var_name = &name[1..];
            // First check external variables, then internal
            match variables
                .get(var_name)
                .or_else(|| self.variables.get(var_name))
            {
                Some(value) => Ok(value.clone()),
                None => Err(RulesrvError::RuleError(format!(
                    "Variable not found: {}",
                    var_name
                ))),
            }
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

    /// Evaluate a simple expression
    pub fn evaluate_expression(&self, expression: &str) -> Result<bool> {
        // Very simple expression evaluator (should be replaced with a proper one)
        if expression.contains("==") {
            let parts: Vec<&str> = expression.split("==").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(RulesrvError::RuleError(format!(
                    "Invalid expression: {}",
                    expression
                )));
            }

            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;

            Ok(left == right)
        } else if expression.contains("!=") {
            let parts: Vec<&str> = expression.split("!=").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(RulesrvError::RuleError(format!(
                    "Invalid expression: {}",
                    expression
                )));
            }

            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;

            Ok(left != right)
        } else if expression.contains(">") {
            let parts: Vec<&str> = expression.split(">").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(RulesrvError::RuleError(format!(
                    "Invalid expression: {}",
                    expression
                )));
            }

            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;

            match (left.as_f64(), right.as_f64()) {
                (Some(l), Some(r)) => Ok(l > r),
                _ => Err(RulesrvError::RuleError(format!(
                    "Cannot compare: {} and {}",
                    left, right
                ))),
            }
        } else if expression.contains("<") {
            let parts: Vec<&str> = expression.split("<").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(RulesrvError::RuleError(format!(
                    "Invalid expression: {}",
                    expression
                )));
            }

            let left = self.resolve_variable(parts[0])?;
            let right = self.resolve_variable(parts[1])?;

            match (left.as_f64(), right.as_f64()) {
                (Some(l), Some(r)) => Ok(l < r),
                _ => Err(RulesrvError::RuleError(format!(
                    "Cannot compare: {} and {}",
                    left, right
                ))),
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
                None => Err(RulesrvError::RuleError(format!(
                    "Variable not found: {}",
                    var_name
                ))),
            }
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
    pub async fn execute_device_action(
        &self,
        device_id: &str,
        operation: &str,
        parameters: &Value,
    ) -> Result<Value> {
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
        self.store
            .as_ref()
            .set_string(&cmd_key, &command.to_string())
            .await?;

        // TODO: Implement queue operations when RedisStore supports them
        // Currently RedisStore doesn't expose RPUSH operations
        // Would need to add rpush method to RedisStore

        Ok(json!({
            "command_id": cmd_id,
            "status": "queued"
        }))
    }

    /// Process the rule execution result with registered post-processors
    pub async fn process_result(&self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
        for processor in &self.post_processors {
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

/// Evaluate an expression with external variables
fn evaluate_expression_with_vars(
    expression: &str,
    context: &ExecutionContext,
    variables: &HashMap<String, Value>,
) -> Result<bool> {
    // Very simple expression evaluator (should be replaced with a proper one)
    if expression.contains("==") {
        let parts: Vec<&str> = expression.split("==").map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(RulesrvError::RuleError(format!(
                "Invalid expression: {}",
                expression
            )));
        }

        let left = context.resolve_variable_with_vars(parts[0], variables)?;
        let right = context.resolve_variable_with_vars(parts[1], variables)?;

        Ok(left == right)
    } else if expression.contains("!=") {
        let parts: Vec<&str> = expression.split("!=").map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(RulesrvError::RuleError(format!(
                "Invalid expression: {}",
                expression
            )));
        }

        let left = context.resolve_variable_with_vars(parts[0], variables)?;
        let right = context.resolve_variable_with_vars(parts[1], variables)?;

        Ok(left != right)
    } else if expression.contains(">") {
        let parts: Vec<&str> = expression.split(">").map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(RulesrvError::RuleError(format!(
                "Invalid expression: {}",
                expression
            )));
        }

        let left = context.resolve_variable_with_vars(parts[0], variables)?;
        let right = context.resolve_variable_with_vars(parts[1], variables)?;

        match (left.as_f64(), right.as_f64()) {
            (Some(l), Some(r)) => Ok(l > r),
            _ => Err(RulesrvError::RuleError(format!(
                "Cannot compare: {} and {}",
                left, right
            ))),
        }
    } else if expression.contains("<") {
        let parts: Vec<&str> = expression.split("<").map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(RulesrvError::RuleError(format!(
                "Invalid expression: {}",
                expression
            )));
        }

        let left = context.resolve_variable_with_vars(parts[0], variables)?;
        let right = context.resolve_variable_with_vars(parts[1], variables)?;

        match (left.as_f64(), right.as_f64()) {
            (Some(l), Some(r)) => Ok(l < r),
            _ => Err(RulesrvError::RuleError(format!(
                "Cannot compare: {} and {}",
                left, right
            ))),
        }
    } else {
        // Assume it's a boolean variable or literal
        let value = context.resolve_variable_with_vars(expression, variables)?;

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
        let from_idx = node_map
            .get(&edge_def.from)
            .ok_or_else(|| RulesrvError::RuleError(format!("Node not found: {}", edge_def.from)))?;

        let to_idx = node_map
            .get(&edge_def.to)
            .ok_or_else(|| RulesrvError::RuleError(format!("Node not found: {}", edge_def.to)))?;

        graph.add_edge(*from_idx, *to_idx, edge_def.condition.clone());
    }

    // Check if graph is acyclic
    if toposort(&graph, None).is_err() {
        return Err(RulesrvError::RuleError(
            "Cycle detected in rule graph".to_string(),
        ));
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
    context: &ExecutionContext,
    variables: &mut HashMap<String, Value>,
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
        let has_incoming = rule
            .graph
            .edges_directed(node_idx, Direction::Incoming)
            .count()
            > 0;
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
            match execute_node(node, context, variables).await {
                Ok(result) => {
                    node.result = Some(result);
                    node.state = NodeState::Completed;
                    completed.insert(node_idx);

                    // Store result in variables using node ID as variable name
                    if let Some(result) = &node.result {
                        variables.insert(node.definition.id.clone(), result.clone());
                    }
                }
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
                    match evaluate_expression_with_vars(condition, context, variables) {
                        Ok(result) => {
                            if !result {
                                can_execute = false;
                                break;
                            }
                        }
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
async fn execute_node(
    node: &RuleNode,
    context: &ExecutionContext,
    variables: &HashMap<String, Value>,
) -> Result<Value> {
    match node.definition.node_type {
        NodeType::Input => {
            // Process input node
            // For now, input nodes should reference modsrv outputs or point data
            // Example: modsrv:model1:output or channel_id:type:point_id
            let source = node
                .definition
                .config
                .get("source")
                .and_then(Value::as_str)
                .ok_or_else(|| RulesrvError::RuleError("Input node missing source".to_string()))?;

            // Get value from Redis based on source format
            match context.store.get_string(source).await {
                Ok(Some(value_str)) => {
                    // Try to parse as JSON, or use raw string
                    match serde_json::from_str(&value_str) {
                        Ok(value) => Ok(value),
                        Err(_) => {
                            // Check if it's a point value format (value:timestamp)
                            if let Some(val_str) = value_str.split(':').next() {
                                if let Ok(val) = val_str.parse::<f64>() {
                                    Ok(json!(val))
                                } else {
                                    Ok(json!(value_str))
                                }
                            } else {
                                Ok(json!(value_str))
                            }
                        }
                    }
                }
                Ok(None) => {
                    warn!("Input source {} not found", source);
                    Ok(json!(null))
                }
                Err(_) => Ok(json!(null)),
            }
        }
        NodeType::Condition => {
            // Process condition node
            let expression = node
                .definition
                .config
                .get("expression")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RulesrvError::RuleError("Condition node missing expression".to_string())
                })?;

            let result = evaluate_expression_with_vars(expression, context, variables)?;
            Ok(json!(result))
        }
        NodeType::Transform => {
            // Process transform node
            let transform_type = node
                .definition
                .config
                .get("transform_type")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RulesrvError::RuleError("Transform node missing transform_type".to_string())
                })?;

            let input = node
                .definition
                .config
                .get("input")
                .cloned()
                .unwrap_or(json!({}));

            match transform_type {
                "scale" => {
                    let value_expr =
                        input
                            .get("value_expr")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                RulesrvError::RuleError(
                                    "Scale transform missing value_expr".to_string(),
                                )
                            })?;

                    let factor = input.get("factor").and_then(Value::as_f64).unwrap_or(1.0);

                    let value = context.resolve_variable_with_vars(value_expr, variables)?;

                    if let Some(num) = value.as_f64() {
                        Ok(json!(num * factor))
                    } else {
                        Err(RulesrvError::RuleError(format!(
                            "Value is not a number: {}",
                            value
                        )))
                    }
                }
                "threshold" => {
                    let value_expr =
                        input
                            .get("value_expr")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                RulesrvError::RuleError(
                                    "Threshold transform missing value_expr".to_string(),
                                )
                            })?;

                    let threshold =
                        input
                            .get("threshold")
                            .and_then(Value::as_f64)
                            .ok_or_else(|| {
                                RulesrvError::RuleError(
                                    "Threshold transform missing threshold".to_string(),
                                )
                            })?;

                    let value = context.resolve_variable_with_vars(value_expr, variables)?;

                    if let Some(num) = value.as_f64() {
                        Ok(json!(num >= threshold))
                    } else {
                        Err(RulesrvError::RuleError(format!(
                            "Value is not a number: {}",
                            value
                        )))
                    }
                }
                _ => Err(RulesrvError::RuleError(format!(
                    "Unsupported transform type: {}",
                    transform_type
                ))),
            }
        }
        NodeType::Action => {
            // Process action node
            if let Some(action_type) = node
                .definition
                .config
                .get("action_type")
                .and_then(Value::as_str)
            {
                // Use ActionHandler implementation
                context
                    .execute_action(action_type, &node.definition.config)
                    .await
            } else if let Some(control_id) = node
                .definition
                .config
                .get("control_id")
                .and_then(Value::as_str)
            {
                // Directly execute control operation by ID
                let config = json!({
                    "action_type": "control",
                    "control_id": control_id
                });
                context.execute_action("control", &config).await
            } else if let (Some(device_id), Some(operation)) = (
                node.definition
                    .config
                    .get("device_id")
                    .and_then(Value::as_str),
                node.definition
                    .config
                    .get("operation")
                    .and_then(Value::as_str),
            ) {
                // Legacy direct device action
                let parameters = node
                    .definition
                    .config
                    .get("parameters")
                    .cloned()
                    .unwrap_or(json!({}));

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
                            warn!(
                                "Action handler failed: {}, falling back to legacy implementation",
                                e
                            );
                            context
                                .execute_device_action(device_id, operation, &parameters)
                                .await
                        }
                    }
                } else {
                    // Use legacy implementation directly
                    context
                        .execute_device_action(device_id, operation, &parameters)
                        .await
                }
            } else {
                Err(RulesrvError::RuleError(
                    "Invalid action node configuration".to_string(),
                ))
            }
        }
        NodeType::Aggregate => {
            // Process aggregate node
            let aggregation_type = node
                .definition
                .config
                .get("aggregation_type")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RulesrvError::RuleError("Aggregate node missing aggregation_type".to_string())
                })?;

            let inputs = node
                .definition
                .config
                .get("inputs")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    RulesrvError::RuleError("Aggregate node missing inputs".to_string())
                })?;

            let mut values = Vec::new();

            for input in inputs {
                if let Some(var_name) = input.as_str() {
                    if let Some(value) = variables.get(var_name) {
                        values.push(value.clone());
                    }
                }
            }

            match aggregation_type {
                "and" => {
                    let result = values.iter().all(|v| v.as_bool().unwrap_or(false));
                    Ok(json!(result))
                }
                "or" => {
                    let result = values.iter().any(|v| v.as_bool().unwrap_or(false));
                    Ok(json!(result))
                }
                "sum" => {
                    let sum = values.iter().filter_map(|v| v.as_f64()).sum::<f64>();
                    Ok(json!(sum))
                }
                "avg" => {
                    let nums: Vec<f64> = values.iter().filter_map(|v| v.as_f64()).collect();

                    if nums.is_empty() {
                        Ok(json!(0.0))
                    } else {
                        let avg = nums.iter().sum::<f64>() / nums.len() as f64;
                        Ok(json!(avg))
                    }
                }
                _ => Err(RulesrvError::RuleError(format!(
                    "Unsupported aggregation type: {}",
                    aggregation_type
                ))),
            }
        }
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
pub trait RulePostProcessor: Send + Sync {
    /// Process a rule execution result
    async fn process(&self, rule_id: &str, result: &RuleExecutionResult) -> Result<()>;

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
    async fn process(&self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
        match result.status.as_str() {
            "completed" => {
                info!(
                    "Rule '{}' execution completed successfully in {} ms",
                    rule_id, result.duration_ms
                );
            }
            "failed" => {
                error!(
                    "Rule '{}' execution failed in {} ms: {}",
                    rule_id,
                    result.duration_ms,
                    result.error.as_deref().unwrap_or("Unknown error")
                );
            }
            _ => {
                warn!(
                    "Rule '{}' execution has unknown status: {}",
                    rule_id, result.status
                );
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
    // /// Redis connection
    // redis: Mutex<Option<RedisConnection>>,
}

impl NotificationPostProcessor {
    /// Create a new notification post-processor
    pub fn new(threshold_ms: u128, key_prefix: &str) -> Self {
        Self {
            threshold_ms,
            key_prefix: key_prefix.to_string(),
            // redis: Mutex::new(None),
        }
    }

    // /// Initialize Redis connection
    // pub fn init(&mut self, redis_url: &str) -> Result<()> {
    //     let mut conn = RedisConnection::new();
    //     conn.connect(redis_url)?;

    //     let mut redis_guard = self.redis.lock().map_err(|_| RulesrvError::LockError)?;
    //     *redis_guard = Some(conn);

    //     Ok(())
    // }
}

#[async_trait::async_trait]
impl RulePostProcessor for NotificationPostProcessor {
    async fn process(&self, rule_id: &str, result: &RuleExecutionResult) -> Result<()> {
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

            // // Try to send notification via Redis pub/sub
            // let redis_guard = self.redis.lock().map_err(|_| RulesrvError::LockError)?;
            // if let Some(mut conn) = redis_guard.clone() {
            //     let channel = format!("{}notifications", self.key_prefix);
            //     conn.publish(&channel, &notification.to_string())?;
            // }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "NotificationPostProcessor"
    }
}

/// Execute rules with monitoring and metrics
pub struct RuleExecutor {
    store: Arc<RedisStore>,
    /// Action handler registry
    action_handlers: Arc<RwLock<Vec<Arc<dyn ActionHandler + Send + Sync>>>>,
    /// Post-processor registry
    post_processors: Arc<RwLock<Vec<Arc<dyn RulePostProcessor + Send + Sync>>>>,
}

impl RuleExecutor {
    /// Create a new rule executor
    pub fn new(store: Arc<RedisStore>) -> Self {
        // Initialize with default post-processors
        let mut post_processors = Vec::new();
        post_processors.push(Arc::new(LoggingPostProcessor::new()) as Arc<dyn RulePostProcessor + Send + Sync>);

        Self {
            store,
            action_handlers: Arc::new(RwLock::new(Vec::new())),
            post_processors: Arc::new(RwLock::new(post_processors)),
        }
    }

    /// Register an action handler
    pub async fn register_action_handler<T: ActionHandler + Send + Sync + 'static>(
        &self,
        handler: T,
    ) -> Result<()> {
        let handler_name = handler.name().to_string();
        let mut handlers = self.action_handlers.write().await;
        handlers.push(Arc::new(handler));

        debug!("Registered action handler: {}", handler_name);
        Ok(())
    }

    /// Register a post-processor
    pub async fn register_post_processor<T: RulePostProcessor + Send + Sync + 'static>(
        &self,
        processor: T,
    ) -> Result<()> {
        let processor_name = processor.name().to_string();
        let mut processors = self.post_processors.write().await;
        processors.push(Arc::new(processor));

        debug!("Registered post-processor: {}", processor_name);
        Ok(())
    }

    /// Execute a rule
    pub async fn execute_rule(&self, rule_id: &str, input_data: Option<Value>) -> Result<Value> {
        // Create execution context and register action handlers
        let mut context = ExecutionContext::new(self.store.clone());

        // Register action handlers
        {
            let handlers = self.action_handlers.read().await;
            for handler in handlers.iter() {
                context.register_action_handler(Arc::clone(handler));
            }
        }

        // Register post-processors
        {
            let processors = self.post_processors.read().await;
            for processor in processors.iter() {
                context.register_post_processor(Arc::clone(processor));
            }
        }

        // Try loading rule directly from store first
        let rule_key = format!("rule:{}", rule_id);
        let rule_def = match self.store.get_string(&rule_key).await {
            Ok(Some(rule_json)) => match serde_json::from_str::<DagRule>(&rule_json) {
                Ok(rule) => rule,
                Err(e) => {
                    return Err(RulesrvError::RuleParsingError(format!(
                        "Failed to parse rule {}: {}",
                        rule_id, e
                    )));
                }
            },
            Ok(None) => {
                return Err(RulesrvError::RuleNotFound(format!(
                    "Rule {} not found in store",
                    rule_id
                )));
            }
            Err(e) => {
                return Err(RulesrvError::RuleNotFound(format!(
                    "Rule {} not found: {}",
                    rule_id, e
                )));
            }
        };

        // Check if rule is enabled
        if !rule_def.enabled {
            return Err(RulesrvError::RuleDisabled(format!(
                "Rule {} is disabled",
                rule_id
            )));
        }

        // Build rule graph
        let mut runtime_rule = build_rule_graph(rule_def)?;

        // Record start time
        let start_time = std::time::Instant::now();

        // Load device status

        // Create variables map and set input data
        let mut variables = HashMap::new();
        if let Some(input) = &input_data {
            if let Value::Object(map) = input {
                for (key, value) in map {
                    variables.insert(key.clone(), value.clone());
                }
            }
        }

        // Execute rule
        let result = execute_rule_graph(&mut runtime_rule, &context, &mut variables).await;

        // Calculate execution time
        let execution_time = start_time.elapsed();

        // Record execution result
        let execution_id = Uuid::new_v4().to_string();
        let execution_result = RuleExecutionResult {
            rule_id: rule_id.to_string(),
            execution_id: execution_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
            duration_ms: execution_time.as_millis(),
            status: if result.is_ok() {
                "completed".to_string()
            } else {
                "failed".to_string()
            },
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
        if let Err(e) = self
            .store
            .set_string(&record_key, &record.to_string())
            .await
        {
            error!("Failed to store execution record: {}", e);
        }

        // Store in rule history
        // TODO: Implement list operations when RedisStore supports them
        // Currently RedisStore doesn't expose LPUSH/LTRIM operations
        // let history_key = format!("ems:rule:history:{}", rule_id);
        // Would need to implement list operations in RedisStore

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

    /// List all available rules
    pub async fn list_rules(&self) -> Result<Vec<crate::rules::Rule>> {
        self.store
            .list_rules()
            .await
            .map_err(|e| RulesrvError::RedisError(e.to_string()))
    }

    /// Execute a simple rule (not DAG)
    pub async fn execute_simple_rule(
        &self,
        rule: &crate::rules::Rule,
        context: &Value,
    ) -> Result<bool> {
        // Evaluate condition
        let condition_result = self.evaluate_condition(&rule.condition, context)?;

        if condition_result {
            info!("Rule '{}' condition met, executing actions", rule.name);

            // Execute actions
            for action in &rule.actions {
                if let Some(action_type) = action.get("type").and_then(|v| v.as_str()) {
                    match action_type {
                        "publish" => {
                            if let (Some(channel), Some(message)) = (
                                action.get("channel").and_then(|v| v.as_str()),
                                action.get("message").and_then(|v| v.as_str()),
                            ) {
                                // Publish to Redis channel
                                match self.store.publish(channel, message).await {
                                    Ok(_) => {
                                        info!("Published to channel '{}': {}", channel, message);
                                    }
                                    Err(e) => {
                                        error!("Failed to publish to channel '{}': {}", channel, e);
                                    }
                                }
                            }
                        }
                        _ => {
                            debug!("Unknown action type: {}", action_type);
                        }
                    }
                }
            }
        }

        Ok(condition_result)
    }

    /// Evaluate a simple condition expression
    fn evaluate_condition(&self, condition: &str, context: &Value) -> Result<bool> {
        // Simple condition evaluation for "variable > value" format
        // This is a basic implementation - can be extended for more complex conditions

        // Try to parse "variable operator value" format
        let parts: Vec<&str> = condition.split_whitespace().collect();
        if parts.len() == 3 {
            let var_name = parts[0];
            let operator = parts[1];
            let value_str = parts[2];

            // Get variable value from context
            let var_value = if let Some(val) = context.get(var_name) {
                val
            } else if var_name == "temperature" {
                // Special handling for temperature - check in value field
                if let Some(val) = context.get("value").and_then(|v| v.get("temperature")) {
                    val
                } else {
                    return Ok(false); // Variable not found in context
                }
            } else {
                return Ok(false); // Variable not found
            };

            // Parse comparison value
            let comp_value: f64 = value_str.parse().map_err(|_| {
                RulesrvError::ConditionError(format!("Invalid number: {}", value_str))
            })?;

            // Get numeric value
            let num_value = if let Some(n) = var_value.as_f64() {
                n
            } else if let Some(n) = var_value.as_i64() {
                n as f64
            } else {
                return Ok(false); // Not a numeric value
            };

            // Evaluate operator
            match operator {
                ">" => Ok(num_value > comp_value),
                "<" => Ok(num_value < comp_value),
                ">=" => Ok(num_value >= comp_value),
                "<=" => Ok(num_value <= comp_value),
                "==" => Ok((num_value - comp_value).abs() < f64::EPSILON),
                "!=" => Ok((num_value - comp_value).abs() >= f64::EPSILON),
                _ => Err(RulesrvError::InvalidOperator(operator.to_string())),
            }
        } else {
            Err(RulesrvError::ConditionError(format!(
                "Invalid condition format: {}",
                condition
            )))
        }
    }
}
