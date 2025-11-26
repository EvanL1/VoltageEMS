//! Vue Flow JSON Parser
//!
//! Parses frontend Vue Flow rule JSON into the flattened Rule structure
//! for execution by the rule engine.

use serde_json::Value;
use voltage_config::rulesrv::{
    ArithmeticOp, ChannelPointType, CompareOp, FlowNode, FormulaToken, InstancePointType, LogicOp,
    Rule, RuleCondition, SwitchRule, ValueChange, Variable,
};

use crate::error::{Result, RuleError};

/// Parsed flow result with extracted metadata
pub struct ParsedFlow {
    /// Parsed variables
    pub variables: Vec<Variable>,
    /// Parsed nodes
    pub nodes: Vec<FlowNode>,
    /// Start node ID
    pub start_node_id: String,
}

/// Parse Vue Flow JSON into Rule
pub fn parse_flow_json(flow_json: &Value) -> Result<ParsedFlow> {
    let nodes_array = flow_json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RuleError::ParseError("Missing 'nodes' array".to_string()))?;

    // Parse nodes and collect variables
    let mut flow_nodes = Vec::new();
    let mut variables = Vec::new();
    let mut start_node_id = String::new();

    for node in nodes_array {
        let node_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("custom");

        let node_id = node
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Node missing 'id'".to_string()))?
            .to_string();

        let data = node.get("data");

        match node_type {
            "start" => {
                start_node_id = node_id.clone();
                let next = get_wire_target(data, "default")?;
                flow_nodes.push(FlowNode::Start { id: node_id, next });
            },
            "end" => {
                flow_nodes.push(FlowNode::End { id: node_id });
            },
            "custom" => {
                if let Some(data) = data {
                    let inner_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    match inner_type {
                        "function-switch" => {
                            // Extract variables from this switch node
                            if let Some(config) = data.get("config") {
                                if let Some(vars) =
                                    config.get("variables").and_then(|v| v.as_array())
                                {
                                    for var in vars {
                                        if let Ok(v) = parse_variable(var) {
                                            // Avoid duplicates by variable name
                                            let new_name = get_variable_name(&v);
                                            if !variables.iter().any(|existing| {
                                                get_variable_name(existing) == new_name
                                            }) {
                                                variables.push(v);
                                            }
                                        }
                                    }
                                }

                                // Parse switch rules
                                let rules = parse_switch_rules(config)?;
                                flow_nodes.push(FlowNode::Switch { id: node_id, rules });
                            }
                        },
                        "action-changeValue" => {
                            let changes = if let Some(config) = data.get("config") {
                                parse_value_changes(config)?
                            } else {
                                vec![]
                            };
                            let next = get_wire_target(data.get("config"), "default")?;
                            flow_nodes.push(FlowNode::ChangeValue {
                                id: node_id,
                                changes,
                                next,
                            });
                        },
                        _ => {
                            // Unknown node type, skip
                            tracing::warn!("Unknown node type: {}", inner_type);
                        },
                    }
                }
            },
            _ => {
                tracing::warn!("Unknown top-level node type: {}", node_type);
            },
        }
    }

    if start_node_id.is_empty() {
        return Err(RuleError::ParseError(
            "No start node found in flow".to_string(),
        ));
    }

    Ok(ParsedFlow {
        variables,
        nodes: flow_nodes,
        start_node_id,
    })
}

/// Parse Vue Flow JSON into complete Rule structure
#[allow(dead_code)] // Reserved for future Vue Flow integration
pub fn parse_flow_to_rule(flow_json: &Value) -> Result<Rule> {
    let id = flow_json
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RuleError::ParseError("Missing 'id' field".to_string()))?
        .to_string();

    let name = flow_json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unnamed Rule")
        .to_string();

    let description = flow_json
        .get("description")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let parsed = parse_flow_json(flow_json)?;

    Ok(Rule {
        id,
        name,
        description,
        enabled: true,
        priority: 0,
        cooldown_ms: 0,
        variables: parsed.variables,
        nodes: parsed.nodes,
        start_node_id: parsed.start_node_id,
        flow_json: Some(flow_json.clone()),
    })
}

/// Get wire target node ID from config
fn get_wire_target(config: Option<&Value>, wire_name: &str) -> Result<String> {
    config
        .and_then(|c| c.get("config"))
        .or(config)
        .and_then(|c| c.get("wires"))
        .and_then(|w| w.get(wire_name))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| RuleError::ParseError(format!("Missing wire target for '{}'", wire_name)))
}

/// Get variable name from any Variable variant
fn get_variable_name(var: &Variable) -> &str {
    match var {
        Variable::Instance { name, .. } => name,
        Variable::Channel { name, .. } => name,
        Variable::Combined { name, .. } => name,
    }
}

/// Parse instance point type (M/A)
fn parse_instance_point_type(s: &str) -> Result<InstancePointType> {
    match s.to_uppercase().as_str() {
        "M" | "MEASUREMENT" => Ok(InstancePointType::Measurement),
        "A" | "ACTION" => Ok(InstancePointType::Action),
        _ => Err(RuleError::ParseError(format!(
            "Invalid instance point type: '{}'. Expected 'M' or 'A'",
            s
        ))),
    }
}

/// Parse channel point type (T/S/C/A)
fn parse_channel_point_type(s: &str) -> Result<ChannelPointType> {
    match s.to_uppercase().as_str() {
        "T" | "TELEMETRY" => Ok(ChannelPointType::Telemetry),
        "S" | "SIGNAL" => Ok(ChannelPointType::Signal),
        "C" | "CONTROL" => Ok(ChannelPointType::Control),
        "A" | "ADJUSTMENT" => Ok(ChannelPointType::Adjustment),
        _ => Err(RuleError::ParseError(format!(
            "Invalid channel point type: '{}'. Expected T/S/C/A",
            s
        ))),
    }
}

/// Parse a variable definition
///
/// Supports three types:
/// - instance: Read from modsrv instance (inst:{id}:M or inst:{id}:A)
/// - channel: Read from comsrv channel (comsrv:{id}:T/S/C/A)
/// - combined: Calculate from other variables
fn parse_variable(var: &Value) -> Result<Variable> {
    let name = var
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RuleError::ParseError("Variable missing 'name'".to_string()))?
        .to_string();

    let var_type = var
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("instance");

    match var_type {
        "instance" => {
            let instance_id = var
                .get("instance_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    RuleError::ParseError("Instance variable missing 'instance_id'".to_string())
                })? as u16;

            let point_type_str = var
                .get("point_type")
                .and_then(|v| v.as_str())
                .unwrap_or("M");
            let point_type = parse_instance_point_type(point_type_str)?;

            let point_id = var
                .get("point_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    RuleError::ParseError("Instance variable missing 'point_id'".to_string())
                })?
                .to_string();

            Ok(Variable::Instance {
                name,
                instance_id,
                point_type,
                point_id,
            })
        },
        "channel" => {
            let channel_id = var
                .get("channel_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    RuleError::ParseError("Channel variable missing 'channel_id'".to_string())
                })? as u16;

            let point_type_str = var
                .get("point_type")
                .and_then(|v| v.as_str())
                .unwrap_or("T");
            let point_type = parse_channel_point_type(point_type_str)?;

            let point_id = var
                .get("point_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    RuleError::ParseError("Channel variable missing 'point_id'".to_string())
                })?
                .to_string();

            Ok(Variable::Channel {
                name,
                channel_id,
                point_type,
                point_id,
            })
        },
        "combined" => {
            let formula_arr = var
                .get("formula")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    RuleError::ParseError("Combined variable missing 'formula'".to_string())
                })?;

            let formula = parse_formula(formula_arr)?;
            Ok(Variable::Combined { name, formula })
        },
        _ => Err(RuleError::ParseError(format!(
            "Unknown variable type: '{}'. Expected instance/channel/combined",
            var_type
        ))),
    }
}

/// Parse formula tokens from array
fn parse_formula(arr: &[Value]) -> Result<Vec<FormulaToken>> {
    let mut tokens = Vec::new();

    for item in arr {
        if let Some(s) = item.as_str() {
            // Check if it's an operator
            match s {
                "+" => tokens.push(FormulaToken::Op {
                    op: ArithmeticOp::Add,
                }),
                "-" => tokens.push(FormulaToken::Op {
                    op: ArithmeticOp::Sub,
                }),
                "*" => tokens.push(FormulaToken::Op {
                    op: ArithmeticOp::Mul,
                }),
                "/" => tokens.push(FormulaToken::Op {
                    op: ArithmeticOp::Div,
                }),
                _ => {
                    // Try to parse as number, otherwise treat as variable
                    if let Ok(num) = s.parse::<f64>() {
                        tokens.push(FormulaToken::Num { value: num });
                    } else {
                        tokens.push(FormulaToken::Var {
                            name: s.to_string(),
                        });
                    }
                },
            }
        } else if let Some(num) = item.as_f64() {
            tokens.push(FormulaToken::Num { value: num });
        } else if let Some(num) = item.as_i64() {
            tokens.push(FormulaToken::Num { value: num as f64 });
        }
    }

    Ok(tokens)
}

/// Parse switch rules from config
fn parse_switch_rules(config: &Value) -> Result<Vec<SwitchRule>> {
    let rules_arr = config
        .get("rules")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RuleError::ParseError("Switch missing 'rules' array".to_string()))?;

    let wires = config.get("wires");

    let mut switch_rules = Vec::new();

    for rule in rules_arr {
        let name = rule
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Rule missing 'name'".to_string()))?
            .to_string();

        // Get next node from wires
        let next_node = wires
            .and_then(|w| w.get(&name))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "end".to_string());

        // Parse conditions
        let conditions = parse_rule_conditions(rule)?;

        switch_rules.push(SwitchRule {
            name,
            conditions,
            next_node,
        });
    }

    Ok(switch_rules)
}

/// Parse conditions from a rule
fn parse_rule_conditions(rule: &Value) -> Result<Vec<RuleCondition>> {
    let rule_arr = rule
        .get("rule")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RuleError::ParseError("Rule missing 'rule' array".to_string()))?;

    let mut conditions: Vec<RuleCondition> = Vec::new();
    let mut pending_relation: Option<LogicOp> = None;

    for item in rule_arr {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match item_type {
            "variable" => {
                let left = item
                    .get("variables")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RuleError::ParseError("Condition missing 'variables'".to_string())
                    })?
                    .to_string();

                let operator_str = item
                    .get("operator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("==");
                let operator = parse_compare_op(operator_str)?;

                let right = item
                    .get("value")
                    .map(|v| {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else if let Some(n) = v.as_f64() {
                            n.to_string()
                        } else if let Some(n) = v.as_i64() {
                            n.to_string()
                        } else {
                            v.to_string()
                        }
                    })
                    .unwrap_or_default();

                // Attach pending relation to previous condition if exists
                if let Some(rel) = pending_relation.take() {
                    if let Some(last) = conditions.last_mut() {
                        last.relation = Some(rel);
                    }
                }

                conditions.push(RuleCondition {
                    left,
                    operator,
                    right,
                    relation: None,
                });
            },
            "relation" => {
                let rel_str = item.get("value").and_then(|v| v.as_str()).unwrap_or("&&");
                pending_relation = Some(parse_logic_op(rel_str)?);
            },
            _ => {},
        }
    }

    Ok(conditions)
}

/// Parse comparison operator
fn parse_compare_op(s: &str) -> Result<CompareOp> {
    match s {
        "==" | "eq" => Ok(CompareOp::Eq),
        "!=" | "ne" => Ok(CompareOp::Ne),
        ">" | "gt" => Ok(CompareOp::Gt),
        "<" | "lt" => Ok(CompareOp::Lt),
        ">=" | "gte" => Ok(CompareOp::Gte),
        "<=" | "lte" => Ok(CompareOp::Lte),
        _ => Err(RuleError::ParseError(format!(
            "Unknown comparison operator: {}",
            s
        ))),
    }
}

/// Parse logical operator
fn parse_logic_op(s: &str) -> Result<LogicOp> {
    match s {
        "&&" | "and" | "AND" => Ok(LogicOp::And),
        "||" | "or" | "OR" => Ok(LogicOp::Or),
        _ => Err(RuleError::ParseError(format!(
            "Unknown logical operator: {}",
            s
        ))),
    }
}

/// Parse value changes from action config
///
/// Supports two target types:
/// - instance: Write to modsrv instance action point (uses M2C routing)
/// - channel: Write directly to comsrv channel point (bypasses routing)
pub fn parse_value_changes(config: &Value) -> Result<Vec<ValueChange>> {
    let empty_vec = vec![];
    let rules_arr = config
        .get("rules")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    let mut changes = Vec::new();

    for rule in rules_arr {
        let target = rule
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("instance");

        let value = rule
            .get("value")
            .map(|v| {
                if let Some(s) = v.as_str() {
                    s.to_string()
                } else if let Some(n) = v.as_f64() {
                    n.to_string()
                } else if let Some(n) = v.as_i64() {
                    n.to_string()
                } else {
                    v.to_string()
                }
            })
            .unwrap_or_default();

        match target {
            "instance" => {
                let instance_id = rule
                    .get("instance_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        RuleError::ParseError(
                            "ValueChange instance missing 'instance_id'".to_string(),
                        )
                    })? as u16;

                let point_id = rule
                    .get("point_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RuleError::ParseError("ValueChange instance missing 'point_id'".to_string())
                    })?
                    .to_string();

                changes.push(ValueChange::Instance {
                    instance_id,
                    point_id,
                    value,
                });
            },
            "channel" => {
                let channel_id =
                    rule.get("channel_id")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            RuleError::ParseError(
                                "ValueChange channel missing 'channel_id'".to_string(),
                            )
                        })? as u16;

                let point_type_str = rule
                    .get("point_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("A");
                let point_type = parse_channel_point_type(point_type_str)?;

                let point_id = rule
                    .get("point_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        RuleError::ParseError("ValueChange channel missing 'point_id'".to_string())
                    })?
                    .to_string();

                changes.push(ValueChange::Channel {
                    channel_id,
                    point_type,
                    point_id,
                    value,
                });
            },
            _ => {
                return Err(RuleError::ParseError(format!(
                    "Unknown ValueChange target: '{}'. Expected instance/channel",
                    target
                )));
            },
        }
    }

    Ok(changes)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_flow_with_instance_variable() {
        let flow = json!({
            "id": "test-chain-1",
            "name": "Test Rule Chain",
            "description": "A test rule chain",
            "nodes": [
                {
                    "id": "start",
                    "type": "start",
                    "data": {
                        "config": {
                            "wires": {
                                "default": ["node-1"]
                            }
                        }
                    }
                },
                {
                    "id": "node-1",
                    "type": "custom",
                    "data": {
                        "type": "function-switch",
                        "config": {
                            "variables": [
                                {
                                    "name": "Voltage",
                                    "type": "instance",
                                    "instance_id": 5,
                                    "point_type": "M",
                                    "point_id": "10"
                                }
                            ],
                            "rules": [
                                {
                                    "name": "out001",
                                    "rule": [
                                        {
                                            "type": "variable",
                                            "variables": "Voltage",
                                            "operator": ">",
                                            "value": "750"
                                        }
                                    ]
                                }
                            ],
                            "wires": {
                                "out001": ["end"]
                            }
                        }
                    }
                },
                {
                    "id": "end",
                    "type": "end"
                }
            ]
        });

        let parsed = parse_flow_json(&flow).unwrap();

        assert_eq!(parsed.start_node_id, "start");
        assert_eq!(parsed.variables.len(), 1);
        assert_eq!(parsed.nodes.len(), 3);

        // Verify variable is correctly parsed as Instance type
        match &parsed.variables[0] {
            Variable::Instance {
                name,
                instance_id,
                point_type,
                point_id,
            } => {
                assert_eq!(name, "Voltage");
                assert_eq!(*instance_id, 5);
                assert_eq!(*point_type, InstancePointType::Measurement);
                assert_eq!(point_id, "10");
            },
            _ => panic!("Expected Instance variable"),
        }
    }

    #[test]
    fn test_parse_channel_variable() {
        let var = json!({
            "name": "RawTemp",
            "type": "channel",
            "channel_id": 1001,
            "point_type": "T",
            "point_id": "sensor_001"
        });

        let parsed = parse_variable(&var).unwrap();
        match parsed {
            Variable::Channel {
                name,
                channel_id,
                point_type,
                point_id,
            } => {
                assert_eq!(name, "RawTemp");
                assert_eq!(channel_id, 1001);
                assert_eq!(point_type, ChannelPointType::Telemetry);
                assert_eq!(point_id, "sensor_001");
            },
            _ => panic!("Expected Channel variable"),
        }
    }

    #[test]
    fn test_parse_formula() {
        let formula_arr = vec![
            json!("X1"),
            json!("+"),
            json!("X2"),
            json!("-"),
            json!("12"),
        ];

        let tokens = parse_formula(&formula_arr).unwrap();

        assert_eq!(tokens.len(), 5);
        match &tokens[0] {
            FormulaToken::Var { name } => assert_eq!(name, "X1"),
            _ => panic!("Expected Var"),
        }
        match &tokens[1] {
            FormulaToken::Op { op } => assert_eq!(*op, ArithmeticOp::Add),
            _ => panic!("Expected Op"),
        }
        match &tokens[4] {
            FormulaToken::Num { value } => assert_eq!(*value, 12.0),
            _ => panic!("Expected Num"),
        }
    }

    #[test]
    fn test_parse_conditions() {
        let rule = json!({
            "name": "out001",
            "rule": [
                {
                    "type": "variable",
                    "variables": "X1",
                    "operator": "==",
                    "value": "X2"
                },
                {
                    "type": "relation",
                    "value": "&&"
                },
                {
                    "type": "variable",
                    "variables": "X1",
                    "operator": "<",
                    "value": "100"
                }
            ]
        });

        let conditions = parse_rule_conditions(&rule).unwrap();

        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].left, "X1");
        assert_eq!(conditions[0].operator, CompareOp::Eq);
        assert_eq!(conditions[0].right, "X2");
        assert_eq!(conditions[0].relation, Some(LogicOp::And));
        assert_eq!(conditions[1].left, "X1");
        assert_eq!(conditions[1].operator, CompareOp::Lt);
        assert_eq!(conditions[1].right, "100");
        assert_eq!(conditions[1].relation, None);
    }
}
