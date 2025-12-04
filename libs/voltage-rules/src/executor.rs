//! Rule Executor - Execute Vue Flow rules with RuleFlow
//!
//! Executes rule flow by:
//! 1. Traversing nodes from start to end
//! 2. For each node: reading node-local variables, evaluating conditions
//! 3. Executing actions and following wires

use crate::error::Result;
use crate::logger::format_conditions;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::rules::{
    FlowCondition, Rule, RuleNode, RuleSwitchBranch, RuleValueAssignment, RuleVariable,
};
use voltage_config::RoutingCache;
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
    /// Matched condition expression (e.g., "X1>=49" or "X1>10 && X2<50")
    pub matched_condition: Option<String>,
    /// Variable values at execution time (for logging)
    pub variable_values: HashMap<String, f64>,
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

    /// Execute a rule with RuleFlow
    pub async fn execute(&self, rule: &Rule) -> Result<RuleExecutionResult> {
        let mut result = RuleExecutionResult {
            rule_id: rule.id.clone(),
            success: false,
            actions_executed: vec![],
            error: None,
            execution_path: vec![],
            matched_condition: None,
            variable_values: HashMap::new(),
        };

        // Execute from start node, accumulating variable values along the path
        let mut values: HashMap<String, f64> = HashMap::new();
        let mut current_id = rule.flow.start_node.as_str();
        let max_iterations = 100; // Prevent infinite loops
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > max_iterations {
                result.error = Some("Execution exceeded maximum iterations".to_string());
                return Ok(result);
            }

            result.execution_path.push(current_id.to_string());

            let node = match rule.flow.nodes.get(current_id) {
                Some(n) => n,
                None => {
                    result.error = Some(format!("Node not found: {}", current_id));
                    return Ok(result);
                },
            };

            match node {
                RuleNode::End => {
                    // Reached end - execution successful
                    result.success = true;
                    break;
                },
                RuleNode::Start { wires } => {
                    current_id = match wires.default.first() {
                        Some(next) => next.as_str(),
                        None => {
                            result.error = Some("Start node has no output wire".to_string());
                            return Ok(result);
                        },
                    };
                },
                RuleNode::Switch {
                    variables,
                    rule: rules,
                    wires,
                } => {
                    // Read node-local variables
                    if let Err(e) = self.read_rule_variables(variables, &mut values).await {
                        result.error = Some(format!("Failed to read variables: {}", e));
                        // Save variable values even on error for logging
                        result.variable_values = values.clone();
                        return Ok(result);
                    }

                    // Save variable values for logging
                    result.variable_values = values.clone();

                    // Evaluate switch rules to determine next node and capture matched condition
                    let (next_node, matched_cond) =
                        self.evaluate_rule_switch(rules, wires, &values);
                    result.matched_condition = matched_cond;

                    match next_node {
                        Some(next) => current_id = next,
                        None => {
                            result.error = Some("No matching switch rule".to_string());
                            return Ok(result);
                        },
                    }
                },
                RuleNode::ChangeValue {
                    variables,
                    rule: assignments,
                    wires,
                } => {
                    // Read target variables
                    if let Err(e) = self.read_rule_variables(variables, &mut values).await {
                        result.error = Some(format!("Failed to read variables: {}", e));
                        return Ok(result);
                    }

                    // Execute value assignments
                    for assignment in assignments {
                        let variable = variables.iter().find(|v| v.name == assignment.variables);
                        if let Some(var) = variable {
                            let executed = self.execute_rule_change(var, assignment, &values).await;
                            result.actions_executed.push(executed);
                        }
                    }

                    current_id = match wires.default.first() {
                        Some(next) => next.as_str(),
                        None => {
                            result.error = Some("ChangeValue node has no output wire".to_string());
                            return Ok(result);
                        },
                    };
                },
            }
        }

        Ok(result)
    }

    /// Read compact variables from RTDB (node-local variables)
    ///
    /// Resolves instance_name to instance_id via `inst:name:index` hash,
    /// then reads from `inst:{instance_id}:M` or `inst:{instance_id}:A`.
    async fn read_rule_variables(
        &self,
        variables: &[RuleVariable],
        values: &mut HashMap<String, f64>,
    ) -> Result<()> {
        for var in variables {
            // Skip combined variables - they need to be calculated separately
            if var.var_type == "combined" {
                // TODO: Calculate combined variables from formula
                continue;
            }

            // For "single" type, read from instance
            let instance_name = match &var.instance {
                Some(name) => name,
                None => continue, // Skip if no instance specified
            };

            // Resolve instance_name to instance_id via name index
            let instance_id = match self.rtdb.hash_get("inst:name:index", instance_name).await {
                Ok(Some(id_bytes)) => String::from_utf8_lossy(&id_bytes).to_string(),
                Ok(None) => {
                    tracing::warn!("Var {}: inst '{}' not found", var.name, instance_name);
                    values.insert(var.name.clone(), 0.0);
                    continue;
                },
                Err(e) => {
                    tracing::error!("Var {}: inst '{}' err: {}", var.name, instance_name, e);
                    values.insert(var.name.clone(), 0.0);
                    continue;
                },
            };

            let point_type = var.point_type.as_deref().unwrap_or("measurement");
            let point = var.point.unwrap_or(0);

            // Construct key using instance_id (numeric)
            // Format: "inst:{instance_id}:M" or "inst:{instance_id}:A"
            let key = if point_type == "action" {
                format!("inst:{}:A", instance_id)
            } else {
                format!("inst:{}:M", instance_id)
            };

            let field = point.to_string();

            match self.rtdb.hash_get(&key, &field).await {
                Ok(Some(val_bytes)) => {
                    let val_str = String::from_utf8_lossy(&val_bytes);
                    if let Ok(val) = val_str.parse::<f64>() {
                        values.insert(var.name.clone(), val);
                    } else {
                        tracing::warn!(
                            "Var {}: '{}' not number at {}:{}",
                            var.name,
                            val_str,
                            key,
                            field
                        );
                        values.insert(var.name.clone(), 0.0);
                    }
                },
                Ok(None) => {
                    tracing::warn!("Var {}: {}:{} not found", var.name, key, field);
                    values.insert(var.name.clone(), 0.0);
                },
                Err(e) => {
                    tracing::error!("Var {} read err: {}", var.name, e);
                    values.insert(var.name.clone(), 0.0);
                },
            }
        }

        Ok(())
    }

    /// Evaluate compact switch rules and return the next node ID with matched condition
    ///
    /// Returns: (next_node_id, matched_condition_expression)
    fn evaluate_rule_switch<'a>(
        &self,
        rules: &[RuleSwitchBranch],
        wires: &'a HashMap<String, Vec<String>>,
        values: &HashMap<String, f64>,
    ) -> (Option<&'a str>, Option<String>) {
        for rule in rules {
            if self.evaluate_flow_conditions(&rule.rule, values) {
                // Format the matched condition expression
                let condition_str = format_conditions(&rule.rule);

                // Find the wire target for this rule's output
                if let Some(targets) = wires.get(&rule.name) {
                    if let Some(target) = targets.first() {
                        return (Some(target.as_str()), Some(condition_str));
                    }
                }
            }
        }
        (None, None)
    }

    /// Evaluate compact conditions
    fn evaluate_flow_conditions(
        &self,
        conditions: &[FlowCondition],
        values: &HashMap<String, f64>,
    ) -> bool {
        if conditions.is_empty() {
            return true;
        }

        let mut result = true;
        let mut pending_relation: Option<&str> = None;

        for cond in conditions {
            if cond.cond_type == "relation" {
                // Store relation for next condition
                pending_relation = cond.value.as_ref().and_then(|v| v.as_str());
                continue;
            }

            // Evaluate variable condition
            let cond_result = self.evaluate_flow_condition(cond, values);

            // Combine with previous result
            match pending_relation {
                Some("||") | Some("or") | Some("OR") => {
                    result = result || cond_result;
                },
                _ => {
                    // Default to AND
                    result = result && cond_result;
                },
            }
            pending_relation = None;
        }

        result
    }

    /// Evaluate a single compact condition
    fn evaluate_flow_condition(&self, cond: &FlowCondition, values: &HashMap<String, f64>) -> bool {
        let var_name = match &cond.variables {
            Some(name) => name,
            None => return false,
        };

        let operator = cond.operator.as_deref().unwrap_or("==");

        let left = values.get(var_name).copied().unwrap_or(0.0);
        let right = match &cond.value {
            Some(v) => {
                if let Some(n) = v.as_f64() {
                    n
                } else if let Some(n) = v.as_i64() {
                    n as f64
                } else if let Some(s) = v.as_str() {
                    // Could be a variable reference
                    values.get(s).copied().unwrap_or(s.parse().unwrap_or(0.0))
                } else {
                    0.0
                }
            },
            None => 0.0,
        };

        match operator {
            "==" | "eq" => (left - right).abs() < f64::EPSILON,
            "!=" | "ne" => (left - right).abs() >= f64::EPSILON,
            ">" | "gt" => left > right,
            "<" | "lt" => left < right,
            ">=" | "gte" => left >= right,
            "<=" | "lte" => left <= right,
            _ => false,
        }
    }

    /// Execute a compact value change action
    async fn execute_rule_change(
        &self,
        variable: &RuleVariable,
        assignment: &RuleValueAssignment,
        values: &HashMap<String, f64>,
    ) -> ActionResult {
        // Resolve the value to write
        let resolved_value: f64 = if let Some(n) = assignment.value.as_f64() {
            n
        } else if let Some(n) = assignment.value.as_i64() {
            n as f64
        } else if let Some(s) = assignment.value.as_str() {
            // Could be a variable reference
            values.get(s).copied().unwrap_or(s.parse().unwrap_or(0.0))
        } else {
            0.0
        };

        let instance_name = variable.instance.as_deref().unwrap_or("unknown");
        let point = variable.point.unwrap_or(0);

        // Use voltage_routing to set the action point
        let result = set_action_point(
            self.rtdb.as_ref(),
            &self.routing_cache,
            instance_name,
            &point.to_string(),
            resolved_value,
        )
        .await;

        ActionResult {
            target_type: "instance".to_string(),
            target_id: 0, // Instance ID not available in compact format
            point_type: variable
                .point_type
                .as_deref()
                .unwrap_or("action")
                .to_string(),
            point_id: point.to_string(),
            value: resolved_value.to_string(),
            success: result.is_ok(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use serde_json::json;
    use voltage_config::rules::Rule;
    use voltage_rtdb::{Bytes, MemoryRtdb};

    use crate::parser::extract_rule_flow;

    #[tokio::test]
    async fn test_evaluate_flow_condition() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let mut values = HashMap::new();
        values.insert("X1".to_string(), 100.0);
        values.insert("X2".to_string(), 50.0);

        // X1 > X2 (100 > 50 = true)
        let condition = FlowCondition {
            cond_type: "variable".to_string(),
            variables: Some("X1".to_string()),
            operator: Some(">".to_string()),
            value: Some(json!("X2")),
        };
        assert!(executor.evaluate_flow_condition(&condition, &values));

        // X1 <= 100 (true)
        let condition2 = FlowCondition {
            cond_type: "variable".to_string(),
            variables: Some("X1".to_string()),
            operator: Some("<=".to_string()),
            value: Some(json!(100)),
        };
        assert!(executor.evaluate_flow_condition(&condition2, &values));

        // X2 >= 60 (50 >= 60 = false)
        let condition3 = FlowCondition {
            cond_type: "variable".to_string(),
            variables: Some("X2".to_string()),
            operator: Some(">=".to_string()),
            value: Some(json!(60)),
        };
        assert!(!executor.evaluate_flow_condition(&condition3, &values));
    }

    #[tokio::test]
    async fn test_evaluate_flow_conditions_with_logic() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let mut values = HashMap::new();
        values.insert("X1".to_string(), 100.0);
        values.insert("X2".to_string(), 50.0);

        // X1 == 100 && X2 < 60 (true AND true = true)
        let conditions = vec![
            FlowCondition {
                cond_type: "variable".to_string(),
                variables: Some("X1".to_string()),
                operator: Some("==".to_string()),
                value: Some(json!(100)),
            },
            FlowCondition {
                cond_type: "relation".to_string(),
                variables: None,
                operator: None,
                value: Some(json!("&&")),
            },
            FlowCondition {
                cond_type: "variable".to_string(),
                variables: Some("X2".to_string()),
                operator: Some("<".to_string()),
                value: Some(json!(60)),
            },
        ];
        assert!(executor.evaluate_flow_conditions(&conditions, &values));

        // X1 > 200 || X2 == 50 (false OR true = true)
        let conditions2 = vec![
            FlowCondition {
                cond_type: "variable".to_string(),
                variables: Some("X1".to_string()),
                operator: Some(">".to_string()),
                value: Some(json!(200)),
            },
            FlowCondition {
                cond_type: "relation".to_string(),
                variables: None,
                operator: None,
                value: Some(json!("||")),
            },
            FlowCondition {
                cond_type: "variable".to_string(),
                variables: Some("X2".to_string()),
                operator: Some("==".to_string()),
                value: Some(json!(50)),
            },
        ];
        assert!(executor.evaluate_flow_conditions(&conditions2, &values));
    }

    #[tokio::test]
    async fn test_evaluate_rule_switch() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);

        let rules = vec![
            RuleSwitchBranch {
                name: "out001".to_string(),
                rule_type: "default".to_string(),
                rule: vec![FlowCondition {
                    cond_type: "variable".to_string(),
                    variables: Some("X1".to_string()),
                    operator: Some("<=".to_string()),
                    value: Some(json!(5)),
                }],
            },
            RuleSwitchBranch {
                name: "out002".to_string(),
                rule_type: "default".to_string(),
                rule: vec![FlowCondition {
                    cond_type: "variable".to_string(),
                    variables: Some("X1".to_string()),
                    operator: Some(">".to_string()),
                    value: Some(json!(5)),
                }],
            },
        ];

        let mut wires = HashMap::new();
        wires.insert("out001".to_string(), vec!["node-low".to_string()]);
        wires.insert("out002".to_string(), vec!["node-high".to_string()]);

        // X1=10 > 5, should match out002
        let (next, condition) = executor.evaluate_rule_switch(&rules, &wires, &values);
        assert_eq!(next, Some("node-high"));
        assert_eq!(condition, Some("X1>5".to_string()));
    }

    // =========================================================================
    // SOC Strategy Tests
    // =========================================================================

    /// Helper: Setup instance name index for testing
    async fn setup_name_index(rtdb: &MemoryRtdb) {
        rtdb.hash_set("inst:name:index", "battery_01", Bytes::from("5"))
            .await
            .unwrap();
        rtdb.hash_set("inst:name:index", "pv_01", Bytes::from("6"))
            .await
            .unwrap();
        rtdb.hash_set("inst:name:index", "diesel_gen_01", Bytes::from("7"))
            .await
            .unwrap();
    }

    /// Helper: Build simplified SOC strategy flow JSON for testing
    ///
    /// Logic:
    /// - X1 <= 5 (low battery) → out001 → changeValue1 (pv_01:A:5=999)
    /// - X1 >= 49 (medium)     → out002 → changeValue2 (diesel_gen_01:A:2=1)
    /// - X1 >= 99 (high)       → out003 → changeValue3 (pv_01:A:5=78)
    fn soc_strategy_json() -> serde_json::Value {
        json!({
            "nodes": [
                {
                    "id": "start",
                    "type": "start",
                    "data": {
                        "config": {
                            "wires": { "default": ["switch1"] }
                        }
                    }
                },
                {
                    "id": "switch1",
                    "type": "custom",
                    "data": {
                        "type": "function-switch",
                        "config": {
                            "variables": [{
                                "name": "X1",
                                "type": "single",
                                "instance": "battery_01",
                                "pointType": "measurement",
                                "point": 3
                            }],
                            "rule": [
                                {
                                    "name": "out001",
                                    "type": "default",
                                    "rule": [{
                                        "type": "variable",
                                        "variables": "X1",
                                        "operator": "<=",
                                        "value": 5
                                    }]
                                },
                                {
                                    "name": "out002",
                                    "type": "default",
                                    "rule": [{
                                        "type": "variable",
                                        "variables": "X1",
                                        "operator": ">=",
                                        "value": 49
                                    }]
                                },
                                {
                                    "name": "out003",
                                    "type": "default",
                                    "rule": [{
                                        "type": "variable",
                                        "variables": "X1",
                                        "operator": ">=",
                                        "value": 99
                                    }]
                                }
                            ],
                            "wires": {
                                "out001": ["changeValue1"],
                                "out002": ["changeValue2"],
                                "out003": ["changeValue3"]
                            }
                        }
                    }
                },
                {
                    "id": "changeValue1",
                    "type": "custom",
                    "data": {
                        "type": "action-changeValue",
                        "config": {
                            "variables": [{
                                "name": "Y1",
                                "type": "single",
                                "instance": "pv_01",
                                "pointType": "action",
                                "point": 5
                            }],
                            "rule": [{ "Variables": "Y1", "value": 999 }],
                            "wires": { "default": ["end"] }
                        }
                    }
                },
                {
                    "id": "changeValue2",
                    "type": "custom",
                    "data": {
                        "type": "action-changeValue",
                        "config": {
                            "variables": [{
                                "name": "Y2",
                                "type": "single",
                                "instance": "diesel_gen_01",
                                "pointType": "action",
                                "point": 2
                            }],
                            "rule": [{ "Variables": "Y2", "value": 1 }],
                            "wires": { "default": ["end"] }
                        }
                    }
                },
                {
                    "id": "changeValue3",
                    "type": "custom",
                    "data": {
                        "type": "action-changeValue",
                        "config": {
                            "variables": [{
                                "name": "Y3",
                                "type": "single",
                                "instance": "pv_01",
                                "pointType": "action",
                                "point": 5
                            }],
                            "rule": [{ "Variables": "Y3", "value": 78 }],
                            "wires": { "default": ["end"] }
                        }
                    }
                },
                {
                    "id": "end",
                    "type": "end"
                }
            ]
        })
    }

    /// Helper: Create Rule from JSON
    fn create_soc_rule() -> Rule {
        let flow_json = soc_strategy_json();
        let rule_flow = extract_rule_flow(&flow_json).unwrap();
        Rule {
            id: "soc-strategy-001".to_string(),
            name: "SOC Strategy".to_string(),
            description: None,
            enabled: true,
            priority: 0,
            cooldown_ms: 0,
            flow: rule_flow,
        }
    }

    #[tokio::test]
    async fn test_soc_strategy_low_battery() {
        // SOC = 3.5 → should match out001 (X1 <= 5)
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        setup_name_index(&rtdb).await;
        rtdb.hash_set("inst:5:M", "3", Bytes::from("3.5"))
            .await
            .unwrap();

        let rule = create_soc_rule();
        let executor = RuleExecutor::new(rtdb.clone(), routing_cache);
        let result = executor.execute(&rule).await.unwrap();

        assert!(result.success, "Execution should succeed");
        assert!(
            result.execution_path.contains(&"changeValue1".to_string()),
            "Should execute changeValue1 for low battery. Path: {:?}",
            result.execution_path
        );
        assert_eq!(result.actions_executed.len(), 1);
        assert_eq!(result.actions_executed[0].value, "999");
    }

    #[tokio::test]
    async fn test_soc_strategy_boundary_5() {
        // SOC = 5.0 → should match out001 (X1 <= 5)
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        setup_name_index(&rtdb).await;
        rtdb.hash_set("inst:5:M", "3", Bytes::from("5.0"))
            .await
            .unwrap();

        let rule = create_soc_rule();
        let executor = RuleExecutor::new(rtdb.clone(), routing_cache);
        let result = executor.execute(&rule).await.unwrap();

        assert!(result.success);
        assert!(result.execution_path.contains(&"changeValue1".to_string()));
    }

    #[tokio::test]
    async fn test_soc_strategy_medium_battery() {
        // SOC = 50.0 → should match out002 (X1 >= 49)
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        setup_name_index(&rtdb).await;
        rtdb.hash_set("inst:5:M", "3", Bytes::from("50.0"))
            .await
            .unwrap();

        let rule = create_soc_rule();
        let executor = RuleExecutor::new(rtdb.clone(), routing_cache);
        let result = executor.execute(&rule).await.unwrap();

        assert!(result.success);
        assert!(
            result.execution_path.contains(&"changeValue2".to_string()),
            "Should execute changeValue2 for medium battery. Path: {:?}",
            result.execution_path
        );
        assert_eq!(result.actions_executed.len(), 1);
        assert_eq!(result.actions_executed[0].value, "1");
    }

    #[tokio::test]
    async fn test_soc_strategy_high_battery() {
        // SOC = 99.5 → should match out003 (X1 >= 99)
        // Note: out002 (X1 >= 49) is also true, but out003 is checked first in order
        // Actually in the JSON, conditions are in order: out001, out002, out003
        // 99.5 >= 49 is true, so out002 matches first!
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        setup_name_index(&rtdb).await;
        rtdb.hash_set("inst:5:M", "3", Bytes::from("99.5"))
            .await
            .unwrap();

        let rule = create_soc_rule();
        let executor = RuleExecutor::new(rtdb.clone(), routing_cache);
        let result = executor.execute(&rule).await.unwrap();

        assert!(result.success);
        // Note: Due to condition order, out002 (>=49) matches before out003 (>=99)
        assert!(
            result.execution_path.contains(&"changeValue2".to_string()),
            "Due to condition order, out002 matches first. Path: {:?}",
            result.execution_path
        );
    }

    #[tokio::test]
    async fn test_soc_strategy_no_match() {
        // SOC = 25.0 → no match (5 < 25 < 49)
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        setup_name_index(&rtdb).await;
        rtdb.hash_set("inst:5:M", "3", Bytes::from("25.0"))
            .await
            .unwrap();

        let rule = create_soc_rule();
        let executor = RuleExecutor::new(rtdb.clone(), routing_cache);
        let result = executor.execute(&rule).await.unwrap();

        // Should fail because no matching branch
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("No matching switch rule"));
    }

    #[tokio::test]
    async fn test_read_rule_variables_with_name_index() {
        // Test that read_rule_variables correctly uses name index
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        // Setup name index
        rtdb.hash_set("inst:name:index", "test_device", Bytes::from("100"))
            .await
            .unwrap();
        // Setup measurement data using numeric ID
        rtdb.hash_set("inst:100:M", "1", Bytes::from("42.5"))
            .await
            .unwrap();

        let executor = RuleExecutor::new(rtdb, routing_cache);

        let variables = vec![RuleVariable {
            name: "X1".to_string(),
            var_type: "single".to_string(),
            instance: Some("test_device".to_string()),
            point_type: Some("measurement".to_string()),
            point: Some(1),
            formula: vec![],
        }];

        let mut values = HashMap::new();
        executor
            .read_rule_variables(&variables, &mut values)
            .await
            .unwrap();

        assert_eq!(values.get("X1"), Some(&42.5));
    }
}
