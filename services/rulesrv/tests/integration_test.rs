use anyhow::Result;
use rulesrv::{
    actions::{ActionHandler, ControlActionHandler},
    engine::{RuleEngine, RuleExecutor},
    models::{Rule, RuleGroup, TriggerType, ConditionType, ActionType},
    redis::{RedisStore, RedisSubscriber},
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 创建测试用的 Redis Store
async fn create_test_store() -> Result<Arc<RedisStore>> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let store = Arc::new(RedisStore::new(&redis_url, Some("rulesrv_test"))?);
    Ok(store)
}

/// 清理测试数据
async fn cleanup_test_data(store: &RedisStore) -> Result<()> {
    // 删除所有测试规则
    let rules = store.list_rules().await?;
    for rule in rules {
        let _ = store.delete_rule(&rule.id).await;
    }
    
    // 删除所有测试规则组
    let groups = store.list_rule_groups().await?;
    for group in groups {
        let _ = store.delete_rule_group(&group.id).await;
    }
    
    Ok(())
}

#[tokio::test]
async fn test_rule_crud_operations() -> Result<()> {
    let store = create_test_store().await?;
    
    // 清理之前的测试数据
    cleanup_test_data(&store).await?;
    
    // 创建规则组
    let group = RuleGroup {
        id: "test_group".to_string(),
        name: "Test Group".to_string(),
        description: Some("Test rule group".to_string()),
        enabled: true,
    };
    store.save_rule_group(&group).await?;
    
    // 创建规则
    let rule = Rule {
        id: "test_rule_1".to_string(),
        name: "Test Rule 1".to_string(),
        description: Some("Test rule for CRUD operations".to_string()),
        group_id: Some("test_group".to_string()),
        enabled: true,
        priority: 10,
        trigger: TriggerType::DataChange {
            sources: vec!["device:sensor1:temperature".to_string()],
        },
        conditions: vec![ConditionType::Threshold {
            source: "device:sensor1:temperature".to_string(),
            operator: ">".to_string(),
            value: json!(30.0),
            duration: None,
        }],
        actions: vec![ActionType::Control {
            config: json!({
                "channel_id": 1001,
                "point_type": "c",
                "point_id": 30001,
                "value": true
            }),
        }],
        metadata: Default::default(),
    };
    
    // 保存规则
    store.save_rule(&rule).await?;
    
    // 读取规则
    let retrieved = store.get_rule(&rule.id).await?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.name, rule.name);
    
    // 更新规则
    let mut updated_rule = retrieved.clone();
    updated_rule.name = "Updated Test Rule 1".to_string();
    store.save_rule(&updated_rule).await?;
    
    // 验证更新
    let updated = store.get_rule(&rule.id).await?.unwrap();
    assert_eq!(updated.name, "Updated Test Rule 1");
    
    // 列出规则
    let rules = store.list_rules().await?;
    assert!(!rules.is_empty());
    
    // 获取组内规则
    let group_rules = store.get_group_rules("test_group").await?;
    assert_eq!(group_rules.len(), 1);
    
    // 删除规则
    let deleted = store.delete_rule(&rule.id).await?;
    assert!(deleted);
    
    // 验证删除
    let deleted_rule = store.get_rule(&rule.id).await?;
    assert!(deleted_rule.is_none());
    
    // 清理
    cleanup_test_data(&store).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_rule_execution() -> Result<()> {
    let store = create_test_store().await?;
    cleanup_test_data(&store).await?;
    
    // 创建规则执行器
    let executor = Arc::new(RuleExecutor::new(store.clone()));
    
    // 注册控制动作处理器
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let control_handler = ControlActionHandler::new(&redis_url)?;
    executor.register_action_handler(control_handler).await?;
    
    // 创建简单的阈值规则
    let rule = Rule {
        id: "test_threshold_rule".to_string(),
        name: "Temperature Threshold Rule".to_string(),
        description: Some("Triggers when temperature exceeds threshold".to_string()),
        group_id: None,
        enabled: true,
        priority: 10,
        trigger: TriggerType::DataChange {
            sources: vec!["temperature".to_string()],
        },
        conditions: vec![ConditionType::Expression {
            expression: "temperature > 30".to_string(),
        }],
        actions: vec![ActionType::Control {
            config: json!({
                "channel_id": 1001,
                "point_type": "c",
                "point_id": 30001,
                "value": false
            }),
        }],
        metadata: Default::default(),
    };
    
    // 保存规则
    store.save_rule(&rule).await?;
    
    // 执行规则（温度低于阈值）
    let input = json!({ "temperature": 25.0 });
    let result = executor.execute_rule(&rule.id, Some(input)).await?;
    
    // 验证结果
    assert_eq!(result["status"], "success");
    
    // 执行规则（温度高于阈值）
    let input = json!({ "temperature": 35.0 });
    let result = executor.execute_rule(&rule.id, Some(input)).await?;
    
    // 验证结果
    assert_eq!(result["status"], "success");
    
    // 清理
    cleanup_test_data(&store).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_rule_engine_with_subscription() -> Result<()> {
    let store = create_test_store().await?;
    cleanup_test_data(&store).await?;
    
    // 创建规则引擎
    let mut engine = RuleEngine::new(store.clone());
    
    // 注册控制动作处理器
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let control_handler = ControlActionHandler::new(&redis_url)?;
    engine.register_action_handler(Box::new(control_handler));
    
    // 创建规则
    let rule = Rule {
        id: "test_engine_rule".to_string(),
        name: "Engine Test Rule".to_string(),
        description: Some("Rule for testing engine functionality".to_string()),
        group_id: None,
        enabled: true,
        priority: 10,
        trigger: TriggerType::DataChange {
            sources: vec!["modsrv:model1:output".to_string()],
        },
        conditions: vec![ConditionType::Expression {
            expression: "value > 100".to_string(),
        }],
        actions: vec![ActionType::Alarm {
            level: "warning".to_string(),
            message: "Value exceeded 100".to_string(),
        }],
        metadata: Default::default(),
    };
    
    store.save_rule(&rule).await?;
    
    // 加载规则到引擎
    engine.load_rules().await?;
    
    // 创建订阅器
    let engine_arc = Arc::new(tokio::sync::RwLock::new(engine));
    let mut subscriber = RedisSubscriber::new(&redis_url, engine_arc.clone())?;
    
    // 启动订阅器
    subscriber.start().await?;
    
    // 等待订阅器启动
    sleep(Duration::from_millis(100)).await;
    
    // TODO: 发布测试数据到 Redis 并验证规则触发
    // 这需要实际的 Redis 发布功能
    
    // 停止订阅器
    subscriber.stop().await?;
    
    // 清理
    cleanup_test_data(&store).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_execution_history() -> Result<()> {
    let store = create_test_store().await?;
    cleanup_test_data(&store).await?;
    
    // 创建执行历史记录
    let history = rulesrv::redis::store::ExecutionHistory {
        id: "exec_1".to_string(),
        rule_id: "test_rule".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        triggered: true,
        actions_executed: vec!["control_action".to_string()],
        success: true,
        error: None,
        duration_ms: 50,
        context: Default::default(),
    };
    
    // 保存执行历史
    store.save_execution_history(&history.rule_id, &history).await?;
    
    // 获取执行历史
    let histories = store.get_execution_history(&history.rule_id, 10).await?;
    assert!(!histories.is_empty());
    assert_eq!(histories[0].id, history.id);
    
    // 清理
    cleanup_test_data(&store).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_complex_rule_with_dag() -> Result<()> {
    let store = create_test_store().await?;
    cleanup_test_data(&store).await?;
    
    // 创建 DAG 规则
    let dag_rule = rulesrv::rules::definition::DagRule {
        id: "complex_dag_rule".to_string(),
        name: "Complex DAG Rule".to_string(),
        description: Some("Rule with DAG execution".to_string()),
        enabled: true,
        nodes: vec![
            rulesrv::rules::definition::NodeDefinition {
                id: "input1".to_string(),
                node_type: rulesrv::rules::definition::NodeType::Input,
                config: json!({
                    "device_id": "sensor1",
                    "parameter": "temperature"
                }),
            },
            rulesrv::rules::definition::NodeDefinition {
                id: "input2".to_string(),
                node_type: rulesrv::rules::definition::NodeType::Input,
                config: json!({
                    "device_id": "sensor2",
                    "parameter": "humidity"
                }),
            },
            rulesrv::rules::definition::NodeDefinition {
                id: "condition1".to_string(),
                node_type: rulesrv::rules::definition::NodeType::Condition,
                config: json!({
                    "expression": "$input1 > 30"
                }),
            },
            rulesrv::rules::definition::NodeDefinition {
                id: "action1".to_string(),
                node_type: rulesrv::rules::definition::NodeType::Action,
                config: json!({
                    "action_type": "control",
                    "channel_id": 1001,
                    "point_type": "c",
                    "point_id": 30001,
                    "value": true
                }),
            },
        ],
        edges: vec![
            rulesrv::rules::definition::EdgeDefinition {
                from: "input1".to_string(),
                to: "condition1".to_string(),
                condition: None,
            },
            rulesrv::rules::definition::EdgeDefinition {
                from: "condition1".to_string(),
                to: "action1".to_string(),
                condition: Some("$condition1 == true".to_string()),
            },
        ],
    };
    
    // 保存 DAG 规则
    let dag_json = serde_json::to_string(&dag_rule)?;
    store.set_string(&format!("rule:{}", dag_rule.id), &dag_json)?;
    
    // 创建执行器并执行
    let executor = Arc::new(RuleExecutor::new(store.clone()));
    
    // 注册动作处理器
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let control_handler = ControlActionHandler::new(&redis_url)?;
    executor.register_action_handler(control_handler).await?;
    
    // 设置设备参数（模拟）
    store.set_string("device:status:sensor1:temperature", "35")?;
    store.set_string("device:status:sensor2:humidity", "60")?;
    
    // 执行 DAG 规则
    let result = executor.execute_rule(&dag_rule.id, None).await?;
    assert_eq!(result["status"], "success");
    
    // 清理
    cleanup_test_data(&store).await?;
    
    Ok(())
}