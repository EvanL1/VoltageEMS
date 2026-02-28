use super::*;
use serde_json::json;
use voltage_rtdb::{Bytes, MemoryRtdb};

use crate::parser::extract_rule_flow;

/// Helper: create executor with fresh MemoryRtdb
fn new_executor() -> (Arc<MemoryRtdb>, RuleExecutor<MemoryRtdb>) {
    let rtdb = Arc::new(MemoryRtdb::new());
    let routing_cache = Arc::new(RoutingCache::default());
    let executor = RuleExecutor::new(Arc::clone(&rtdb), routing_cache);
    (rtdb, executor)
}

/// Helper: setup SOC strategy test with given battery value
async fn setup_soc_test(
    soc_value: &'static str,
) -> (Arc<MemoryRtdb>, RuleExecutor<MemoryRtdb>, Rule) {
    let (rtdb, executor) = new_executor();
    // Setup instance name index
    for (name, id) in [("battery_01", "5"), ("pv_01", "6"), ("diesel_gen_01", "7")] {
        rtdb.hash_set("inst:name:index", name, Bytes::from(id))
            .await
            .unwrap();
    }
    rtdb.hash_set("inst:5:M", "3", Bytes::from(soc_value))
        .await
        .unwrap();
    let rule = create_soc_rule();
    (rtdb, executor, rule)
}

/// Helper: Build simplified SOC strategy flow JSON
///
/// Logic:
/// - X1 <= 5 (low battery) → out001 → changeValue1 (pv_01:A:5=999)
/// - X1 >= 49 (medium)     → out002 → changeValue2 (diesel_gen_01:A:2=1)
/// - X1 >= 99 (high)       → out003 → changeValue3 (pv_01:A:5=78)
fn soc_strategy_json() -> serde_json::Value {
    json!({
        "nodes": [
            { "id": "start", "type": "start", "data": { "config": { "wires": { "default": ["switch1"] } } } },
            {
                "id": "switch1", "type": "custom",
                "data": {
                    "type": "function-switch",
                    "config": {
                        "variables": [{ "name": "X1", "type": "single", "instance": 5, "pointType": "measurement", "point": 3 }],
                        "rule": [
                            { "name": "out001", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": "<=", "value": 5 }] },
                            { "name": "out002", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": ">=", "value": 49 }] },
                            { "name": "out003", "type": "default", "rule": [{ "type": "variable", "variables": "X1", "operator": ">=", "value": 99 }] }
                        ],
                        "wires": { "out001": ["changeValue1"], "out002": ["changeValue2"], "out003": ["changeValue3"] }
                    }
                }
            },
            { "id": "changeValue1", "type": "custom", "data": { "type": "action-changeValue", "config": {
                "variables": [{ "name": "Y1", "type": "single", "instance": 6, "pointType": "action", "point": 5 }],
                "rule": [{ "Variables": "Y1", "value": 999 }], "wires": { "default": ["end"] }
            }}},
            { "id": "changeValue2", "type": "custom", "data": { "type": "action-changeValue", "config": {
                "variables": [{ "name": "Y2", "type": "single", "instance": 7, "pointType": "action", "point": 2 }],
                "rule": [{ "Variables": "Y2", "value": 1 }], "wires": { "default": ["end"] }
            }}},
            { "id": "changeValue3", "type": "custom", "data": { "type": "action-changeValue", "config": {
                "variables": [{ "name": "Y3", "type": "single", "instance": 6, "pointType": "action", "point": 5 }],
                "rule": [{ "Variables": "Y3", "value": 78 }], "wires": { "default": ["end"] }
            }}},
            { "id": "end", "type": "end" }
        ]
    })
}

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

// =========================================================================
// Condition Evaluation Tests
// =========================================================================

#[tokio::test]
async fn test_evaluate_flow_condition() {
    let (_rtdb, executor) = new_executor();

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
    let (_rtdb, executor) = new_executor();

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
    let (_rtdb, executor) = new_executor();

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
// SOC Strategy Tests (using setup_soc_test helper)
// =========================================================================

#[tokio::test]
async fn test_soc_strategy_low_battery() {
    // SOC = 3.5 → should match out001 (X1 <= 5)
    let (_rtdb, executor, rule) = setup_soc_test("3.5").await;
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
    let (_rtdb, executor, rule) = setup_soc_test("5.0").await;
    let result = executor.execute(&rule).await.unwrap();

    assert!(result.success);
    assert!(result.execution_path.contains(&"changeValue1".to_string()));
}

#[tokio::test]
async fn test_soc_strategy_medium_battery() {
    // SOC = 50.0 → should match out002 (X1 >= 49)
    let (_rtdb, executor, rule) = setup_soc_test("50.0").await;
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
    // SOC = 99.5 → out002 (>=49) matches before out003 (>=99) due to condition order
    let (_rtdb, executor, rule) = setup_soc_test("99.5").await;
    let result = executor.execute(&rule).await.unwrap();

    assert!(result.success);
    assert!(
        result.execution_path.contains(&"changeValue2".to_string()),
        "Due to condition order, out002 matches first. Path: {:?}",
        result.execution_path
    );
}

#[tokio::test]
async fn test_soc_strategy_no_match() {
    // SOC = 25.0 → no match (5 < 25 < 49)
    let (_rtdb, executor, rule) = setup_soc_test("25.0").await;
    let result = executor.execute(&rule).await.unwrap();

    assert!(!result.success);
    assert!(result.error.is_some());
    assert!(result.error.unwrap().contains("No matching switch rule"));
}

// =========================================================================
// Variable Reading Tests
// =========================================================================

#[tokio::test]
async fn test_read_rule_variables_with_name_index() {
    let (rtdb, executor) = new_executor();

    rtdb.hash_set("inst:name:index", "test_device", Bytes::from("100"))
        .await
        .unwrap();
    rtdb.hash_set("inst:100:M", "1", Bytes::from("42.5"))
        .await
        .unwrap();

    let variables = vec![RuleVariable {
        name: "X1".to_string(),
        instance: Some(100),
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

#[tokio::test]
async fn test_read_variables_no_cache_uses_redis() {
    let (rtdb, executor) = new_executor();

    rtdb.hash_set("inst:10:M", "1", Bytes::from("55.5"))
        .await
        .unwrap();

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

    assert_eq!(values.get("DIRECT"), Some(&55.5));
}

// =========================================================================
// RPN Formula Calculation Tests
// =========================================================================

#[test]
fn test_rpn_simple_addition() {
    let formula = vec![json!("X1"), json!("X2"), json!("+")];
    let mut values = HashMap::new();
    values.insert("X1".to_string(), 10.0);
    values.insert("X2".to_string(), 20.0);
    assert_eq!(evaluate_rpn_formula(&formula, &values), Some(30.0));
}

#[test]
fn test_rpn_complex_expression() {
    // (X1 + X2) * 2 = (10 + 20) * 2 = 60
    let formula = vec![json!("X1"), json!("X2"), json!("+"), json!(2), json!("*")];
    let mut values = HashMap::new();
    values.insert("X1".to_string(), 10.0);
    values.insert("X2".to_string(), 20.0);
    assert_eq!(evaluate_rpn_formula(&formula, &values), Some(60.0));
}

#[test]
fn test_rpn_all_operators() {
    // (a + b) - (c * d / e) = (10+5) - (6*4/2) = 15 - 12 = 3
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
    for (k, v) in [("a", 10.0), ("b", 5.0), ("c", 6.0), ("d", 4.0), ("e", 2.0)] {
        values.insert(k.to_string(), v);
    }
    assert_eq!(evaluate_rpn_formula(&formula, &values), Some(3.0));
}

#[test]
fn test_rpn_division_by_zero() {
    let formula = vec![json!("X1"), json!(0), json!("/")];
    let mut values = HashMap::new();
    values.insert("X1".to_string(), 10.0);
    assert_eq!(evaluate_rpn_formula(&formula, &values), None);
}

#[test]
fn test_rpn_undefined_variable() {
    let formula = vec![json!("X1"), json!("UNDEFINED"), json!("+")];
    let mut values = HashMap::new();
    values.insert("X1".to_string(), 10.0);
    assert_eq!(evaluate_rpn_formula(&formula, &values), None);
}

#[test]
fn test_rpn_numeric_literals() {
    // 5 + (3 * 2) = 11
    let formula = vec![json!(5), json!(3), json!(2), json!("*"), json!("+")];
    assert_eq!(evaluate_rpn_formula(&formula, &HashMap::new()), Some(11.0));
}

#[test]
fn test_rpn_float_precision() {
    let formula = vec![json!(1.5), json!(2.5), json!("+")];
    assert_eq!(evaluate_rpn_formula(&formula, &HashMap::new()), Some(4.0));
}
