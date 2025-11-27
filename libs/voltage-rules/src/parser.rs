//! Vue Flow JSON Parser
//!
//! Parses frontend Vue Flow rule JSON into the simplified RuleFlow structure
//! for execution by the rule engine.
//!
//! The parser extracts only execution-necessary information from Vue Flow JSON,
//! discarding UI-only data like positions, labels, and edge styling.

use serde_json::Value;
use std::collections::HashMap;
use voltage_config::rulesrv::{
    FlowCondition, RuleFlow, RuleNode, RuleSwitchBranch, RuleValueAssignment, RuleVariable,
    RuleWires,
};

use crate::error::{Result, RuleError};

// =============================================================================
// Rule Flow Extraction (Vue Flow JSON → RuleFlow)
// =============================================================================

/// Extract rule flow topology from Vue Flow JSON
///
/// This function converts the full Vue Flow JSON (with UI information like positions,
/// labels, edges) into a minimal RuleFlow structure containing only execution-necessary
/// information.
///
/// # Arguments
/// * `full_json` - The complete Vue Flow JSON from frontend
///
/// # Returns
/// * `Ok(RuleFlow)` - Simplified topology with HashMap-based node lookup
/// * `Err(RuleError)` - If parsing fails or required fields are missing
///
/// # Example
/// ```ignore
/// let flow = extract_rule_flow(&vue_flow_json)?;
/// // flow.nodes is HashMap<String, RuleNode> for O(1) lookup
/// ```
pub fn extract_rule_flow(full_json: &Value) -> Result<RuleFlow> {
    let nodes_array = full_json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RuleError::ParseError("Missing 'nodes' array".to_string()))?;

    let mut nodes = HashMap::new();
    let mut start_node = String::new();

    for node in nodes_array {
        let node_id = node
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Node missing 'id'".to_string()))?
            .to_string();

        let node_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("custom");

        let data = node.get("data");

        let compact_node = match node_type {
            "start" => {
                start_node = node_id.clone();
                let wires = extract_rule_wires_default(data)?;
                RuleNode::Start { wires }
            },
            "end" => RuleNode::End,
            "custom" => {
                let inner_type = data
                    .and_then(|d| d.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match inner_type {
                    "function-switch" => extract_switch_rule_node(data)?,
                    "action-changeValue" => extract_change_value_rule_node(data)?,
                    _ => {
                        tracing::warn!("Unknown custom node type: {}, skipping", inner_type);
                        continue;
                    },
                }
            },
            _ => {
                tracing::warn!("Unknown top-level node type: {}, skipping", node_type);
                continue;
            },
        };

        nodes.insert(node_id, compact_node);
    }

    if start_node.is_empty() {
        return Err(RuleError::ParseError(
            "No start node found in flow".to_string(),
        ));
    }

    Ok(RuleFlow { start_node, nodes })
}

/// Extract RuleWires from node data (for default wire)
fn extract_rule_wires_default(data: Option<&Value>) -> Result<RuleWires> {
    let default_targets = data
        .and_then(|d| d.get("config"))
        .or(data)
        .and_then(|c| c.get("wires"))
        .and_then(|w| w.get("default"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(RuleWires {
        default: default_targets,
    })
}

/// Extract function-switch node as RuleNode::Switch
fn extract_switch_rule_node(data: Option<&Value>) -> Result<RuleNode> {
    let config = data
        .and_then(|d| d.get("config"))
        .ok_or_else(|| RuleError::ParseError("Switch node missing 'config'".to_string()))?;

    // Extract variables
    let variables = extract_rule_variables(config)?;

    // Extract rules
    let rule = extract_rule_switch_branches(config)?;

    // Extract wires (HashMap for multiple outputs)
    let wires = extract_rule_wires_map(config)?;

    Ok(RuleNode::Switch {
        variables,
        rule,
        wires,
    })
}

/// Extract action-changeValue node as RuleNode::ChangeValue
fn extract_change_value_rule_node(data: Option<&Value>) -> Result<RuleNode> {
    let config = data.and_then(|d| d.get("config"));

    // Extract variables (target points)
    let variables = config
        .map(extract_rule_variables)
        .transpose()?
        .unwrap_or_default();

    // Extract value assignments
    let rule = config
        .map(extract_rule_value_assignments)
        .transpose()?
        .unwrap_or_default();

    // Extract wires (default output)
    let wires = extract_rule_wires_default(config)?;

    Ok(RuleNode::ChangeValue {
        variables,
        rule,
        wires,
    })
}

/// Extract compact variables from config
fn extract_rule_variables(config: &Value) -> Result<Vec<RuleVariable>> {
    let vars_arr = match config.get("variables").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut variables = Vec::new();
    for var in vars_arr {
        let name = var
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Variable missing 'name'".to_string()))?
            .to_string();

        let var_type = var
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("single")
            .to_string();

        let instance = var
            .get("instance")
            .and_then(|v| v.as_str())
            .map(String::from);

        let point_type = var
            .get("pointType")
            .and_then(|v| v.as_str())
            .map(String::from);

        let point = var.get("point").and_then(|v| v.as_u64()).map(|n| n as u16);

        let formula = var
            .get("formula")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        variables.push(RuleVariable {
            name,
            var_type,
            instance,
            point_type,
            point,
            formula,
        });
    }

    Ok(variables)
}

/// Extract compact switch rules from config
fn extract_rule_switch_branches(config: &Value) -> Result<Vec<RuleSwitchBranch>> {
    let rules_arr = match config.get("rule").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut rules = Vec::new();
    for rule in rules_arr {
        let name = rule
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Rule missing 'name'".to_string()))?
            .to_string();

        let rule_type = rule
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let conditions = extract_flow_conditions(rule)?;

        rules.push(RuleSwitchBranch {
            name,
            rule_type,
            rule: conditions,
        });
    }

    Ok(rules)
}

/// Extract compact conditions from a rule
fn extract_flow_conditions(rule: &Value) -> Result<Vec<FlowCondition>> {
    let rule_arr = match rule.get("rule").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut conditions = Vec::new();
    for item in rule_arr {
        let cond_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("variable")
            .to_string();

        let variables = item
            .get("variables")
            .and_then(|v| v.as_str())
            .map(String::from);
        let operator = item
            .get("operator")
            .and_then(|v| v.as_str())
            .map(String::from);
        let value = item.get("value").cloned();

        conditions.push(FlowCondition {
            cond_type,
            variables,
            operator,
            value,
        });
    }

    Ok(conditions)
}

/// Extract compact value assignments from config
fn extract_rule_value_assignments(config: &Value) -> Result<Vec<RuleValueAssignment>> {
    let rules_arr = match config.get("rule").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let mut assignments = Vec::new();
    for rule in rules_arr {
        let variables = rule
            .get("Variables")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RuleError::ParseError("Assignment missing 'Variables'".to_string()))?
            .to_string();

        let value = rule
            .get("value")
            .cloned()
            .ok_or_else(|| RuleError::ParseError("Assignment missing 'value'".to_string()))?;

        assignments.push(RuleValueAssignment { variables, value });
    }

    Ok(assignments)
}

/// Extract wires as HashMap for multiple outputs (used by switch nodes)
fn extract_rule_wires_map(config: &Value) -> Result<HashMap<String, Vec<String>>> {
    let wires_obj = match config.get("wires").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Ok(HashMap::new()),
    };

    let mut wires_map = HashMap::new();
    for (key, value) in wires_obj {
        let targets: Vec<String> = value
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        wires_map.insert(key.clone(), targets);
    }

    Ok(wires_map)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_rule_flow() {
        // Test Vue Flow JSON similar to user's example
        let flow = json!({
            "id": "rule-001",
            "name": "Test Rule",
            "nodes": [
                {
                    "id": "start",
                    "type": "start",
                    "position": { "x": 0, "y": 0 },
                    "data": {
                        "config": {
                            "wires": { "default": ["node-1"] }
                        }
                    }
                },
                {
                    "id": "node-1",
                    "type": "custom",
                    "position": { "x": 100, "y": 100 },
                    "data": {
                        "type": "function-switch",
                        "label": "Check Value",
                        "config": {
                            "variables": [
                                {
                                    "name": "X1",
                                    "type": "single",
                                    "instance": "battery_01",
                                    "pointType": "measurement",
                                    "point": 3
                                }
                            ],
                            "rule": [
                                {
                                    "name": "out001",
                                    "type": "default",
                                    "rule": [
                                        {
                                            "type": "variable",
                                            "variables": "X1",
                                            "operator": "<=",
                                            "value": 5
                                        }
                                    ]
                                }
                            ],
                            "wires": {
                                "out001": ["node-2"],
                                "out002": ["end"]
                            }
                        }
                    }
                },
                {
                    "id": "node-2",
                    "type": "custom",
                    "position": { "x": 200, "y": 200 },
                    "data": {
                        "type": "action-changeValue",
                        "config": {
                            "variables": [
                                {
                                    "name": "Y1",
                                    "type": "single",
                                    "instance": "pv_01",
                                    "pointType": "action",
                                    "point": 5
                                }
                            ],
                            "rule": [
                                { "Variables": "Y1", "value": 999 }
                            ],
                            "wires": { "default": ["end"] }
                        }
                    }
                },
                {
                    "id": "end",
                    "type": "end",
                    "position": { "x": 300, "y": 300 }
                }
            ],
            "edges": [
                { "id": "e1", "source": "start", "target": "node-1" }
            ],
            "metadata": { "exportedAt": "2024-01-01" }
        });

        let compact = extract_rule_flow(&flow).unwrap();

        // Verify start node
        assert_eq!(compact.start_node, "start");
        assert_eq!(compact.nodes.len(), 4);

        // Verify start node structure
        match compact.nodes.get("start").unwrap() {
            RuleNode::Start { wires } => {
                assert_eq!(wires.default, vec!["node-1"]);
            },
            _ => panic!("Expected Start node"),
        }

        // Verify switch node structure
        match compact.nodes.get("node-1").unwrap() {
            RuleNode::Switch {
                variables,
                rule,
                wires,
            } => {
                assert_eq!(variables.len(), 1);
                assert_eq!(variables[0].name, "X1");
                assert_eq!(variables[0].var_type, "single");
                assert_eq!(variables[0].instance, Some("battery_01".to_string()));
                assert_eq!(variables[0].point_type, Some("measurement".to_string()));
                assert_eq!(variables[0].point, Some(3));

                assert_eq!(rule.len(), 1);
                assert_eq!(rule[0].name, "out001");

                assert_eq!(wires.get("out001").unwrap(), &vec!["node-2"]);
                assert_eq!(wires.get("out002").unwrap(), &vec!["end"]);
            },
            _ => panic!("Expected Switch node"),
        }

        // Verify changeValue node structure
        match compact.nodes.get("node-2").unwrap() {
            RuleNode::ChangeValue {
                variables,
                rule,
                wires,
            } => {
                assert_eq!(variables.len(), 1);
                assert_eq!(variables[0].name, "Y1");
                assert_eq!(variables[0].point_type, Some("action".to_string()));

                assert_eq!(rule.len(), 1);
                assert_eq!(rule[0].variables, "Y1");
                assert_eq!(rule[0].value, json!(999));

                assert_eq!(wires.default, vec!["end"]);
            },
            _ => panic!("Expected ChangeValue node"),
        }

        // Verify end node
        assert!(matches!(compact.nodes.get("end").unwrap(), RuleNode::End));
    }

    #[test]
    fn test_compact_flow_serialization() {
        // Test that RuleFlow can be serialized to the expected JSON format
        let flow = json!({
            "nodes": [
                {
                    "id": "start",
                    "type": "start",
                    "data": {
                        "config": {
                            "wires": { "default": ["end"] }
                        }
                    }
                },
                {
                    "id": "end",
                    "type": "end"
                }
            ]
        });

        let compact = extract_rule_flow(&flow).unwrap();
        let serialized = serde_json::to_string(&compact).unwrap();
        let deserialized: RuleFlow = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.start_node, "start");
        assert_eq!(deserialized.nodes.len(), 2);
    }
}
