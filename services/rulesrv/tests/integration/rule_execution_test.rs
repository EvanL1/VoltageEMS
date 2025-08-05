use rulesrv::engine::{
    ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator, Rule,
    RuleAction, RuleEngine,
};
use rulesrv::redis::RedisStore;
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

async fn create_test_store() -> Arc<RedisStore> {
    Arc::new(
        RedisStore::new("redis://localhost:6379", Some("exec_test"))
            .expect("Failed to create test store"),
    )
}

async fn cleanup_redis_keys(store: &RedisStore, pattern: &str) {
    let keys = store.scan_keys(pattern).await.unwrap();
    for key in keys {
        store.delete(&key).await.ok();
    }
}

#[tokio::test]
async fn test_complete_rule_execution_flow() {
    let store = create_test_store().await;
    let mut engine = RuleEngine::new(store.clone());

    // Setup test environment
    store.set_string("battery.soc", "15").await.unwrap();
    store
        .set_string("generator.status", "stopped")
        .await
        .unwrap();
    store.set_string("generator.fuel", "85").await.unwrap();

    let rule = Rule {
        id: "battery_management".to_string(),
        name: "Battery Management Rule".to_string(),
        description: Some("Manage generator based on battery SOC".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![
                Condition {
                    source: "battery.soc".to_string(),
                    operator: ComparisonOperator::LessThanOrEqual,
                    value: json!(20.0),
                    description: Some("Battery low".to_string()),
                },
                Condition {
                    source: "generator.status".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!("stopped"),
                    description: Some("Generator is not running".to_string()),
                },
                Condition {
                    source: "generator.fuel".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: json!(10.0),
                    description: Some("Sufficient fuel".to_string()),
                },
            ],
        },
        actions: vec![
            RuleAction {
                action_type: ActionType::DeviceControl,
                config: ActionConfig::DeviceControl {
                    device_id: "generator_001".to_string(),
                    channel: "control".to_string(),
                    point: "start".to_string(),
                    value: json!(true),
                },
                description: Some("Start generator".to_string()),
            },
            RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "generator.status".to_string(),
                    value: json!("starting"),
                    ttl: None,
                },
                description: Some("Update status".to_string()),
            },
            RuleAction {
                action_type: ActionType::Notify,
                config: ActionConfig::Notify {
                    level: "warning".to_string(),
                    message: "Generator started due to low battery".to_string(),
                    recipients: Some(vec!["operator@example.com".to_string()]),
                },
                description: Some("Send notification".to_string()),
            },
        ],
        enabled: true,
        priority: 1,
        cooldown_seconds: Some(300),
    };

    // Store and execute rule
    engine.store_rule(&rule).await.unwrap();
    let result = engine.execute_rule("battery_management").await.unwrap();

    // Verify execution results
    assert!(result.conditions_met);
    assert!(result.success);
    assert_eq!(result.actions_executed.len(), 3);

    // Verify generator status was updated
    let status = store.get_string("generator.status").await.unwrap();
    assert_eq!(status, Some("\"starting\"".to_string()));

    // Verify execution statistics were stored
    let stats_key = "rulesrv:rule:battery_management:stats";
    let stats = store.get_string(stats_key).await.unwrap();
    assert!(stats.is_some());

    // Cleanup
    cleanup_redis_keys(&store, "battery.*").await;
    cleanup_redis_keys(&store, "generator.*").await;
    cleanup_redis_keys(&store, "rulesrv:*").await;
    cleanup_redis_keys(&store, "ems:*").await;
}

#[tokio::test]
async fn test_rule_priority_execution() {
    let store = create_test_store().await;
    let engine = RuleEngine::new(store.clone());

    // Create multiple rules with different priorities
    let high_priority_rule = Rule {
        id: "high_priority".to_string(),
        name: "High Priority Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.priority".to_string(),
                operator: ComparisonOperator::Equals,
                value: json!(true),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.high_executed".to_string(),
                value: json!(true),
                ttl: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 10, // High priority
        cooldown_seconds: None,
    };

    let low_priority_rule = Rule {
        id: "low_priority".to_string(),
        name: "Low Priority Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.priority".to_string(),
                operator: ComparisonOperator::Equals,
                value: json!(true),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.low_executed".to_string(),
                value: json!(true),
                ttl: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 1, // Low priority
        cooldown_seconds: None,
    };

    // Store rules
    engine.store_rule(&high_priority_rule).await.unwrap();
    engine.store_rule(&low_priority_rule).await.unwrap();

    // Set trigger condition
    store.set_string("test.priority", "true").await.unwrap();

    // In a real system, rules would be executed based on priority
    // For this test, we're verifying that both rules exist and can be executed

    let rules = engine.list_rules().await.unwrap();
    assert!(rules.len() >= 2);

    // Verify rules are sorted by priority (if implemented)
    let high_priority_exists = rules
        .iter()
        .any(|r| r.id == "high_priority" && r.priority == 10);
    let low_priority_exists = rules
        .iter()
        .any(|r| r.id == "low_priority" && r.priority == 1);
    assert!(high_priority_exists);
    assert!(low_priority_exists);

    // Cleanup
    cleanup_redis_keys(&store, "test.*").await;
    cleanup_redis_keys(&store, "rulesrv:rule:*").await;
}

#[tokio::test]
async fn test_concurrent_rule_execution() {
    let store = create_test_store().await;
    let engine = Arc::new(tokio::sync::RwLock::new(RuleEngine::new(store.clone())));

    // Create a rule that increments a counter
    let rule = Rule {
        id: "concurrent_test".to_string(),
        name: "Concurrent Test Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.concurrent".to_string(),
                operator: ComparisonOperator::Equals,
                value: json!(true),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.counter".to_string(),
                value: json!("incremented"),
                ttl: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    // Store rule
    {
        let engine = engine.read().await;
        engine.store_rule(&rule).await.unwrap();
    }

    // Set trigger condition
    store.set_string("test.concurrent", "true").await.unwrap();

    // Execute rule concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let engine_clone = engine.clone();
        let rule_id = "concurrent_test".to_string();
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(i * 10)).await; // Slight delay to ensure concurrency
            let mut engine = engine_clone.write().await;
            engine.execute_rule(&rule_id).await
        });
        handles.push(handle);
    }

    // Wait for all executions
    let mut successful_executions = 0;
    for handle in handles {
        if let Ok(Ok(result)) = handle.await {
            if result.success {
                successful_executions += 1;
            }
        }
    }

    // At least one execution should succeed
    assert!(successful_executions > 0);

    // Cleanup
    cleanup_redis_keys(&store, "test.*").await;
    cleanup_redis_keys(&store, "rulesrv:*").await;
}

#[tokio::test]
async fn test_rule_with_complex_conditions() {
    let store = create_test_store().await;
    let mut engine = RuleEngine::new(store.clone());

    // Setup complex test data
    store.set_string("system.temperature", "75").await.unwrap();
    store.set_string("system.pressure", "120").await.unwrap();
    store.set_string("system.flow_rate", "50").await.unwrap();
    store.set_string("system.status", "running").await.unwrap();
    store
        .set_hash_field("sensor:readings", "temp_sensor_1", "78.5")
        .await
        .unwrap();

    let rule = Rule {
        id: "complex_conditions".to_string(),
        name: "Complex Conditions Rule".to_string(),
        description: Some("Test rule with nested conditions".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![
                Condition {
                    source: "system.status".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!("running"),
                    description: Some("System is running".to_string()),
                },
                Condition {
                    source: "system.temperature".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: json!(70.0),
                    description: Some("Temperature above threshold".to_string()),
                },
                Condition {
                    source: "system.pressure".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: json!(150.0),
                    description: Some("Pressure within limits".to_string()),
                },
                Condition {
                    source: "sensor:readings.temp_sensor_1".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: json!(80.0),
                    description: Some("Sensor reading normal".to_string()),
                },
            ],
        },
        actions: vec![
            RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "system.alert_status".to_string(),
                    value: json!("normal"),
                    ttl: None,
                },
                description: Some("Set alert status".to_string()),
            },
            RuleAction {
                action_type: ActionType::Publish,
                config: ActionConfig::Publish {
                    channel: "system:status".to_string(),
                    message: "All systems operating normally".to_string(),
                },
                description: Some("Publish status".to_string()),
            },
        ],
        enabled: true,
        priority: 5,
        cooldown_seconds: Some(60),
    };

    // Store and execute
    engine.store_rule(&rule).await.unwrap();
    let result = engine.execute_rule("complex_conditions").await.unwrap();

    // Verify results
    assert!(result.conditions_met);
    assert!(result.success);
    assert_eq!(result.actions_executed.len(), 2);

    // Verify alert status was set
    let alert_status = store.get_string("system.alert_status").await.unwrap();
    assert_eq!(alert_status, Some("\"normal\"".to_string()));

    // Cleanup
    cleanup_redis_keys(&store, "system.*").await;
    cleanup_redis_keys(&store, "sensor:*").await;
    cleanup_redis_keys(&store, "rulesrv:*").await;
}

#[tokio::test]
async fn test_rule_execution_with_errors() {
    let store = create_test_store().await;
    let mut engine = RuleEngine::new(store.clone());

    // Create a rule with an action that will fail
    let rule = Rule {
        id: "error_test".to_string(),
        name: "Error Test Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.error".to_string(),
                operator: ComparisonOperator::Equals,
                value: json!(true),
                description: None,
            }],
        },
        actions: vec![
            RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "test.first_action".to_string(),
                    value: json!("executed"),
                    ttl: None,
                },
                description: Some("First action (should succeed)".to_string()),
            },
            // This action has invalid config and should fail
            RuleAction {
                action_type: ActionType::DeviceControl,
                config: ActionConfig::SetValue {
                    // Wrong config type for DeviceControl
                    key: "invalid".to_string(),
                    value: json!("config"),
                    ttl: None,
                },
                description: Some("Invalid action".to_string()),
            },
            RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "test.third_action".to_string(),
                    value: json!("should_not_execute"),
                    ttl: None,
                },
                description: Some("Third action (should not execute)".to_string()),
            },
        ],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    // Store rule and set trigger
    engine.store_rule(&rule).await.unwrap();
    store.set_string("test.error", "true").await.unwrap();

    // Execute rule
    let result = engine.execute_rule("error_test").await.unwrap();

    // Verify partial execution
    assert!(result.conditions_met);
    assert!(!result.success); // Should fail due to invalid action
    assert!(result.error.is_some());
    assert_eq!(result.actions_executed.len(), 1); // Only first action should execute

    // Verify first action was executed
    let first_action = store.get_string("test.first_action").await.unwrap();
    assert_eq!(first_action, Some("\"executed\"".to_string()));

    // Verify third action was not executed
    let third_action = store.get_string("test.third_action").await.unwrap();
    assert!(third_action.is_none());

    // Cleanup
    cleanup_redis_keys(&store, "test.*").await;
    cleanup_redis_keys(&store, "rulesrv:*").await;
}

#[tokio::test]
async fn test_rule_cooldown_in_real_scenario() {
    let store = create_test_store().await;
    let mut engine = RuleEngine::new(store.clone());

    // Create a rule with short cooldown for testing
    let rule = Rule {
        id: "cooldown_scenario".to_string(),
        name: "Cooldown Scenario Rule".to_string(),
        description: Some("Test cooldown in real scenario".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "alert.temperature".to_string(),
                operator: ComparisonOperator::GreaterThan,
                value: json!(80.0),
                description: Some("High temperature".to_string()),
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::Notify,
            config: ActionConfig::Notify {
                level: "critical".to_string(),
                message: "High temperature alert!".to_string(),
                recipients: Some(vec!["ops@example.com".to_string()]),
            },
            description: Some("Send alert".to_string()),
        }],
        enabled: true,
        priority: 10,
        cooldown_seconds: Some(3), // 3 seconds for testing
    };

    // Store rule
    engine.store_rule(&rule).await.unwrap();

    // Simulate temperature spike
    store.set_string("alert.temperature", "85").await.unwrap();

    // First execution should succeed
    let result1 = engine.execute_rule("cooldown_scenario").await.unwrap();
    assert!(result1.conditions_met);
    assert!(result1.success);
    let first_exec_time = std::time::Instant::now();

    // Immediate second execution should be blocked by cooldown
    let result2 = engine.execute_rule("cooldown_scenario").await.unwrap();
    assert!(!result2.conditions_met);
    assert!(result2.error.unwrap().contains("cooldown"));

    // Wait for half the cooldown period
    sleep(Duration::from_secs(2)).await;

    // Still in cooldown
    let result3 = engine.execute_rule("cooldown_scenario").await.unwrap();
    assert!(!result3.conditions_met);

    // Wait for cooldown to expire
    sleep(Duration::from_secs(2)).await;

    // Now should execute again
    let result4 = engine.execute_rule("cooldown_scenario").await.unwrap();
    assert!(result4.conditions_met);
    assert!(result4.success);

    let elapsed = first_exec_time.elapsed().as_secs();
    assert!(elapsed >= 3);

    // Cleanup
    cleanup_redis_keys(&store, "alert.*").await;
    cleanup_redis_keys(&store, "rulesrv:*").await;
}
