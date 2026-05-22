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
    RuleVariable, RuleWires,
};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use voltage_calc::{CalcEngine, MemoryStateStore, StateStore};
use voltage_model::{PointType, ValidationConfig, validate_value};
use voltage_routing::{
    RoutingCache, route_context_from_target, set_action_point_with_target,
    set_action_point_with_target_at, validate_action_value,
};
use voltage_rtdb::numfmt::precomputed;
use voltage_rtdb::traits::Rtdb;
use voltage_rtdb::{KeySpaceConfig, SystemTimeProvider, TimeProvider};
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

/// Validate variable fields and sanitize value for write operations.
///
/// Common validation shared by `execute_rule_change`, `write_calculation_result`,
/// and `write_period_delta_result`. Returns `(instance_id, point_id, sanitized_value, point_type)`
/// or a failed `ActionResult` if validation fails.
fn validate_write_target(
    variable: &RuleVariable,
    raw_value: f64,
    default_point_type: &'static str,
    context: &str,
) -> std::result::Result<(u32, u32, f64, &'static str), ActionResult> {
    let config = ValidationConfig::default();
    let pt = point_type_to_static(variable.point_type.as_deref(), default_point_type);

    // Reject NaN/Inf/out-of-range — never silently coerce to 0.0. The old
    // sanitize_value path turned a malformed compute result into a real
    // SCADA write of 0, which is the exact failure mode this skill is
    // sealing across the codebase. Caller treats Err as "skip this action".
    let value = match validate_value(raw_value, &config) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "{} skipped: value {} failed validation ({}) (variable '{}')",
                context,
                raw_value,
                e,
                variable.name
            );
            return Err(ActionResult {
                target_type: "instance",
                target_id: variable.instance.unwrap_or(0),
                point_type: pt,
                point_id: variable.point.unwrap_or(0),
                value: f64::NAN,
                success: false,
            });
        },
    };

    let instance_id = variable.instance.ok_or_else(|| {
        tracing::error!(
            "{} skipped: variable '{}' missing instance_id",
            context,
            variable.name
        );
        ActionResult {
            target_type: "instance",
            target_id: 0,
            point_type: pt,
            point_id: 0,
            value,
            success: false,
        }
    })?;

    let point = variable.point.ok_or_else(|| {
        tracing::error!(
            "{} skipped: variable '{}' missing point_id (instance_id={})",
            context,
            variable.name,
            instance_id
        );
        ActionResult {
            target_type: "instance",
            target_id: instance_id,
            point_type: pt,
            point_id: 0,
            value,
            success: false,
        }
    })?;

    Ok((instance_id, point, value, pt))
}

/// Create or reuse a cached snapshot of variable values.
///
/// Returns the cached `Arc` if `values_changed` is false, otherwise creates a new one.
fn snapshot_or_reuse(
    cache: &mut Option<Arc<HashMap<String, f64>>>,
    values: &HashMap<String, f64>,
    values_changed: bool,
) -> Arc<HashMap<String, f64>> {
    if !values_changed && let Some(snapshot) = cache.as_ref() {
        return Arc::clone(snapshot);
    }
    let snapshot = Arc::new(values.clone());
    *cache = Some(Arc::clone(&snapshot));
    snapshot
}

/// Evaluate a formula in Reverse Polish Notation (RPN)
/// Convert a JSON token array `["X1", "+", "X2"]` to an expression string `"X1 + X2"`,
/// then evaluate via the shared `formula::evaluate_formula` engine (evalexpr).
fn evaluate_token_formula(
    tokens: &[serde_json::Value],
    values: &HashMap<String, f64>,
) -> Option<f64> {
    // Build expression string from tokens
    let expr: String = tokens
        .iter()
        .map(|t| match t {
            serde_json::Value::String(s) => Cow::Borrowed(s.as_str()),
            serde_json::Value::Number(n) => Cow::Owned(n.to_string()),
            other => Cow::Owned(other.to_string()),
        })
        .collect::<Vec<_>>()
        .join(" ");

    match crate::formula::evaluate_formula(&expr, values) {
        Ok(val) => Some(val),
        Err(e) => {
            tracing::warn!("Formula '{}' evaluation failed: {}", expr, e);
            None
        },
    }
}

/// Build an ActionResult marking that a rule action was skipped because the
/// resolved value couldn't be determined (missing variable, unsupported type).
/// Carries the variable's identity so caller and logs can attribute the skip.
fn action_skipped(variable: &RuleVariable, reason: &str) -> ActionResult {
    tracing::warn!(
        "Rule action skipped (variable '{}', instance={:?}, point={:?}): {}",
        variable.name,
        variable.instance,
        variable.point,
        reason
    );
    ActionResult {
        target_type: "instance",
        target_id: variable.instance.unwrap_or(0),
        point_type: point_type_to_static(variable.point_type.as_deref(), "A"),
        point_id: variable.point.unwrap_or(0),
        value: f64::NAN,
        success: false,
    }
}

/// Outcome of one variable-read pass for a node.
///
/// Captures both "did anything change?" (for snapshot-cache reuse) and
/// "which variables were unavailable?" (so callers can skip evaluation
/// instead of substituting 0.0 for absent readings — silently triggering
/// conditions like "current < threshold" on missing data).
#[derive(Debug, Default)]
pub(crate) struct RuleReadOutcome {
    pub values_changed: bool,
    /// Variables whose data was unavailable this cycle (no SHM hit, no Redis
    /// field, or RPC failure). Caller MUST short-circuit rule evaluation when
    /// non-empty.
    pub missing: Vec<String>,
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
/// Uses SharedMemory (~5μs, cross-process mmap) with Redis (~1ms) as remote fallback.
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
    pub fn with_shared_reader(mut self, reader: Arc<UnifiedReader>) -> Self {
        self.shared_reader = Some(reader);
        self
    }

    /// Enable UnifiedWriter for M2C via shared memory
    ///
    /// When enabled, action outputs are written to SHM in addition to Redis.
    /// SHM serves as the source for comsrv's ShmCommandListener (UDS event-driven dispatch).
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

        let mut values_snapshot: Option<Arc<HashMap<String, f64>>> = None;

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
                    // Read node-local variables. Missing variables → skip this
                    // cycle so absent data can never trigger threshold rules
                    // (a 0.0 fallback would fire "X1 < 5" on missing readings).
                    let outcome = match self.read_rule_variables(variables, &mut values).await {
                        Ok(o) => o,
                        Err(e) => {
                            result.error = Some(format!("Failed to read variables: {}", e));
                            result.variable_values = Arc::new(std::mem::take(&mut values));
                            return Ok(result);
                        },
                    };
                    if !outcome.missing.is_empty() {
                        result.error = Some(format!(
                            "Rule cycle skipped: variables unavailable: {}",
                            outcome.missing.join(", ")
                        ));
                        result.variable_values = Arc::new(std::mem::take(&mut values));
                        return Ok(result);
                    }
                    let values_changed = outcome.values_changed;

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
                    // Read target variables. Skip the cycle on missing data —
                    // a 0.0 fallback would write meaningless action values.
                    let outcome = match self.read_rule_variables(variables, &mut values).await {
                        Ok(o) => o,
                        Err(e) => {
                            result.error = Some(format!("Failed to read variables: {}", e));
                            return Ok(result);
                        },
                    };
                    if !outcome.missing.is_empty() {
                        result.error = Some(format!(
                            "Rule cycle skipped: variables unavailable: {}",
                            outcome.missing.join(", ")
                        ));
                        return Ok(result);
                    }
                    let values_changed = outcome.values_changed;

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
                } => match self
                    .handle_calculation_node(
                        current_id,
                        variables,
                        calculations,
                        wires,
                        &mut values,
                        &mut values_snapshot,
                        &mut result,
                        rule.id,
                    )
                    .await
                {
                    Some(next) => current_id = next,
                    None => return Ok(result),
                },
                RuleNode::PeriodDelta {
                    input,
                    output,
                    period,
                    wires,
                } => match self
                    .handle_period_delta_node(
                        current_id,
                        input,
                        output,
                        period,
                        wires,
                        &mut values,
                        &mut values_snapshot,
                        &mut result,
                        rule.id,
                    )
                    .await
                {
                    Some(next) => current_id = next,
                    None => return Ok(result),
                },
            }
        }

        Ok(result)
    }

    /// Handle Calculation node: evaluate formulas and write results
    #[allow(clippy::too_many_arguments)]
    async fn handle_calculation_node<'a>(
        &self,
        node_id: &str,
        variables: &[RuleVariable],
        calculations: &[CalculationRule],
        wires: &'a RuleWires,
        values: &mut HashMap<String, f64>,
        snapshot_cache: &mut Option<Arc<HashMap<String, f64>>>,
        result: &mut RuleExecutionResult,
        rule_id: i64,
    ) -> Option<&'a str> {
        let outcome = match self.read_rule_variables(variables, values).await {
            Ok(o) => o,
            Err(e) => {
                result.error = Some(format!("Failed to read variables: {}", e));
                return None;
            },
        };
        if !outcome.missing.is_empty() {
            result.error = Some(format!(
                "Calculation skipped: variables unavailable: {}",
                outcome.missing.join(", ")
            ));
            return None;
        }
        let values_changed = outcome.values_changed;

        let input_snapshot = snapshot_or_reuse(snapshot_cache, values, values_changed);
        result.variable_values = Arc::clone(&input_snapshot);

        let calc_engine =
            CalcEngine::new(Arc::clone(&self.state_store), format!("rule_{}", rule_id));
        let mut node_actions = Vec::new();

        for calc in calculations {
            let calc_result = match calc_engine.evaluate(&calc.formula, values).await {
                Ok(v) => v,
                Err(e) => {
                    result.error = Some(format!("Calc '{}' error: {}", calc.formula, e));
                    return None;
                },
            };

            if let Some(var) = variables.iter().find(|v| v.name == calc.output) {
                let action = self.write_calculation_result(var, calc_result, calc).await;
                node_actions.push(action);
                result.actions_executed.push(action);
            }

            values.insert(calc.output.clone(), calc_result);
        }

        *snapshot_cache = None; // Values modified; invalidate cache
        result.node_details.insert(
            node_id.to_string(),
            NodeExecutionDetail {
                node_type: "calculation",
                input_values: input_snapshot,
                condition_results: None,
                matched_port: None,
                actions: Some(node_actions),
            },
        );

        match wires.default.first() {
            Some(next) => Some(next.as_str()),
            None => {
                result.error = Some("Calculation node has no output wire".to_string());
                None
            },
        }
    }

    /// Handle PeriodDelta node: calculate period delta and write result
    #[allow(clippy::too_many_arguments)]
    async fn handle_period_delta_node<'a>(
        &self,
        node_id: &str,
        input: &RuleVariable,
        output: &RuleVariable,
        period: &str,
        wires: &'a RuleWires,
        values: &mut HashMap<String, f64>,
        snapshot_cache: &mut Option<Arc<HashMap<String, f64>>>,
        result: &mut RuleExecutionResult,
        rule_id: i64,
    ) -> Option<&'a str> {
        let input_vars = vec![input.clone()];
        let outcome = match self.read_rule_variables(&input_vars, values).await {
            Ok(o) => o,
            Err(e) => {
                result.error = Some(format!("Failed to read input variable: {}", e));
                return None;
            },
        };
        if !outcome.missing.is_empty() {
            result.error = Some(format!(
                "PeriodDelta skipped: input variable unavailable: {}",
                outcome.missing.join(", ")
            ));
            return None;
        }
        let values_changed = outcome.values_changed;

        let input_snapshot = snapshot_or_reuse(snapshot_cache, values, values_changed);
        result.variable_values = Arc::clone(&input_snapshot);

        let input_value = match values.get(&input.name).copied() {
            Some(v) if v.is_finite() => v,
            Some(v) => {
                result.error = Some(format!(
                    "PeriodDelta skipped: input variable '{}' is non-finite ({})",
                    input.name, v
                ));
                return None;
            },
            None => {
                result.error = Some(format!(
                    "PeriodDelta skipped: input variable '{}' unavailable after read",
                    input.name
                ));
                return None;
            },
        };
        let calc_engine =
            CalcEngine::new(Arc::clone(&self.state_store), format!("rule_{}", rule_id));

        let state_key = format!(
            "{}:{}:{}",
            rule_id,
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
                return None;
            },
        };

        let action = self.write_period_delta_result(output, delta, period).await;
        result.actions_executed.push(action);

        values.insert(output.name.clone(), delta);
        *snapshot_cache = None; // Invalidate cache

        result.node_details.insert(
            node_id.to_string(),
            NodeExecutionDetail {
                node_type: "periodDelta",
                input_values: input_snapshot,
                condition_results: None,
                matched_port: None,
                actions: Some(vec![action]),
            },
        );

        match wires.default.first() {
            Some(next) => Some(next.as_str()),
            None => {
                result.error = Some("PeriodDelta node has no output wire".to_string());
                None
            },
        }
    }

    /// Read variables from RTDB with two-tier priority
    ///
    /// Priority order:
    /// 1. SharedMemory (~5μs) - cross-process mmap, highest priority
    /// 2. Redis (~1ms) - remote fallback
    ///
    /// Reads variable values from Redis Hash `inst:{id}:M` or `inst:{id}:A`
    async fn read_rule_variables(
        &self,
        variables: &[RuleVariable],
        values: &mut HashMap<String, f64>,
    ) -> Result<RuleReadOutcome> {
        let mut outcome = RuleReadOutcome::default();
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
                    if !val.is_finite() {
                        if is_action {
                            tracing::trace!(
                                "Action var {} from SHM is non-finite ({}) — leaving unset",
                                var_name,
                                val
                            );
                        } else {
                            tracing::warn!(
                                "Var {} from SHM is non-finite ({}) — rule will be skipped this cycle",
                                var_name,
                                val
                            );
                            outcome.missing.push(var_name);
                        }
                        continue;
                    }

                    // SharedMemory hit - fastest path.
                    // total_cmp avoids NaN != NaN busting the Arc snapshot every cycle.
                    outcome.values_changed |= values
                        .insert(var_name, val)
                        .is_none_or(|prev| prev.total_cmp(&val).is_ne());
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
        for ((key, is_action), var_fields) in redis_requests {
            let fields: Vec<&str> = var_fields.iter().map(|(_, f)| f.as_str()).collect();
            match self.rtdb.hash_mget(&key, &fields).await {
                Ok(results) => {
                    for (i, (var_name, field)) in var_fields.into_iter().enumerate() {
                        let parsed =
                            results
                                .get(i)
                                .and_then(|opt| opt.as_ref())
                                .and_then(|bytes| {
                                    let s = String::from_utf8_lossy(bytes);
                                    s.parse::<f64>().ok()
                                });
                        match parsed {
                            Some(val) if val.is_finite() => {
                                outcome.values_changed |= values
                                    .insert(var_name, val)
                                    .is_none_or(|prev| prev.total_cmp(&val).is_ne());
                            },
                            Some(val) if is_action => {
                                tracing::trace!(
                                    "Action var {} ({}:{}) is non-finite ({}) — leaving unset",
                                    var_name,
                                    key,
                                    field,
                                    val
                                );
                            },
                            Some(val) => {
                                tracing::warn!(
                                    "Var {}: {}:{} is non-finite ({}) — rule will be skipped this cycle",
                                    var_name,
                                    key,
                                    field,
                                    val
                                );
                                outcome.missing.push(var_name);
                            },
                            None if is_action => {
                                // Action point variables are write targets; "never
                                // written before" is the normal initial state. Don't
                                // treat as missing — the rule may write to them.
                                tracing::trace!(
                                    "Action var {} ({}:{}) not yet written — leaving unset",
                                    var_name,
                                    key,
                                    field
                                );
                            },
                            None => {
                                tracing::warn!(
                                    "Var {}: {}:{} unavailable — rule will be skipped this cycle",
                                    var_name,
                                    key,
                                    field
                                );
                                outcome.missing.push(var_name);
                            },
                        }
                    }
                },
                Err(e) => {
                    tracing::error!(
                        "Redis HMGET error for {} — variables marked missing: {}",
                        key,
                        e
                    );
                    // RPC failure on action keys is also "treat as unset" — we
                    // can't tell whether the field would have existed.
                    if !is_action {
                        for (var_name, _) in var_fields {
                            outcome.missing.push(var_name);
                        }
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
            match evaluate_token_formula(&var.formula, values) {
                Some(result) => {
                    outcome.values_changed |= values.insert(var_name, result) != Some(result);
                },
                None => {
                    tracing::warn!(
                        "Formula variable '{}' could not be evaluated — rule will be skipped",
                        var_name
                    );
                    outcome.missing.push(var_name);
                },
            }
        }

        Ok(outcome)
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
                if let Some(targets) = wires.get(&rule.name)
                    && let Some(target) = targets.first()
                {
                    return (
                        Some(target.as_str()),
                        Some(rule.name.clone()),
                        Some(condition_str),
                    );
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
        // Resolve the value to write. No 0.0 fallback — silently writing 0
        // when an assignment references a missing variable would corrupt
        // control commands (e.g. "set Y = X1" when X1 is unavailable).
        let raw_value: f64 = if let Some(n) = assignment.value.as_f64() {
            n
        } else if let Some(n) = assignment.value.as_i64() {
            n as f64
        } else if let Some(s) = assignment.value.as_str() {
            // String form is either (a) a numeric literal or (b) a variable name.
            if let Ok(n) = s.parse::<f64>() {
                n
            } else if let Some(&v) = values.get(s) {
                v
            } else {
                tracing::warn!(
                    "Rule action skipped: assignment value '{}' is neither a numeric \
                     literal nor a known variable",
                    s
                );
                return action_skipped(variable, "unknown variable in assignment");
            }
        } else {
            tracing::warn!(
                "Rule action skipped: assignment value type unsupported: {:?}",
                assignment.value
            );
            return action_skipped(variable, "unsupported assignment value type");
        };

        let (instance_id, point, value, pt) =
            match validate_write_target(variable, raw_value, "A", "Rule action") {
                Ok(v) => v,
                Err(action) => return action,
            };
        self.write_to_point(instance_id, point, value, pt, "Rule action")
            .await
    }

    /// Write a validated value to the appropriate point (M or A) and build ActionResult
    async fn write_to_point(
        &self,
        instance_id: u32,
        point: u32,
        value: f64,
        pt: &'static str,
        context: &str,
    ) -> ActionResult {
        let success = match pt {
            "M" => self
                .write_measurement_point(instance_id, point, value)
                .await
                .is_ok(),
            "A" => {
                let point_str = precomputed::get_point_id_str_or_alloc(point);
                self.write_action_point(instance_id, point, &point_str, value, context)
                    .await
            },
            _ => {
                tracing::warn!("Unknown point type '{}' for {}", pt, context);
                false
            },
        };

        ActionResult {
            target_type: "instance",
            target_id: instance_id,
            point_type: pt,
            point_id: point,
            value,
            success,
        }
    }

    async fn write_action_point(
        &self,
        instance_id: u32,
        point: u32,
        point_str: &str,
        value: f64,
        context: &str,
    ) -> bool {
        let target =
            self.routing_cache
                .lookup_m2c_by_parts(instance_id, PointType::Adjustment, point);

        let Some(target) = target else {
            return match set_action_point_with_target(
                self.rtdb.as_ref(),
                instance_id,
                point_str,
                value,
                None,
            )
            .await
            {
                Ok(outcome) => outcome.routed,
                Err(e) => {
                    tracing::error!(
                        "{} write failed (instance_id={}, point_id={}): {}",
                        context,
                        instance_id,
                        point,
                        e
                    );
                    false
                },
            };
        };

        let value = match validate_action_value(instance_id, point_str, value) {
            Ok(value) => value,
            Err(e) => {
                tracing::error!(
                    "{} write rejected (instance_id={}, point_id={}): {}",
                    context,
                    instance_id,
                    point,
                    e
                );
                return false;
            },
        };
        let timestamp_ms = SystemTimeProvider.now_millis();
        let ctx = route_context_from_target(target, timestamp_ms);

        if !self.dispatch_action_to_comsrv(&ctx, value, context).await {
            return false;
        }

        match set_action_point_with_target_at(
            self.rtdb.as_ref(),
            instance_id,
            point_str,
            value,
            Some(target),
            timestamp_ms,
        )
        .await
        {
            Ok(outcome) => outcome.routed,
            Err(e) => {
                tracing::error!(
                    "{} commit failed after dispatch (instance_id={}, point_id={}): {}",
                    context,
                    instance_id,
                    point,
                    e
                );
                false
            },
        }
    }

    async fn dispatch_action_to_comsrv(
        &self,
        ctx: &voltage_routing::RouteContext,
        value: f64,
        context: &str,
    ) -> bool {
        let Some(writer) = self.shm_action_writer.as_ref() else {
            tracing::error!(
                "{} dispatch failed: SHM writer unavailable for ch={} pt={} point={}",
                context,
                ctx.target_channel_id,
                ctx.target_point_type,
                ctx.target_point_id
            );
            return false;
        };

        if !writer.set_action(
            ctx.target_channel_id,
            ctx.target_point_type,
            ctx.target_point_id,
            value,
            ctx.timestamp_ms as u64,
        ) {
            tracing::error!(
                "{} dispatch failed: SHM action slot missing for ch={} pt={} point={}",
                context,
                ctx.target_channel_id,
                ctx.target_point_type,
                ctx.target_point_id
            );
            return false;
        }

        let Some(notifier) = self.shm_notifier.as_ref() else {
            tracing::error!(
                "{} dispatch failed: UDS notifier unavailable for ch={} pt={} point={}",
                context,
                ctx.target_channel_id,
                ctx.target_point_type,
                ctx.target_point_id
            );
            return false;
        };

        let mut guard =
            match tokio::time::timeout(Duration::from_millis(100), notifier.lock()).await {
                Ok(guard) => guard,
                Err(_) => {
                    tracing::error!(
                        "{} dispatch failed: UDS notifier lock timeout for ch={} pt={} point={}",
                        context,
                        ctx.target_channel_id,
                        ctx.target_point_type,
                        ctx.target_point_id
                    );
                    return false;
                },
            };

        let Some(point_type) = PointType::from_u8(ctx.target_point_type) else {
            tracing::error!(
                "{} dispatch failed: invalid point type {} for ch={} point={}",
                context,
                ctx.target_point_type,
                ctx.target_channel_id,
                ctx.target_point_id
            );
            return false;
        };

        let result = guard
            .notify(
                ctx.target_channel_id,
                point_type,
                ctx.target_point_id,
                value,
                ctx.timestamp_ms as u64,
            )
            .await;

        if result.uds_sent {
            true
        } else {
            tracing::error!(
                "{} dispatch failed: UDS notification not delivered for ch={} pt={:?} point={}",
                context,
                ctx.target_channel_id,
                point_type,
                ctx.target_point_id
            );
            false
        }
    }

    /// Write calculation result to instance point (M or A)
    async fn write_calculation_result(
        &self,
        variable: &RuleVariable,
        raw_value: f64,
        calc: &CalculationRule,
    ) -> ActionResult {
        let (instance_id, point, value, pt) = match validate_write_target(
            variable,
            raw_value,
            "M",
            &format!("Calc '{}'", calc.output),
        ) {
            Ok(v) => v,
            Err(action) => return action,
        };
        self.write_to_point(
            instance_id,
            point,
            value,
            pt,
            &format!("Calc '{}'", calc.output),
        )
        .await
    }

    /// Write period delta result to instance point (always measurement type)
    async fn write_period_delta_result(
        &self,
        variable: &RuleVariable,
        raw_value: f64,
        period: &str,
    ) -> ActionResult {
        let (instance_id, point, value, _pt) = match validate_write_target(
            variable,
            raw_value,
            "M",
            &format!("PeriodDelta({})", period),
        ) {
            Ok(v) => v,
            Err(action) => return action,
        };
        tracing::debug!(
            "PeriodDelta write: inst:{}:M:{} = {} (period={})",
            instance_id,
            point,
            value,
            period
        );
        self.write_to_point(
            instance_id,
            point,
            value,
            "M",
            &format!("PeriodDelta({})", period),
        )
        .await
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
        let key = config.instance_measurement_key(instance_id);
        let ts_key = config.instance_measurement_ts_key(instance_id);
        let point_str = precomputed::get_point_id_str_or_alloc(point);
        let value_bytes = voltage_rtdb::numfmt::f64_to_bytes(value);
        // Stamp ts in the same call as the value: OnChange triggers that
        // watch calculation outputs must see value + ts updated atomically
        // or the time deadband never expires.
        let now_ms = SystemTimeProvider.now_millis();
        let ts_bytes = voltage_rtdb::numfmt::i64_to_bytes(now_ms);

        self.rtdb
            .hash_set(&key, &point_str, value_bytes)
            .await
            .map_err(|e| crate::error::RuleError::ExecutionError(e.to_string()))?;
        self.rtdb
            .hash_set(&ts_key, &point_str, ts_bytes)
            .await
            .map_err(|e| crate::error::RuleError::ExecutionError(e.to_string()))?;

        tracing::debug!(
            "Calc write: inst:{}:M:{} = {} @{}",
            instance_id,
            point,
            value,
            now_ms
        );

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
#[path = "executor_tests.rs"]
mod tests;
