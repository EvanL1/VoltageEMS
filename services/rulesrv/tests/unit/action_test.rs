#[cfg(test)]
mod action_tests {
    use rulesrv::engine::{
        ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator,
        Rule, RuleAction, RuleEngine,
    };
    use rulesrv::redis::RedisStore;
    use serde_json::json;
    use std::sync::Arc;

    async fn create_test_store() -> Arc<RedisStore> {
        Arc::new(
            RedisStore::new("redis://localhost:6379", Some("test_action"))
                .expect("Failed to create test store"),
        )
    }

    fn create_rule_with_action(id: &str, action: RuleAction) -> Rule {
        Rule {
            id: id.to_string(),
            name: format!("Test Rule {}", id),
            description: Some("Test rule with action".to_string()),
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.trigger".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(true),
                    description: Some("Always true for testing".to_string()),
                }],
            },
            actions: vec![action],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_device_control_action() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 设置触发条件
        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::DeviceControl,
            config: ActionConfig::DeviceControl {
                device_id: "test_device".to_string(),
                channel: "control".to_string(),
                point: "power".to_string(),
                value: json!(true),
            },
            description: Some("Turn on test device".to_string()),
        };

        let rule = create_rule_with_action("test_device_control", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_device_control").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 1);
        assert!(result.actions_executed[0].contains("Device control command queued"));

        // 验证命令是否被创建
        // 注意：实际的命令ID是动态生成的，我们无法预测
        // 但可以检查是否有新的控制命令被创建

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("rulesrv:rule:test_device_control").await.ok();
        // 清理可能创建的控制命令
        let keys = store.scan_keys("ems:control:cmd:*").await.unwrap();
        for key in keys {
            store.delete(&key).await.ok();
        }
    }

    #[tokio::test]
    async fn test_publish_action() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::Publish,
            config: ActionConfig::Publish {
                channel: "test:channel".to_string(),
                message: "Test message from rule engine".to_string(),
            },
            description: Some("Publish test message".to_string()),
        };

        let rule = create_rule_with_action("test_publish", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_publish").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 1);
        assert!(result.actions_executed[0].contains("Published to channel"));

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("rulesrv:rule:test_publish").await.ok();
    }

    #[tokio::test]
    async fn test_set_value_action() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.output".to_string(),
                value: json!({
                    "status": "triggered",
                    "timestamp": "2024-01-01T00:00:00Z"
                }),
                ttl: None,
            },
            description: Some("Set test output value".to_string()),
        };

        let rule = create_rule_with_action("test_set_value", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_set_value").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 1);
        assert!(result.actions_executed[0].contains("Set value"));

        // 验证值是否被设置
        let output = store.get_string("test.output").await.unwrap();
        assert!(output.is_some());
        let output_value: serde_json::Value = serde_json::from_str(&output.unwrap()).unwrap();
        assert_eq!(output_value["status"], "triggered");

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("test.output").await.ok();
        store.delete("rulesrv:rule:test_set_value").await.ok();
    }

    #[tokio::test]
    async fn test_notify_action() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::Notify,
            config: ActionConfig::Notify {
                level: "warning".to_string(),
                message: "Test notification from rule engine".to_string(),
                recipients: Some(vec!["admin@example.com".to_string()]),
            },
            description: Some("Send test notification".to_string()),
        };

        let rule = create_rule_with_action("test_notify", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_notify").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 1);
        assert!(result.actions_executed[0].contains("Notification sent"));

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("rulesrv:rule:test_notify").await.ok();
    }

    #[tokio::test]
    async fn test_multiple_actions() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let rule = Rule {
            id: "test_multiple_actions".to_string(),
            name: "Test Multiple Actions".to_string(),
            description: Some("Test rule with multiple actions".to_string()),
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.trigger".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(true),
                    description: None,
                }],
            },
            actions: vec![
                RuleAction {
                    action_type: ActionType::SetValue,
                    config: ActionConfig::SetValue {
                        key: "test.step1".to_string(),
                        value: json!("completed"),
                        ttl: None,
                    },
                    description: Some("First action".to_string()),
                },
                RuleAction {
                    action_type: ActionType::Publish,
                    config: ActionConfig::Publish {
                        channel: "test:multi".to_string(),
                        message: "Step 1 completed".to_string(),
                    },
                    description: Some("Second action".to_string()),
                },
                RuleAction {
                    action_type: ActionType::SetValue,
                    config: ActionConfig::SetValue {
                        key: "test.step2".to_string(),
                        value: json!("completed"),
                        ttl: None,
                    },
                    description: Some("Third action".to_string()),
                },
            ],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_multiple_actions").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 3);

        // 验证所有动作都被执行
        let step1 = store.get_string("test.step1").await.unwrap();
        assert_eq!(step1, Some("\"completed\"".to_string()));

        let step2 = store.get_string("test.step2").await.unwrap();
        assert_eq!(step2, Some("\"completed\"".to_string()));

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("test.step1").await.ok();
        store.delete("test.step2").await.ok();
        store
            .delete("rulesrv:rule:test_multiple_actions")
            .await
            .ok();
    }

    #[tokio::test]
    async fn test_action_with_numeric_value() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.numeric".to_string(),
                value: json!(42.5),
                ttl: None,
            },
            description: Some("Set numeric value".to_string()),
        };

        let rule = create_rule_with_action("test_numeric_action", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_numeric_action").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);

        // 验证数值被正确设置
        let value = store.get_string("test.numeric").await.unwrap();
        assert_eq!(value, Some("42.5".to_string()));

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("test.numeric").await.ok();
        store.delete("rulesrv:rule:test_numeric_action").await.ok();
    }

    #[tokio::test]
    async fn test_action_with_array_value() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        let action = RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.array".to_string(),
                value: json!(["item1", "item2", "item3"]),
                ttl: None,
            },
            description: Some("Set array value".to_string()),
        };

        let rule = create_rule_with_action("test_array_action", action);
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_array_action").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);

        // 验证数组被正确设置
        let value = store.get_string("test.array").await.unwrap();
        assert!(value.is_some());
        let array_value: Vec<String> = serde_json::from_str(&value.unwrap()).unwrap();
        assert_eq!(array_value.len(), 3);
        assert_eq!(array_value[0], "item1");

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("test.array").await.ok();
        store.delete("rulesrv:rule:test_array_action").await.ok();
    }

    #[tokio::test]
    async fn test_action_execution_order() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.trigger", "true").await.unwrap();

        // 创建一个规则，其动作有依赖关系
        let rule = Rule {
            id: "test_action_order".to_string(),
            name: "Test Action Order".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.trigger".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(true),
                    description: None,
                }],
            },
            actions: vec![
                RuleAction {
                    action_type: ActionType::SetValue,
                    config: ActionConfig::SetValue {
                        key: "test.counter".to_string(),
                        value: json!(1),
                        ttl: None,
                    },
                    description: Some("Initialize counter".to_string()),
                },
                RuleAction {
                    action_type: ActionType::SetValue,
                    config: ActionConfig::SetValue {
                        key: "test.status".to_string(),
                        value: json!("started"),
                        ttl: None,
                    },
                    description: Some("Set status".to_string()),
                },
                RuleAction {
                    action_type: ActionType::SetValue,
                    config: ActionConfig::SetValue {
                        key: "test.final".to_string(),
                        value: json!("completed"),
                        ttl: None,
                    },
                    description: Some("Final action".to_string()),
                },
            ],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_action_order").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);
        assert_eq!(result.actions_executed.len(), 3);

        // 验证所有值都被设置
        assert!(store.get_string("test.counter").await.unwrap().is_some());
        assert!(store.get_string("test.status").await.unwrap().is_some());
        assert!(store.get_string("test.final").await.unwrap().is_some());

        // 清理
        store.delete("test.trigger").await.ok();
        store.delete("test.counter").await.ok();
        store.delete("test.status").await.ok();
        store.delete("test.final").await.ok();
        store.delete("rulesrv:rule:test_action_order").await.ok();
    }
}
