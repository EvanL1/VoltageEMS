//! Rule Executor - Execute Vue Flow rules
//!
//! Executes parsed rules by:
//! 1. Reading all variable values from RTDB
//! 2. Calculating combined (formula) variables
//! 3. Traversing nodes from start to end
//! 4. Evaluating switch conditions and executing actions

use crate::error::{Result, RuleError};
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::rulesrv::{
    ArithmeticOp, ChannelPointType, CompareOp, FlowNode, FormulaToken, InstancePointType, LogicOp,
    Rule, RuleCondition, SwitchRule, ValueChange, Variable,
};
use voltage_config::{KeySpaceConfig, RoutingCache};
use voltage_routing::set_action_point;
use voltage_rtdb::traits::Rtdb;

/// Result of executing a rule
#[derive(Debug, Clone)]
pub struct RuleExecutionResult {
    pub rule_id: String,
    pub success: bool,
    pub actions_executed: Vec<ActionResult>,
    pub error: Option<String>,
    pub execution_path: Vec<String>, // Node IDs visited
}

/// Record of an executed action
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Target type: "instance" or "channel"
    pub target_type: String,
    /// Target ID (instance_id or channel_id)
    pub target_id: u16,
    /// Point type (M/A for instance, T/S/C/A for channel)
    pub point_type: String,
    /// Point ID
    pub point_id: String,
    /// Value written
    pub value: String,
    /// Whether the action succeeded
    pub success: bool,
}

/// Rule executor
pub struct RuleExecutor<R: Rtdb + ?Sized> {
    rtdb: Arc<R>,
    routing_cache: Arc<RoutingCache>,
}

impl<R: Rtdb + ?Sized> RuleExecutor<R> {
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<RoutingCache>) -> Self {
        Self {
            rtdb,
            routing_cache,
        }
    }

    /// Execute a rule
    pub async fn execute(&self, rule: &Rule) -> Result<RuleExecutionResult> {
        let mut result = RuleExecutionResult {
            rule_id: rule.id.clone(),
            success: false,
            actions_executed: vec![],
            error: None,
            execution_path: vec![],
        };

        // 1. Read all variable values
        let mut values = match self.read_variables(&rule.variables).await {
            Ok(v) => v,
            Err(e) => {
                result.error = Some(format!("Failed to read variables: {}", e));
                return Ok(result);
            },
        };

        // 2. Calculate combined variables
        if let Err(e) = self.calculate_combined_variables(&rule.variables, &mut values) {
            result.error = Some(format!("Failed to calculate variables: {}", e));
            return Ok(result);
        }

        // 3. Build node lookup map
        let node_map: HashMap<&str, &FlowNode> =
            rule.nodes.iter().map(|n| (node_id(n), n)).collect();

        // 4. Execute from start node
        let mut current_id = rule.start_node_id.as_str();
        let max_iterations = 100; // Prevent infinite loops
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > max_iterations {
                result.error = Some("Execution exceeded maximum iterations".to_string());
                return Ok(result);
            }

            result.execution_path.push(current_id.to_string());

            let node = match node_map.get(current_id) {
                Some(n) => *n,
                None => {
                    result.error = Some(format!("Node not found: {}", current_id));
                    return Ok(result);
                },
            };

            match node {
                FlowNode::End { .. } => {
                    // Reached end - execution successful
                    result.success = true;
                    break;
                },
                FlowNode::Start { next, .. } => {
                    current_id = next.as_str();
                },
                FlowNode::Switch { rules, .. } => {
                    // Evaluate switch rules to determine next node
                    match self.evaluate_switch(rules, &values) {
                        Some(next) => current_id = next,
                        None => {
                            result.error = Some("No matching switch rule".to_string());
                            return Ok(result);
                        },
                    }
                },
                FlowNode::ChangeValue { changes, next, .. } => {
                    // Execute value changes
                    for change in changes {
                        let executed = self.execute_change(change, &values).await;
                        result.actions_executed.push(executed);
                    }
                    current_id = next.as_str();
                },
            }
        }

        Ok(result)
    }

    /// Read all Instance/Channel variables from RTDB
    async fn read_variables(&self, variables: &[Variable]) -> Result<HashMap<String, f64>> {
        let config = KeySpaceConfig::production();
        let mut values = HashMap::new();

        for var in variables {
            let (key, field, name) = match var {
                Variable::Instance {
                    name,
                    instance_id,
                    point_type,
                    point_id,
                } => {
                    let key = match point_type {
                        InstancePointType::Measurement => {
                            config.instance_measurement_key(*instance_id as u32)
                        },
                        InstancePointType::Action => {
                            config.instance_action_key(*instance_id as u32)
                        },
                    };
                    (key.into_owned(), point_id.clone(), name.clone())
                },
                Variable::Channel {
                    name,
                    channel_id,
                    point_type,
                    point_id,
                } => {
                    let key = config.channel_key(*channel_id, *point_type);
                    (key.into_owned(), point_id.clone(), name.clone())
                },
                Variable::Combined { .. } => continue, // Handled in calculate_combined_variables
            };

            match self.rtdb.hash_get(&key, &field).await {
                Ok(Some(val_bytes)) => {
                    let val_str = String::from_utf8_lossy(&val_bytes);
                    if let Ok(val) = val_str.parse::<f64>() {
                        values.insert(name.clone(), val);
                    } else {
                        tracing::warn!(
                            "Variable {}: value '{}' at {}:{} is not a number",
                            name,
                            val_str,
                            key,
                            field
                        );
                        values.insert(name, 0.0);
                    }
                },
                Ok(None) => {
                    tracing::warn!(
                        "Variable {}: key {}:{} not found, using 0.0",
                        name,
                        key,
                        field
                    );
                    values.insert(name, 0.0);
                },
                Err(e) => {
                    tracing::error!("Variable {} read error: {}", name, e);
                    values.insert(name, 0.0);
                },
            }
        }

        Ok(values)
    }

    /// Calculate combined (formula) variables
    fn calculate_combined_variables(
        &self,
        variables: &[Variable],
        values: &mut HashMap<String, f64>,
    ) -> Result<()> {
        for var in variables {
            if let Variable::Combined { name, formula } = var {
                let result = self.evaluate_formula(formula, values)?;
                values.insert(name.clone(), result);
            }
        }
        Ok(())
    }

    /// Evaluate a formula expression
    fn evaluate_formula(
        &self,
        tokens: &[FormulaToken],
        values: &HashMap<String, f64>,
    ) -> Result<f64> {
        // Simple left-to-right evaluation (no operator precedence)
        let mut result = 0.0;
        let mut pending_op: Option<ArithmeticOp> = None;

        for token in tokens {
            let value = match token {
                FormulaToken::Var { name } => values.get(name).copied().unwrap_or(0.0),
                FormulaToken::Num { value } => *value,
                FormulaToken::Op { op } => {
                    pending_op = Some(*op);
                    continue;
                },
            };

            match pending_op.take() {
                None => result = value,
                Some(ArithmeticOp::Add) => result += value,
                Some(ArithmeticOp::Sub) => result -= value,
                Some(ArithmeticOp::Mul) => result *= value,
                Some(ArithmeticOp::Div) => {
                    if value != 0.0 {
                        result /= value;
                    } else {
                        return Err(RuleError::ConditionError("Division by zero".to_string()));
                    }
                },
            }
        }

        Ok(result)
    }

    /// Evaluate switch rules and return the next node ID
    fn evaluate_switch<'a>(
        &self,
        rules: &'a [SwitchRule],
        values: &HashMap<String, f64>,
    ) -> Option<&'a str> {
        for rule in rules {
            if self.evaluate_conditions(&rule.conditions, values) {
                return Some(&rule.next_node);
            }
        }
        None
    }

    /// Evaluate a list of conditions with logical operators
    fn evaluate_conditions(
        &self,
        conditions: &[RuleCondition],
        values: &HashMap<String, f64>,
    ) -> bool {
        if conditions.is_empty() {
            return true;
        }

        let mut result = self.evaluate_single_condition(&conditions[0], values);

        for i in 1..conditions.len() {
            let cond = &conditions[i];
            let cond_result = self.evaluate_single_condition(cond, values);

            // Use the relation from the previous condition
            if let Some(relation) = &conditions[i - 1].relation {
                match relation {
                    LogicOp::And => result = result && cond_result,
                    LogicOp::Or => result = result || cond_result,
                }
            } else {
                // Default to AND if no relation specified
                result = result && cond_result;
            }
        }

        result
    }

    /// Evaluate a single condition
    fn evaluate_single_condition(
        &self,
        cond: &RuleCondition,
        values: &HashMap<String, f64>,
    ) -> bool {
        let left = self.resolve_value(&cond.left, values);
        let right = self.resolve_value(&cond.right, values);

        match cond.operator {
            CompareOp::Eq => (left - right).abs() < f64::EPSILON,
            CompareOp::Ne => (left - right).abs() >= f64::EPSILON,
            CompareOp::Gt => left > right,
            CompareOp::Lt => left < right,
            CompareOp::Gte => left >= right,
            CompareOp::Lte => left <= right,
        }
    }

    /// Resolve a value reference (variable name or number literal)
    fn resolve_value(&self, value: &str, values: &HashMap<String, f64>) -> f64 {
        // Try to get from values map first (variable reference)
        if let Some(v) = values.get(value) {
            return *v;
        }

        // Try to parse as number
        value.parse::<f64>().unwrap_or(0.0)
    }

    /// Execute a value change action
    ///
    /// Supports two target types:
    /// - Instance: Uses M2C routing via set_action_point()
    /// - Channel: Writes directly to comsrv channel point
    async fn execute_change(
        &self,
        change: &ValueChange,
        values: &HashMap<String, f64>,
    ) -> ActionResult {
        match change {
            ValueChange::Instance {
                instance_id,
                point_id,
                value,
            } => {
                // Resolve the value (could be a variable reference or literal)
                let resolved_value: f64 = if let Some(v) = values.get(value) {
                    *v
                } else {
                    value.parse().unwrap_or(0.0)
                };

                // Use voltage_routing to set the action point
                // Note: set_action_point uses instance name, not ID
                // TODO: Add set_action_point_by_id() to voltage-routing
                let instance_name = format!("instance_{}", instance_id);
                let result = set_action_point(
                    self.rtdb.as_ref(),
                    &self.routing_cache,
                    &instance_name,
                    point_id,
                    resolved_value,
                )
                .await;

                ActionResult {
                    target_type: "instance".to_string(),
                    target_id: *instance_id,
                    point_type: "A".to_string(),
                    point_id: point_id.clone(),
                    value: resolved_value.to_string(),
                    success: result.is_ok(),
                }
            },
            ValueChange::Channel {
                channel_id,
                point_type,
                point_id,
                value,
            } => {
                // Resolve the value
                let resolved_value: f64 = if let Some(v) = values.get(value) {
                    *v
                } else {
                    value.parse().unwrap_or(0.0)
                };

                // Write directly to channel point (bypasses routing)
                let config = KeySpaceConfig::production();
                let key = config.channel_key(*channel_id, *point_type);
                let result = self
                    .rtdb
                    .hash_set(&key, point_id, resolved_value.to_string().into())
                    .await;

                let point_type_str = match point_type {
                    ChannelPointType::Telemetry => "T",
                    ChannelPointType::Signal => "S",
                    ChannelPointType::Control => "C",
                    ChannelPointType::Adjustment => "A",
                };

                ActionResult {
                    target_type: "channel".to_string(),
                    target_id: *channel_id,
                    point_type: point_type_str.to_string(),
                    point_id: point_id.clone(),
                    value: resolved_value.to_string(),
                    success: result.is_ok(),
                }
            },
        }
    }
}

/// Helper function to get node ID (since we can't impl methods on external types)
fn node_id(node: &FlowNode) -> &str {
    match node {
        FlowNode::Start { id, .. } => id,
        FlowNode::End { id } => id,
        FlowNode::Switch { id, .. } => id,
        FlowNode::ChangeValue { id, .. } => id,
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use voltage_rtdb::MemoryRtdb;

    #[tokio::test]
    async fn test_evaluate_formula() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);
        values.insert("X2".to_string(), 5.0);

        // X1 + X2 = 15
        let formula = vec![
            FormulaToken::Var {
                name: "X1".to_string(),
            },
            FormulaToken::Op {
                op: ArithmeticOp::Add,
            },
            FormulaToken::Var {
                name: "X2".to_string(),
            },
        ];
        let result = executor.evaluate_formula(&formula, &values).unwrap();
        assert!((result - 15.0).abs() < f64::EPSILON);

        // X1 - 3 = 7
        let formula2 = vec![
            FormulaToken::Var {
                name: "X1".to_string(),
            },
            FormulaToken::Op {
                op: ArithmeticOp::Sub,
            },
            FormulaToken::Num { value: 3.0 },
        ];
        let result2 = executor.evaluate_formula(&formula2, &values).unwrap();
        assert!((result2 - 7.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_evaluate_conditions() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let mut values = HashMap::new();
        values.insert("X1".to_string(), 100.0);
        values.insert("X2".to_string(), 50.0);

        // X1 > X2 (100 > 50 = true)
        let conditions = vec![RuleCondition {
            left: "X1".to_string(),
            operator: CompareOp::Gt,
            right: "X2".to_string(),
            relation: None,
        }];
        assert!(executor.evaluate_conditions(&conditions, &values));

        // X1 == 100 && X2 < 60
        let conditions2 = vec![
            RuleCondition {
                left: "X1".to_string(),
                operator: CompareOp::Eq,
                right: "100".to_string(),
                relation: Some(LogicOp::And),
            },
            RuleCondition {
                left: "X2".to_string(),
                operator: CompareOp::Lt,
                right: "60".to_string(),
                relation: None,
            },
        ];
        assert!(executor.evaluate_conditions(&conditions2, &values));
    }
}
