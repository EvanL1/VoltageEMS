//! Basic integration tests for rulesrv library functionality

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

use rulesrv::rule_engine::{
    Action, ComparisonOperator, Condition, ConditionGroup, LogicalOperator, Rule,
};

#[test]
fn test_rule_structure_creation() {
    // Test creating a basic rule structure
    let rule = Rule {
        id: "test_rule_1".to_string(),
        name: "Test Rule".to_string(),
        category: "test".to_string(),
        description: Some("A test rule".to_string()),
        priority: 100,
        enabled: true,
        triggers: vec![],
        conditions: ConditionGroup::Single(Condition {
            field: "test_field".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: Some(serde_json::json!(50)),
            value_ref: None,
        }),
        actions: vec![Action::SetValue {
            target: "test_target".to_string(),
            value: serde_json::json!(100),
        }],
        metadata: Default::default(),
    };

    assert_eq!(rule.id, "test_rule_1");
    assert_eq!(rule.name, "Test Rule");
    assert!(rule.enabled);
    assert_eq!(rule.priority, 100);
}

#[test]
fn test_condition_group_structure() {
    // Test creating nested condition groups
    let condition_group = ConditionGroup::Group {
        logic: LogicalOperator::And,
        rules: vec![
            ConditionGroup::Single(Condition {
                field: "field1".to_string(),
                operator: ComparisonOperator::GreaterThan,
                value: Some(serde_json::json!(10)),
                value_ref: None,
            }),
            ConditionGroup::Single(Condition {
                field: "field2".to_string(),
                operator: ComparisonOperator::LessThan,
                value: Some(serde_json::json!(100)),
                value_ref: None,
            }),
        ],
    };

    match condition_group {
        ConditionGroup::Group { logic, rules } => {
            assert!(matches!(logic, LogicalOperator::And));
            assert_eq!(rules.len(), 2);
        },
        _ => panic!("Expected Group variant"),
    }
}

#[test]
fn test_action_variants() {
    // Test different action types
    let set_value_action = Action::SetValue {
        target: "test_key".to_string(),
        value: serde_json::json!(42),
    };

    let publish_action = Action::Publish {
        params: {
            let mut p = std::collections::HashMap::new();
            p.insert("channel".to_string(), serde_json::json!("alerts"));
            p.insert("message".to_string(), serde_json::json!("Test message"));
            p
        },
    };

    // Verify actions can be created
    match set_value_action {
        Action::SetValue { target, value } => {
            assert_eq!(target, "test_key");
            assert_eq!(value, serde_json::json!(42));
        },
        _ => panic!("Expected SetValue variant"),
    }

    match publish_action {
        Action::Publish { params } => {
            assert!(params.contains_key("channel"));
            assert!(params.contains_key("message"));
        },
        _ => panic!("Expected Publish variant"),
    }
}

#[test]
fn test_comparison_operator_serialization() {
    // Test that comparison operators can be serialized/deserialized
    let operators = vec![
        ComparisonOperator::Equal,
        ComparisonOperator::NotEqual,
        ComparisonOperator::GreaterThan,
        ComparisonOperator::GreaterThanOrEqual,
        ComparisonOperator::LessThan,
        ComparisonOperator::LessThanOrEqual,
    ];

    for op in operators {
        // Serialize
        let json = serde_json::to_string(&op).unwrap();
        // Deserialize
        let deserialized: ComparisonOperator = serde_json::from_str(&json).unwrap();
        // Verify round-trip
        assert_eq!(serde_json::to_string(&deserialized).unwrap(), json);
    }
}

#[test]
fn test_rule_serialization() {
    // Test that rules can be serialized and deserialized
    let rule = Rule {
        id: "rule_1".to_string(),
        name: "Test Rule".to_string(),
        category: "test".to_string(),
        description: Some("Description".to_string()),
        priority: 100,
        enabled: true,
        triggers: vec![],
        conditions: ConditionGroup::Single(Condition {
            field: "value".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: Some(serde_json::json!(50)),
            value_ref: None,
        }),
        actions: vec![Action::SetValue {
            target: "output".to_string(),
            value: serde_json::json!(100),
        }],
        metadata: Default::default(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&rule).unwrap();

    // Deserialize back
    let deserialized: Rule = serde_json::from_str(&json).unwrap();

    // Verify key fields
    assert_eq!(deserialized.id, rule.id);
    assert_eq!(deserialized.name, rule.name);
    assert_eq!(deserialized.enabled, rule.enabled);
    assert_eq!(deserialized.priority, rule.priority);
}
