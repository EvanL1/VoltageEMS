//! Rule Executor - Execute Vue Flow rules with RuleFlow
//!
//! Executes rule flow by:
//! 1. Traversing nodes from start to end
//! 2. For each node: reading node-local variables, evaluating conditions
//! 3. Executing actions and following wires

use crate::error::Result;
use crate::logger::format_conditions;
use crate::types::{
    CalculationRule, FlowCondition, Rule, RuleNode, RuleSwitchBranch, RuleValueAssignment,
    RuleVariable,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_calc::{CalcEngine, MemoryStateStore, StateStore};
use voltage_model::{sanitize_value, ValidationConfig};
use voltage_routing::set_action_point;
use voltage_routing::RoutingCache;
use voltage_rtdb::numfmt::precomputed;
use voltage_rtdb::traits::Rtdb;
use voltage_rtdb::KeySpaceConfig;
use voltage_rtdb_shm::{ShmNotifier, UnifiedReader};

/// Convert dynamic point type string to static str for zero-allocation ActionResult
#[inline]
fn point_type_to_static(pt: Option<&str>, default: &'static str) -> &'static str {
    match pt {
        Some("M") | Some("measurement") => "M",
        Some("A") | Some("action") => "A",
        Some("T") | Some("telemetry") => "T",
        Some("S") | Some("status") => "S",
        Some("C") | Some("control") => "C",
        _ => default,
    }
}

/// Check if a string is an arithmetic operator
#[inline]
fn is_rpn_operator(s: &str) -> bool {
    matches!(s, "+" | "-" | "*" | "/")
}

/// Evaluate a formula in Reverse Polish Notation (RPN)
///
/// Formula format: `["X1", "X2", "+", 2, "*"]`
/// This evaluates as: `(X1 + X2) * 2`
///
/// Supported operators: `+`, `-`, `*`, `/`
///
/// # Arguments
/// * `formula` - RPN tokens (variable names, numbers, operators)
/// * `values` - Variable values map
///
/// # Returns
/// * `Some(f64)` - Computed result
/// * `None` - If formula is invalid or references undefined variables
fn evaluate_rpn_formula(
    formula: &[serde_json::Value],
    values: &HashMap<String, f64>,
) -> Option<f64> {
    let mut stack: Vec<f64> = Vec::with_capacity(formula.len());

    for token in formula {
        match token {
            // Operator: pop two operands, compute, push result
            serde_json::Value::String(s) if is_rpn_operator(s) => {
                if stack.len() < 2 {
                    tracing::warn!(
                        "RPN formula error: not enough operands for operator '{}'",
                        s
                    );
                    return None;
                }
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match s.as_str() {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" => {
                        if b == 0.0 {
                            tracing::warn!("RPN formula error: division by zero");
                            return None;
                        }
                        a / b
                    },
                    _ => return None,
                };
                stack.push(result);
            },
            // Variable reference: look up in values map
            serde_json::Value::String(var_name) => {
                let val = values.get(var_name).copied().or_else(|| {
                    tracing::warn!("RPN formula error: undefined variable '{}'", var_name);
                    None
                })?;
                stack.push(val);
            },
            // Number literal (integer or float)
            serde_json::Value::Number(n) => {
                let val = n.as_f64().or_else(|| {
                    tracing::warn!("RPN formula error: invalid number {:?}", n);
                    None
                })?;
                stack.push(val);
            },
            _ => {
                tracing::warn!("RPN formula error: invalid token {:?}", token);
                return None;
            },
        }
    }

    // Result should be a single value on the stack
    if stack.len() == 1 {
        stack.pop()
    } else {
        tracing::warn!(
            "RPN formula error: expected 1 result, got {} values on stack",
            stack.len()
        );
        None
    }
}

/// Result of executing a rule
#[derive(Debug, Clone, Serialize)]
pub struct RuleExecutionResult {
    pub rule_id: i64,
    pub success: bool,
    pub actions_executed: Vec<ActionResult>,
    pub error: Option<String>,
    pub execution_path: Vec<String>, // Node IDs visited
    /// Matched condition expression (e.g., "X1>=49" or "X1>10 && X2<50")
    pub matched_condition: Option<String>,
    /// Variable values at execution time (for logging)
    /// Arc-shared to avoid cloning the full HashMap on each node
    pub variable_values: Arc<HashMap<String, f64>>,
    /// Node execution details for debugging/visualization
    pub node_details: HashMap<String, NodeExecutionDetail>,
}

/// Record of an executed action
///
/// All fields are Copy types, making this struct zero-cost to clone.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ActionResult {
    /// Target type: "instance" or "channel"
    pub target_type: &'static str,
    /// Target ID (instance_id or channel_id)
    pub target_id: u32,
    /// Point type (M/A for instance, T/S/C/A for channel)
    pub point_type: &'static str,
    /// Point ID
    pub point_id: u32,
    /// Value written (f64 for zero-allocation)
    pub value: f64,
    /// Whether the action succeeded
    pub success: bool,
}

/// Execution details for a single node (for debugging/visualization)
#[derive(Debug, Clone, Serialize)]
pub struct NodeExecutionDetail {
    /// Node type: "start", "switch", "change", "end", "calculation"
    pub node_type: &'static str,
    /// Variable values when entering this node (Arc-shared snapshot)
    pub input_values: Arc<HashMap<String, f64>>,
    /// Condition evaluation results (for Switch nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_results: Option<Vec<ConditionResult>>,
    /// The matched output port (for Switch nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_port: Option<String>,
    /// Actions executed (for ChangeValue nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<ActionResult>>,
}

/// Result of evaluating a single condition branch
#[derive(Debug, Clone, Serialize)]
pub struct ConditionResult {
    /// The condition expression (e.g., "X1>=49")
    pub expression: String,
    /// Whether this condition evaluated to true
    pub result: bool,
    /// The output port name for this condition
    pub port: String,
}

/// Rule executor
///
/// # Two-tier Architecture
/// Removed VecRtdb (L2 cache). Now using:
/// 1. SharedMemory (~5μs) - cross-process mmap
/// 2. Redis (~1ms) - remote fallback
pub struct RuleExecutor<R: Rtdb, S: StateStore = MemoryStateStore> {
    rtdb: Arc<R>,
    routing_cache: Arc<RoutingCache>,
    /// State store for stateful calculation functions (integrate, moving_avg, etc.)
    state_store: Arc<S>,
    /// Optional UnifiedReader for cross-process zero-copy reads
    shared_reader: Option<Arc<UnifiedReader>>,
    /// Optional UnifiedWriter for M2C via shared memory
    shm_action_writer: Option<Arc<voltage_rtdb_shm::UnifiedWriter>>,
    /// Optional ShmNotifier for UDS event notification (M2C low-latency path)
    shm_notifier: Option<Arc<tokio::sync::Mutex<ShmNotifier>>>,
}

impl<R: Rtdb> RuleExecutor<R, MemoryStateStore> {
    /// Create with default MemoryStateStore
    pub fn new(rtdb: Arc<R>, routing_cache: Arc<RoutingCache>) -> Self {
        Self {
            rtdb,
            routing_cache,
            state_store: Arc::new(MemoryStateStore::new()),
            shared_reader: None,
            shm_action_writer: None,
            shm_notifier: None,
        }
    }
}

impl<R: Rtdb, S: StateStore> RuleExecutor<R, S> {
    /// Create with custom state store
    pub fn with_state_store(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        state_store: Arc<S>,
    ) -> Self {
        Self {
            rtdb,
            routing_cache,
            state_store,
            shared_reader: None,
            shm_action_writer: None,
            shm_notifier: None,
        }
    }

    /// Enable UnifiedReader for cross-process zero-copy reads
    ///
    /// When enabled, `read_rule_variables()` uses two-tier priority:
    /// 1. SharedMemory (~5μs) - cross-process mmap, highest priority
    /// 2. Redis (~1ms) - remote fallback
    ///
    /// SharedMemory is populated by comsrv and works on any filesystem.
    /// Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    pub fn with_shared_reader(mut self, reader: Arc<UnifiedReader>) -> Self {
        self.shared_reader = Some(reader);
        self
    }

    /// Enable UnifiedWriter for M2C via shared memory
    ///
    /// When enabled, action outputs are written to SHM in addition to Redis.
    /// SHM serves as the primary path for comsrv's ShmCommandPoller.
    pub fn with_shm_action_writer(mut self, writer: Arc<voltage_rtdb_shm::UnifiedWriter>) -> Self {
        self.shm_action_writer = Some(writer);
        self
    }

    /// Enable ShmNotifier for UDS event notification
    ///
    /// When enabled, after writing to SHM, sends UDS notification to comsrv
    /// for immediate command dispatch (~1-2ms latency vs polling).
    pub fn with_shm_notifier(mut self, notifier: Arc<tokio::sync::Mutex<ShmNotifier>>) -> Self {
        self.shm_notifier = Some(notifier);
        self
    }

    /// Execute a rule with RuleFlow
    pub async fn execute(&self, rule: &Rule) -> Result<RuleExecutionResult> {
        let mut result = RuleExecutionResult {
            rule_id: rule.id,
            success: false,
            actions_executed: vec![],
            error: None,
            execution_path: vec![],
            matched_condition: None,
            variable_values: Arc::new(HashMap::new()),
            node_details: HashMap::new(),
        };

        // Execute from start node, accumulating variable values along the path
        let mut values: HashMap<String, f64> = HashMap::new();
        let mut current_id = rule.flow.start_node.as_str();
        let max_iterations = 100; // Prevent infinite loops
        let mut iterations = 0;

        // Cached snapshot for avoiding repeated clones
        let mut values_snapshot: Option<Arc<HashMap<String, f64>>> = None;

        fn snapshot_or_reuse(
            cache: &mut Option<Arc<HashMap<String, f64>>>,
            values: &HashMap<String, f64>,
            values_changed: bool,
        ) -> Arc<HashMap<String, f64>> {
            if !values_changed {
                if let Some(snapshot) = cache.as_ref() {
                    return Arc::clone(snapshot);
                }
            }

            let snapshot = Arc::new(values.clone());
            *cache = Some(Arc::clone(&snapshot));
            snapshot
        }

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
                    // Save final variable values and mark success (wrap in Arc)
                    result.variable_values = Arc::new(std::mem::take(&mut values));
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
                    let values_changed =
                        match self.read_rule_variables(variables, &mut values).await {
                            Ok(changed) => changed,
                            Err(e) => {
                                result.error = Some(format!("Failed to read variables: {}", e));
                                // Save variable values even on error (wrap in Arc)
                                result.variable_values = Arc::new(std::mem::take(&mut values));
                                return Ok(result);
                            },
                        };

                    // Snapshot values when entering this node (reuse cache if nothing changed)
                    let snapshot = snapshot_or_reuse(&mut values_snapshot, &values, values_changed);
                    result.variable_values = Arc::clone(&snapshot);

                    // Evaluate all conditions for debugging/visualization
                    let condition_results = self.evaluate_all_conditions(rules, &values);

                    // Evaluate switch rules to determine next node and capture matched condition
                    let (next_node, matched_port, matched_cond) =
                        self.evaluate_rule_switch_with_details(rules, wires, &values);
                    result.matched_condition = matched_cond;

                    // Record node execution detail (reuse Arc snapshot)
                    result.node_details.insert(
                        current_id.to_string(),
                        NodeExecutionDetail {
                            node_type: "switch",
                            input_values: snapshot,
                            condition_results: Some(condition_results),
                            matched_port,
                            actions: None,
                        },
                    );

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
                    let values_changed =
                        match self.read_rule_variables(variables, &mut values).await {
                            Ok(changed) => changed,
                            Err(e) => {
                                result.error = Some(format!("Failed to read variables: {}", e));
                                return Ok(result);
                            },
                        };

                    // Snapshot values when entering this node (before executing actions)
                    let input_snapshot =
                        snapshot_or_reuse(&mut values_snapshot, &values, values_changed);
                    result.variable_values = Arc::clone(&input_snapshot);

                    // Execute value assignments and collect actions for this node
                    let mut node_actions = Vec::new();
                    for assignment in assignments {
                        let variable = variables.iter().find(|v| v.name == assignment.variables);
                        if let Some(var) = variable {
                            let executed = self.execute_rule_change(var, assignment, &values).await;
                            node_actions.push(executed);
                            result.actions_executed.push(executed);
                        }
                    }

                    // Record node execution detail
                    result.node_details.insert(
                        current_id.to_string(),
                        NodeExecutionDetail {
                            node_type: "change",
                            input_values: input_snapshot,
                            condition_results: None,
                            matched_port: None,
                            actions: Some(node_actions),
                        },
                    );

                    current_id = match wires.default.first() {
                        Some(next) => next.as_str(),
                        None => {
                            result.error = Some("ChangeValue node has no output wire".to_string());
                            return Ok(result);
                        },
                    };
                },
                RuleNode::Calculation {
                    variables,
                    rule: calculations,
                    wires,
                } => {
                    // Read input variables
                    let values_changed =
                        match self.read_rule_variables(variables, &mut values).await {
                            Ok(changed) => changed,
                            Err(e) => {
                                result.error = Some(format!("Failed to read variables: {}", e));
                                return Ok(result);
                            },
                        };

                    // Snapshot values when entering this node (before calculation writes to values)
                    let input_snapshot =
                        snapshot_or_reuse(&mut values_snapshot, &values, values_changed);
                    result.variable_values = Arc::clone(&input_snapshot);

                    // Create CalcEngine with rule_id as context (for stateful functions)
                    let calc_engine =
                        CalcEngine::new(Arc::clone(&self.state_store), format!("rule_{}", rule.id));
                    let mut node_actions = Vec::new();

                    for calc in calculations {
                        // Evaluate formula (supports stateful functions like integrate)
                        let calc_result = match calc_engine.evaluate(&calc.formula, &values).await {
                            Ok(v) => v,
                            Err(e) => {
                                result.error =
                                    Some(format!("Calc '{}' error: {}", calc.formula, e));
                                return Ok(result);
                            },
                        };

                        // Find output variable and write result
                        if let Some(var) = variables.iter().find(|v| v.name == calc.output) {
                            let action =
                                self.write_calculation_result(var, calc_result, calc).await;
                            node_actions.push(action);
                            result.actions_executed.push(action);
                        }

                        // Update local values for chained calculations
                        values.insert(calc.output.clone(), calc_result);
                    }

                    // Values were modified by calculations; invalidate snapshot cache.
                    values_snapshot = None;
                    result.node_details.insert(
                        current_id.to_string(),
                        NodeExecutionDetail {
                            node_type: "calculation",
                            input_values: input_snapshot,
                            condition_results: None,
                            matched_port: None,
                            actions: Some(node_actions),
                        },
                    );

                    current_id = match wires.default.first() {
                        Some(next) => next.as_str(),
                        None => {
                            result.error = Some("Calculation node has no output wire".to_string());
                            return Ok(result);
                        },
                    };
                },
                RuleNode::PeriodDelta {
                    input,
                    output,
                    period,
                    wires,
                } => {
                    // Read input variable (cumulative value like total energy)
                    let input_vars = vec![input.clone()];
                    let values_changed =
                        match self.read_rule_variables(&input_vars, &mut values).await {
                            Ok(changed) => changed,
                            Err(e) => {
                                result.error =
                                    Some(format!("Failed to read input variable: {}", e));
                                return Ok(result);
                            },
                        };

                    // Snapshot values when entering this node
                    let input_snapshot =
                        snapshot_or_reuse(&mut values_snapshot, &values, values_changed);
                    result.variable_values = Arc::clone(&input_snapshot);

                    // Get the input value
                    let input_value = values.get(&input.name).copied().unwrap_or(0.0);

                    // Create CalcEngine with rule_id as context (for stateful period_delta)
                    let calc_engine =
                        CalcEngine::new(Arc::clone(&self.state_store), format!("rule_{}", rule.id));

                    // Calculate period delta using builtin function
                    // State key format: calc:state:rule_{id}:period_delta:{var_name}_{period}
                    let state_key = format!(
                        "{}:{}:{}",
                        rule.id,
                        input.instance.unwrap_or(0),
                        input.point.unwrap_or(0)
                    );
                    let delta = match calc_engine
                        .builtin()
                        .period_delta(&state_key, input_value, period)
                        .await
                    {
                        Ok(v) => v,
                        Err(e) => {
                            result.error = Some(format!("period_delta error: {}", e));
                            return Ok(result);
                        },
                    };

                    // Write delta to output variable
                    let action = self.write_period_delta_result(output, delta, period).await;
                    let node_actions = vec![action];
                    result.actions_executed.push(action);

                    // Update local values
                    values.insert(output.name.clone(), delta);
                    values_snapshot = None; // Invalidate cache

                    // Record node execution detail
                    result.node_details.insert(
                        current_id.to_string(),
                        NodeExecutionDetail {
                            node_type: "periodDelta",
                            input_values: input_snapshot,
                            condition_results: None,
                            matched_port: None,
                            actions: Some(node_actions),
                        },
                    );

                    current_id = match wires.default.first() {
                        Some(next) => next.as_str(),
                        None => {
                            result.error = Some("PeriodDelta node has no output wire".to_string());
                            return Ok(result);
                        },
                    };
                },
            }
        }

        Ok(result)
    }

    /// Read variables from RTDB with two-tier priority
    ///
    /// Priority order:
    /// 1. SharedMemory (~5μs) - cross-process mmap, highest priority
    /// 2. Redis (~1ms) - remote fallback
    ///
    /// Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    ///
    /// Reads variable values from Redis Hash `inst:{id}:M` or `inst:{id}:A`
    async fn read_rule_variables(
        &self,
        variables: &[RuleVariable],
        values: &mut HashMap<String, f64>,
    ) -> Result<bool> {
        let mut values_changed = false;
        let keyspace = KeySpaceConfig::production_cached();

        // ★ Phase 1a: Try SHM first, collect Redis fallback requests
        // Group by Redis key for batched HMGET (reduces N calls to ~2 calls)
        // Key: (redis_key, is_action), Value: Vec<(var_name, field)>
        let mut redis_requests: HashMap<(String, bool), Vec<(String, String)>> = HashMap::new();

        for var in variables {
            // Skip formula variables in Phase 1 - calculated in Phase 2 after base variables
            if !var.formula.is_empty() {
                continue;
            }

            let var_name = var.name.clone();

            // Get instance ID (supports both "instance" and "instance_id" via serde alias)
            let instance_id = match var.instance {
                Some(id) => id,
                None => {
                    return Err(crate::error::RuleError::ExecutionError(format!(
                        "Variable '{}' is missing instance_id",
                        var_name
                    )));
                },
            };

            let point_type = var.point_type.as_deref().unwrap_or("measurement");
            let point = var.point.ok_or_else(|| {
                crate::error::RuleError::ExecutionError(format!(
                    "Variable '{}' is missing point_id",
                    var_name
                ))
            })?;

            let is_action = point_type == "action";

            // ★ Priority 1: SharedMemory (~5μs) - cross-process zero-copy
            if let Some(reader) = &self.shared_reader {
                let instance_type = if is_action { 1 } else { 0 };
                if let Some((val, _ts)) =
                    reader.get_instance(instance_id, instance_type, point, &self.routing_cache)
                {
                    // SharedMemory hit - fastest path
                    values_changed |= values.insert(var_name, val) != Some(val);
                    continue;
                }
            }

            // SHM miss - queue for batched Redis fetch
            let key = if is_action {
                keyspace.instance_action_key(instance_id)
            } else {
                keyspace.instance_measurement_key(instance_id)
            };
            let field = precomputed::get_point_id_str_or_alloc(point).to_string();
            redis_requests
                .entry((key, is_action))
                .or_default()
                .push((var_name, field));
        }

        // ★ Phase 1b: Batched Redis fetch using HMGET (single RTT per key)
        for ((key, _is_action), var_fields) in redis_requests {
            let fields: Vec<&str> = var_fields.iter().map(|(_, f)| f.as_str()).collect();
            match self.rtdb.hash_mget(&key, &fields).await {
                Ok(results) => {
                    for (i, (var_name, field)) in var_fields.into_iter().enumerate() {
                        let val = results
                            .get(i)
                            .and_then(|opt| opt.as_ref())
                            .and_then(|bytes| {
                                let s = String::from_utf8_lossy(bytes);
                                s.parse::<f64>().ok()
                            })
                            .unwrap_or_else(|| {
                                tracing::warn!(
                                    "Var {}: {}:{} not found or invalid",
                                    var_name,
                                    key,
                                    field
                                );
                                0.0
                            });
                        values_changed |= values.insert(var_name, val) != Some(val);
                    }
                },
                Err(e) => {
                    tracing::error!("Redis HMGET error for {}: {}", key, e);
                    for (var_name, _) in var_fields {
                        values_changed |= values.insert(var_name, 0.0) != Some(0.0);
                    }
                },
            }
        }

        // ★ Phase 2: Calculate formula variables (depend on base variables)
        // Formula variables use RPN (Reverse Polish Notation) like ["X1", "X2", "+", 2, "*"]
        for var in variables {
            if var.formula.is_empty() {
                continue; // Skip non-formula variables (already handled above)
            }

            let var_name = var.name.clone();
            match evaluate_rpn_formula(&var.formula, values) {
                Some(result) => {
                    values_changed |= values.insert(var_name, result) != Some(result);
                },
                None => {
                    tracing::warn!(
                        "Failed to evaluate formula for variable '{}', using 0.0",
                        var_name
                    );
                    values_changed |= values.insert(var_name, 0.0) != Some(0.0);
                },
            }
        }

        Ok(values_changed)
    }

    /// Evaluate compact switch rules and return the next node ID with matched condition and port
    ///
    /// Returns: (next_node_id, matched_port, matched_condition_expression)
    fn evaluate_rule_switch_with_details<'a>(
        &self,
        rules: &[RuleSwitchBranch],
        wires: &'a HashMap<String, Vec<String>>,
        values: &HashMap<String, f64>,
    ) -> (Option<&'a str>, Option<String>, Option<String>) {
        for rule in rules {
            if self.evaluate_flow_conditions(&rule.rule, values) {
                // Format the matched condition expression
                let condition_str = format_conditions(&rule.rule);

                // Find the wire target for this rule's output
                if let Some(targets) = wires.get(&rule.name) {
                    if let Some(target) = targets.first() {
                        return (
                            Some(target.as_str()),
                            Some(rule.name.clone()),
                            Some(condition_str),
                        );
                    }
                }
            }
        }
        (None, None, None)
    }

    /// Evaluate all switch conditions and return results for each branch
    ///
    /// This is used for debugging/visualization to show which conditions matched/failed
    fn evaluate_all_conditions(
        &self,
        rules: &[RuleSwitchBranch],
        values: &HashMap<String, f64>,
    ) -> Vec<ConditionResult> {
        rules
            .iter()
            .map(|rule| {
                let result = self.evaluate_flow_conditions(&rule.rule, values);
                let expression = format_conditions(&rule.rule);
                ConditionResult {
                    expression,
                    result,
                    port: rule.name.clone(),
                }
            })
            .collect()
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

        // Variable must exist in values, otherwise condition fails
        let left = match values.get(var_name) {
            Some(&v) => v,
            None => {
                tracing::warn!(
                    "Variable '{}' not found in values, condition fails",
                    var_name
                );
                return false;
            },
        };
        let right = match &cond.value {
            Some(v) => {
                if let Some(n) = v.as_f64() {
                    n
                } else if let Some(n) = v.as_i64() {
                    n as f64
                } else if let Some(s) = v.as_str() {
                    // Could be a variable reference - must exist if referenced
                    match values.get(s) {
                        Some(&v) => v,
                        None => match s.parse::<f64>() {
                            Ok(n) => n,
                            Err(_) => {
                                tracing::warn!("Variable '{}' not found and not a number", s);
                                return false;
                            },
                        },
                    }
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
        let raw_value: f64 = if let Some(n) = assignment.value.as_f64() {
            n
        } else if let Some(n) = assignment.value.as_i64() {
            n as f64
        } else if let Some(s) = assignment.value.as_str() {
            // Could be a variable reference
            values.get(s).copied().unwrap_or(s.parse().unwrap_or(0.0))
        } else {
            0.0
        };

        // Sanitize value to prevent NaN/Infinity from reaching devices
        let config = ValidationConfig::default();
        let resolved_value = sanitize_value(raw_value, 0.0, &config);
        if (raw_value - resolved_value).abs() > f64::EPSILON || raw_value.is_nan() {
            tracing::warn!(
                "Rule action value sanitized: {} → {} (variable '{}')",
                raw_value,
                resolved_value,
                variable.name
            );
        }

        let Some(instance_id) = variable.instance else {
            tracing::error!(
                "Rule action skipped: variable '{}' missing instance_id",
                variable.name
            );
            return ActionResult {
                target_type: "instance",
                target_id: 0,
                point_type: point_type_to_static(variable.point_type.as_deref(), "A"),
                point_id: 0,
                value: resolved_value,
                success: false,
            };
        };

        let Some(point) = variable.point else {
            tracing::error!(
                "Rule action skipped: variable '{}' missing point_id (instance_id={})",
                variable.name,
                instance_id
            );
            return ActionResult {
                target_type: "instance",
                target_id: instance_id,
                point_type: point_type_to_static(variable.point_type.as_deref(), "A"),
                point_id: 0,
                value: resolved_value,
                success: false,
            };
        };

        // Use voltage_routing to set the action point
        // Use precomputed pool for common point IDs (0-255)
        let point_str = precomputed::get_point_id_str_or_alloc(point);
        let routed = match set_action_point(
            self.rtdb.as_ref(),
            &self.routing_cache,
            instance_id,
            &point_str,
            resolved_value,
        )
        .await
        {
            Ok(outcome) => outcome.routed,
            Err(e) => {
                tracing::error!(
                    "Rule action write failed (instance_id={}, point_id={}): {}",
                    instance_id,
                    point,
                    e
                );
                false
            },
        };

        ActionResult {
            target_type: "instance",
            target_id: instance_id,
            point_type: point_type_to_static(variable.point_type.as_deref(), "A"),
            point_id: point,
            value: resolved_value,
            success: routed,
        }
    }

    /// Write calculation result to instance point
    ///
    /// Supports both measurement (M) and action (A) point types:
    /// - Measurement: Direct write to inst:{id}:M Hash
    /// - Action: Use M2C routing (triggers comsrv TODO queue)
    async fn write_calculation_result(
        &self,
        variable: &RuleVariable,
        raw_value: f64,
        calc: &CalculationRule,
    ) -> ActionResult {
        // Sanitize calculation result to prevent NaN/Infinity propagation
        let config = ValidationConfig::default();
        let value = sanitize_value(raw_value, 0.0, &config);
        if (raw_value - value).abs() > f64::EPSILON || raw_value.is_nan() {
            tracing::warn!(
                "Calc output '{}' sanitized: {} → {} (formula='{}')",
                calc.output,
                raw_value,
                value,
                calc.formula
            );
        }
        let Some(instance_id) = variable.instance else {
            tracing::error!(
                "Calc output skipped: variable '{}' missing instance_id (output='{}')",
                variable.name,
                calc.output
            );
            return ActionResult {
                target_type: "instance",
                target_id: 0,
                point_type: point_type_to_static(variable.point_type.as_deref(), "M"),
                point_id: 0,
                value,
                success: false,
            };
        };

        let Some(point) = variable.point else {
            tracing::error!(
                "Calc output skipped: variable '{}' missing point_id (instance_id={}, output='{}')",
                variable.name,
                instance_id,
                calc.output
            );
            return ActionResult {
                target_type: "instance",
                target_id: instance_id,
                point_type: point_type_to_static(variable.point_type.as_deref(), "M"),
                point_id: 0,
                value,
                success: false,
            };
        };
        let point_type = variable.point_type.as_deref().unwrap_or("M");

        let success = match point_type {
            "M" | "measurement" => {
                // Direct write to measurement hash (no routing)
                self.write_measurement_point(instance_id, point, value)
                    .await
                    .is_ok()
            },
            "A" | "action" => {
                // Use M2C routing for action points
                // Use precomputed pool for common point IDs (0-255)
                let point_str = precomputed::get_point_id_str_or_alloc(point);
                match set_action_point(
                    self.rtdb.as_ref(),
                    &self.routing_cache,
                    instance_id,
                    &point_str,
                    value,
                )
                .await
                {
                    Ok(outcome) => outcome.routed,
                    Err(e) => {
                        tracing::error!(
                            "Calc action write failed (instance_id={}, point_id={}): {}",
                            instance_id,
                            point,
                            e
                        );
                        false
                    },
                }
            },
            _ => {
                tracing::warn!(
                    "Unknown point type '{}' for calc output '{}'",
                    point_type,
                    calc.output
                );
                false
            },
        };

        ActionResult {
            target_type: "instance",
            target_id: instance_id,
            point_type: point_type_to_static(Some(point_type), "M"),
            point_id: point,
            value,
            success,
        }
    }

    /// Write period delta result to instance point
    ///
    /// Similar to write_calculation_result but specifically for PeriodDelta nodes.
    /// Always writes to measurement points (period deltas are derived values).
    async fn write_period_delta_result(
        &self,
        variable: &RuleVariable,
        raw_value: f64,
        period: &str,
    ) -> ActionResult {
        // Sanitize period delta result to prevent NaN/Infinity propagation
        let config = ValidationConfig::default();
        let value = sanitize_value(raw_value, 0.0, &config);
        if (raw_value - value).abs() > f64::EPSILON || raw_value.is_nan() {
            tracing::warn!(
                "PeriodDelta output '{}' sanitized: {} → {} (period='{}')",
                variable.name,
                raw_value,
                value,
                period
            );
        }
        let Some(instance_id) = variable.instance else {
            tracing::error!(
                "PeriodDelta output skipped: variable '{}' missing instance_id (period='{}')",
                variable.name,
                period
            );
            return ActionResult {
                target_type: "instance",
                target_id: 0,
                point_type: "M",
                point_id: 0,
                value,
                success: false,
            };
        };

        let Some(point) = variable.point else {
            tracing::error!(
                "PeriodDelta output skipped: variable '{}' missing point_id (instance_id={}, period='{}')",
                variable.name,
                instance_id,
                period
            );
            return ActionResult {
                target_type: "instance",
                target_id: instance_id,
                point_type: "M",
                point_id: 0,
                value,
                success: false,
            };
        };

        // Period delta results are always measurement points (derived values)
        let success = self
            .write_measurement_point(instance_id, point, value)
            .await
            .is_ok();

        tracing::debug!(
            "PeriodDelta write: inst:{}:M:{} = {} (period={})",
            instance_id,
            point,
            value,
            period
        );

        ActionResult {
            target_type: "instance",
            target_id: instance_id,
            point_type: "M",
            point_id: point,
            value,
            success,
        }
    }

    /// Write directly to measurement point (no routing)
    ///
    /// Used by calculation nodes to write computed values back to measurement points.
    /// This enables use cases like energy accumulation (kWh from power readings).
    ///
    /// Uses precomputed point ID pool and ryu for zero-allocation formatting.
    async fn write_measurement_point(
        &self,
        instance_id: u32,
        point: u32,
        value: f64,
    ) -> Result<()> {
        let config = KeySpaceConfig::production();

        // Write to inst:{id}:M Hash
        // Use precomputed pool for common point IDs (0-255)
        let key = config.instance_measurement_key(instance_id);
        let point_str = precomputed::get_point_id_str_or_alloc(point);
        let value_bytes = voltage_rtdb::numfmt::f64_to_bytes(value);
        self.rtdb
            .hash_set(&key, &point_str, value_bytes)
            .await
            .map_err(|e| crate::error::RuleError::ExecutionError(e.to_string()))?;

        tracing::debug!("Calc write: inst:{}:M:{} = {}", instance_id, point, value);

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use serde_json::json;
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
        let (next, port, condition) =
            executor.evaluate_rule_switch_with_details(&rules, &wires, &values);
        assert_eq!(next, Some("node-high"));
        assert_eq!(port, Some("out002".to_string()));
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
                                "instance": 5,
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
                                "instance": 6,
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
                                "instance": 7,
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
                                "instance": 6,
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
            id: 1,
            name: "SOC Strategy".to_string(),
            description: None,
            enabled: true,
            priority: 0,
            cooldown_ms: 0,
            trigger_config: None,
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
        assert_eq!(result.actions_executed[0].value, 999.0);
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
        assert_eq!(result.actions_executed[0].value, 1.0);
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
            instance: Some(100), // Use numeric instance ID
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

    // Note: VecRtdb tests removed - now using two-tier architecture (SharedMemory → Redis)

    /// Test: executor reads directly from Redis when no cache layer
    #[tokio::test]
    async fn test_read_variables_no_cache_uses_redis() {
        let rtdb = Arc::new(MemoryRtdb::new());
        let routing_cache = Arc::new(RoutingCache::default());

        // Setup Redis data
        rtdb.hash_set("inst:10:M", "1", Bytes::from("55.5"))
            .await
            .unwrap();

        // No VecRtdb configured
        let executor = RuleExecutor::new(rtdb, routing_cache);

        let variables = vec![RuleVariable {
            name: "DIRECT".to_string(),
            instance: Some(10),
            point_type: Some("measurement".to_string()),
            point: Some(1),
            formula: vec![],
        }];

        let mut values = HashMap::new();
        executor
            .read_rule_variables(&variables, &mut values)
            .await
            .unwrap();

        // Should read directly from Redis
        assert_eq!(values.get("DIRECT"), Some(&55.5));
    }

    // ============================================================================
    // RPN Formula Calculation Tests
    // ============================================================================

    #[test]
    fn test_rpn_simple_addition() {
        // X1 + X2 = 10 + 20 = 30
        let formula = vec![json!("X1"), json!("X2"), json!("+")];
        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);
        values.insert("X2".to_string(), 20.0);

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, Some(30.0));
    }

    #[test]
    fn test_rpn_complex_expression() {
        // (X1 + X2) * 2 = (10 + 20) * 2 = 60
        let formula = vec![json!("X1"), json!("X2"), json!("+"), json!(2), json!("*")];
        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);
        values.insert("X2".to_string(), 20.0);

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, Some(60.0));
    }

    #[test]
    fn test_rpn_all_operators() {
        // a + b - c * d / e
        // In RPN: a b + c d * e / -
        // With values: 10 + 5 = 15, 6 * 4 = 24, 24 / 2 = 12, 15 - 12 = 3
        let formula = vec![
            json!("a"),
            json!("b"),
            json!("+"),
            json!("c"),
            json!("d"),
            json!("*"),
            json!("e"),
            json!("/"),
            json!("-"),
        ];
        let mut values = HashMap::new();
        values.insert("a".to_string(), 10.0);
        values.insert("b".to_string(), 5.0);
        values.insert("c".to_string(), 6.0);
        values.insert("d".to_string(), 4.0);
        values.insert("e".to_string(), 2.0);

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, Some(3.0));
    }

    #[test]
    fn test_rpn_division_by_zero() {
        // X1 / 0 should return None
        let formula = vec![json!("X1"), json!(0), json!("/")];
        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, None);
    }

    #[test]
    fn test_rpn_undefined_variable() {
        // Reference to undefined variable should return None
        let formula = vec![json!("X1"), json!("UNDEFINED"), json!("+")];
        let mut values = HashMap::new();
        values.insert("X1".to_string(), 10.0);

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, None);
    }

    #[test]
    fn test_rpn_numeric_literals() {
        // Pure numeric calculation: 5 + 3 * 2 = 5 + 6 = 11
        // In RPN: 5 3 2 * +
        let formula = vec![json!(5), json!(3), json!(2), json!("*"), json!("+")];
        let values = HashMap::new();

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, Some(11.0));
    }

    #[test]
    fn test_rpn_float_precision() {
        // Float calculation: 1.5 + 2.5 = 4.0
        let formula = vec![json!(1.5), json!(2.5), json!("+")];
        let values = HashMap::new();

        let result = evaluate_rpn_formula(&formula, &values);
        assert_eq!(result, Some(4.0));
    }
}
