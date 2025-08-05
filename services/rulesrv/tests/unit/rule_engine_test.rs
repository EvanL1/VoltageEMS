#[cfg(test)]
mod rule_engine_tests {
    use rulesrv::engine::{
        ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator,
        Rule, RuleAction, RuleEngine,
    };
    use rulesrv::redis::RedisStore;
    use serde_json::json;
    use std::sync::Arc;

    // 创建测试用的Redis存储
    async fn create_test_store() -> Arc<RedisStore> {
        Arc::new(
            RedisStore::new("redis://localhost:6379", Some("test"))
                .expect("Failed to create test store"),
        )
    }

    // 创建简单的测试规则
    fn create_test_rule(id: &str, name: &str) -> Rule {
        Rule {
            id: id.to_string(),
            name: name.to_string(),
            description: Some("Test rule".to_string()),
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: json!(10.0),
                    description: Some("Test condition".to_string()),
                }],
            },
            actions: vec![RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "test.result".to_string(),
                    value: json!("triggered"),
                    ttl: None,
                },
                description: Some("Set test result".to_string()),
            }],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_condition_evaluation_equals() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 设置测试数据
        store.set_string("test.value", "100").await.unwrap();

        let rule = Rule {
            id: "test_equals".to_string(),
            name: "Test Equals".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(100.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        // 存储规则
        engine.store_rule(&rule).await.unwrap();

        // 执行规则
        let result = engine.execute_rule("test_equals").await.unwrap();
        assert!(result.conditions_met);
        assert!(result.success);

        // 清理
        store.delete("test.value").await.ok();
        store.delete("rulesrv:rule:test_equals").await.ok();
    }

    #[tokio::test]
    async fn test_condition_evaluation_not_equals() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.value", "100").await.unwrap();

        let rule = Rule {
            id: "test_not_equals".to_string(),
            name: "Test Not Equals".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::NotEquals,
                    value: json!(50.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_not_equals").await.unwrap();
        assert!(result.conditions_met);

        // 清理
        store.delete("test.value").await.ok();
        store.delete("rulesrv:rule:test_not_equals").await.ok();
    }

    #[tokio::test]
    async fn test_condition_evaluation_greater_than() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.value", "100").await.unwrap();

        let rule = Rule {
            id: "test_gt".to_string(),
            name: "Test Greater Than".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: json!(50.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_gt").await.unwrap();
        assert!(result.conditions_met);

        // 清理
        store.delete("test.value").await.ok();
        store.delete("rulesrv:rule:test_gt").await.ok();
    }

    #[tokio::test]
    async fn test_condition_evaluation_less_than() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.value", "10").await.unwrap();

        let rule = Rule {
            id: "test_lt".to_string(),
            name: "Test Less Than".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: json!(50.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_lt").await.unwrap();
        assert!(result.conditions_met);

        // 清理
        store.delete("test.value").await.ok();
        store.delete("rulesrv:rule:test_lt").await.ok();
    }

    #[tokio::test]
    async fn test_condition_evaluation_contains() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store
            .set_string("test.message", "Error: Connection failed")
            .await
            .unwrap();

        let rule = Rule {
            id: "test_contains".to_string(),
            name: "Test Contains".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.message".to_string(),
                    operator: ComparisonOperator::Contains,
                    value: json!("Error"),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_contains").await.unwrap();
        assert!(result.conditions_met);

        // 清理
        store.delete("test.message").await.ok();
        store.delete("rulesrv:rule:test_contains").await.ok();
    }

    #[tokio::test]
    async fn test_logic_operator_and() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 设置测试数据
        store.set_string("test.a", "100").await.unwrap();
        store.set_string("test.b", "200").await.unwrap();

        let rule = Rule {
            id: "test_and".to_string(),
            name: "Test AND Logic".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![
                    Condition {
                        source: "test.a".to_string(),
                        operator: ComparisonOperator::GreaterThan,
                        value: json!(50.0),
                        description: None,
                    },
                    Condition {
                        source: "test.b".to_string(),
                        operator: ComparisonOperator::GreaterThan,
                        value: json!(150.0),
                        description: None,
                    },
                ],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_and").await.unwrap();
        assert!(result.conditions_met);

        // 测试一个条件不满足
        store.set_string("test.a", "30").await.unwrap();
        let result = engine.execute_rule("test_and").await.unwrap();
        assert!(!result.conditions_met);

        // 清理
        store.delete("test.a").await.ok();
        store.delete("test.b").await.ok();
        store.delete("rulesrv:rule:test_and").await.ok();
    }

    #[tokio::test]
    async fn test_logic_operator_or() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 设置测试数据
        store.set_string("test.a", "100").await.unwrap();
        store.set_string("test.b", "50").await.unwrap();

        let rule = Rule {
            id: "test_or".to_string(),
            name: "Test OR Logic".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::Or,
                conditions: vec![
                    Condition {
                        source: "test.a".to_string(),
                        operator: ComparisonOperator::GreaterThan,
                        value: json!(200.0),
                        description: None,
                    },
                    Condition {
                        source: "test.b".to_string(),
                        operator: ComparisonOperator::LessThan,
                        value: json!(100.0),
                        description: None,
                    },
                ],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_or").await.unwrap();
        assert!(result.conditions_met); // 第二个条件满足

        // 两个条件都不满足
        store.set_string("test.a", "50").await.unwrap();
        store.set_string("test.b", "200").await.unwrap();
        let result = engine.execute_rule("test_or").await.unwrap();
        assert!(!result.conditions_met);

        // 清理
        store.delete("test.a").await.ok();
        store.delete("test.b").await.ok();
        store.delete("rulesrv:rule:test_or").await.ok();
    }

    #[tokio::test]
    async fn test_hash_field_access() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 设置Hash数据（模拟comsrv格式）
        store
            .set_hash_field("comsrv:1001:T", "1", "230.5")
            .await
            .unwrap();

        let rule = Rule {
            id: "test_hash".to_string(),
            name: "Test Hash Field".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "comsrv:1001:T.1".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: json!(220.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();
        let result = engine.execute_rule("test_hash").await.unwrap();
        assert!(result.conditions_met);

        // 清理
        store.delete("comsrv:1001:T").await.ok();
        store.delete("rulesrv:rule:test_hash").await.ok();
    }

    #[tokio::test]
    async fn test_cooldown_period() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        store.set_string("test.value", "100").await.unwrap();

        let rule = Rule {
            id: "test_cooldown".to_string(),
            name: "Test Cooldown".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "test.value".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(100.0),
                    description: None,
                }],
            },
            actions: vec![RuleAction {
                action_type: ActionType::SetValue,
                config: ActionConfig::SetValue {
                    key: "test.executed".to_string(),
                    value: json!(true),
                    ttl: None,
                },
                description: None,
            }],
            enabled: true,
            priority: 1,
            cooldown_seconds: Some(2), // 2秒冷却期
        };

        engine.store_rule(&rule).await.unwrap();

        // 第一次执行应该成功
        let result1 = engine.execute_rule("test_cooldown").await.unwrap();
        assert!(result1.conditions_met);
        assert!(result1.success);

        // 立即再次执行应该被冷却期阻止
        let result2 = engine.execute_rule("test_cooldown").await.unwrap();
        assert!(!result2.conditions_met); // 因为冷却期
        assert!(result2.error.is_some());

        // 等待冷却期结束
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // 现在应该可以再次执行
        let result3 = engine.execute_rule("test_cooldown").await.unwrap();
        assert!(result3.conditions_met);
        assert!(result3.success);

        // 清理
        store.delete("test.value").await.ok();
        store.delete("test.executed").await.ok();
        store.delete("rulesrv:rule:test_cooldown").await.ok();
    }

    #[tokio::test]
    async fn test_disabled_rule() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        let mut rule = create_test_rule("test_disabled", "Test Disabled Rule");
        rule.enabled = false;

        engine.store_rule(&rule).await.unwrap();

        // 执行禁用的规则应该返回错误
        let result = engine.execute_rule("test_disabled").await.unwrap();
        assert!(!result.conditions_met);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("disabled"));

        // 清理
        store.delete("rulesrv:rule:test_disabled").await.ok();
    }

    #[tokio::test]
    async fn test_nonexistent_rule() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store);

        // 尝试执行不存在的规则
        let result = engine.execute_rule("nonexistent_rule").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_data_source() {
        let store = create_test_store().await;
        let mut engine = RuleEngine::new(store.clone());

        // 创建引用不存在数据源的规则
        let rule = Rule {
            id: "test_missing_source".to_string(),
            name: "Test Missing Source".to_string(),
            description: None,
            conditions: ConditionGroup {
                operator: LogicOperator::And,
                conditions: vec![Condition {
                    source: "nonexistent.value".to_string(),
                    operator: ComparisonOperator::Equals,
                    value: json!(100.0),
                    description: None,
                }],
            },
            actions: vec![],
            enabled: true,
            priority: 1,
            cooldown_seconds: None,
        };

        engine.store_rule(&rule).await.unwrap();

        // 当数据源不存在时，条件应该评估为false
        let result = engine.execute_rule("test_missing_source").await.unwrap();
        assert!(!result.conditions_met);
        assert!(result.success); // 但执行本身是成功的

        // 清理
        store.delete("rulesrv:rule:test_missing_source").await.ok();
    }
}
