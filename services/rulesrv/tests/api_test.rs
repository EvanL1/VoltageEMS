use anyhow::Result;
use axum::http::StatusCode;
use rulesrv::{
    api::ApiServer,
    engine::RuleExecutor,
    models::{ActionType, ConditionType, Rule, RuleGroup, TriggerType},
    redis::RedisStore,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 创建测试客户端
fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

/// 获取测试服务器地址
fn test_server_url(path: &str) -> String {
    let port = std::env::var("TEST_API_PORT").unwrap_or_else(|_| "8091".to_string());
    format!("http://localhost:{}{}", port, path)
}

/// 启动测试 API 服务器
async fn start_test_server() -> Result<tokio::task::JoinHandle<()>> {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let store = Arc::new(RedisStore::new(&redis_url, Some("rulesrv_api_test"))?);
    let executor = Arc::new(RuleExecutor::new(store.clone()));
    let port = std::env::var("TEST_API_PORT")
        .unwrap_or_else(|_| "8091".to_string())
        .parse()
        .unwrap();

    let server = ApiServer::new(executor, store, port);

    let handle = tokio::spawn(async move {
        let _ = server.start().await;
    });

    // 等待服务器启动
    sleep(Duration::from_millis(500)).await;

    Ok(handle)
}

#[tokio::test]
async fn test_health_check() -> Result<()> {
    let _server = start_test_server().await?;
    let client = create_test_client();

    let response = client.get(test_server_url("/health")).send().await?;

    assert_eq!(response.status(), StatusCode::OK);

    let health: serde_json::Value = response.json().await?;
    assert_eq!(health["status"], "ok");
    assert!(health["redis_connected"].as_bool().unwrap());

    Ok(())
}

#[tokio::test]
async fn test_rule_crud_via_api() -> Result<()> {
    let _server = start_test_server().await?;
    let client = create_test_client();

    // 创建规则组
    let group = RuleGroup {
        id: "api_test_group".to_string(),
        name: "API Test Group".to_string(),
        description: Some("Test group via API".to_string()),
        enabled: true,
    };

    let response = client
        .post(test_server_url("/api/v1/groups"))
        .json(&json!({ "group": group }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 创建规则
    let rule = Rule {
        id: "api_test_rule".to_string(),
        name: "API Test Rule".to_string(),
        description: Some("Test rule via API".to_string()),
        group_id: Some("api_test_group".to_string()),
        enabled: true,
        priority: 10,
        trigger: TriggerType::DataChange {
            sources: vec!["test_source".to_string()],
        },
        conditions: vec![ConditionType::Expression {
            expression: "value > 10".to_string(),
        }],
        actions: vec![ActionType::Alarm {
            level: "info".to_string(),
            message: "Test alarm".to_string(),
        }],
        metadata: Default::default(),
    };

    let response = client
        .post(test_server_url("/api/v1/rules"))
        .json(&json!({ "rule": rule }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 获取规则
    let response = client
        .get(test_server_url("/api/v1/rules/api_test_rule"))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value = response.json().await?;
    assert_eq!(data["data"]["name"], "API Test Rule");

    // 更新规则
    let mut updated_rule = rule.clone();
    updated_rule.name = "Updated API Test Rule".to_string();

    let response = client
        .put(test_server_url("/api/v1/rules/api_test_rule"))
        .json(&json!({ "rule": updated_rule }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 列出规则
    let response = client.get(test_server_url("/api/v1/rules")).send().await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value = response.json().await?;
    assert!(data["data"].as_array().unwrap().len() > 0);

    // 删除规则
    let response = client
        .delete(test_server_url("/api/v1/rules/api_test_rule"))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 删除规则组
    let response = client
        .delete(test_server_url("/api/v1/groups/api_test_group"))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_rule_execution_via_api() -> Result<()> {
    let _server = start_test_server().await?;
    let client = create_test_client();

    // 创建测试规则
    let rule = Rule {
        id: "exec_test_rule".to_string(),
        name: "Execution Test Rule".to_string(),
        description: Some("Rule for execution testing".to_string()),
        group_id: None,
        enabled: true,
        priority: 10,
        trigger: TriggerType::Manual,
        conditions: vec![ConditionType::Expression {
            expression: "input_value > 50".to_string(),
        }],
        actions: vec![ActionType::Notification {
            channels: vec!["test".to_string()],
            message: "Threshold exceeded".to_string(),
        }],
        metadata: Default::default(),
    };

    // 创建规则
    let response = client
        .post(test_server_url("/api/v1/rules"))
        .json(&json!({ "rule": rule }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 执行规则
    let response = client
        .post(test_server_url("/api/v1/rules/exec_test_rule/execute"))
        .json(&json!({ "input": { "input_value": 75 } }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value = response.json().await?;
    assert_eq!(data["data"]["status"], "success");

    // 获取执行历史
    let response = client
        .get(test_server_url("/api/v1/rules/exec_test_rule/history"))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 清理
    let _ = client
        .delete(test_server_url("/api/v1/rules/exec_test_rule"))
        .send()
        .await?;

    Ok(())
}

#[tokio::test]
async fn test_rule_test_endpoint() -> Result<()> {
    let _server = start_test_server().await?;
    let client = create_test_client();

    // 测试规则（不保存）
    let rule = Rule {
        id: "temporary_test_rule".to_string(),
        name: "Temporary Test Rule".to_string(),
        description: Some("Rule for testing without saving".to_string()),
        group_id: None,
        enabled: true,
        priority: 10,
        trigger: TriggerType::Manual,
        conditions: vec![ConditionType::Expression {
            expression: "test_value == 42".to_string(),
        }],
        actions: vec![ActionType::Alarm {
            level: "info".to_string(),
            message: "Test successful".to_string(),
        }],
        metadata: Default::default(),
    };

    let response = client
        .post(test_server_url("/api/v1/rules/test"))
        .json(&json!({
            "rule": rule,
            "input": { "test_value": 42 }
        }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value = response.json().await?;
    assert_eq!(data["data"]["status"], "success");

    // 验证规则没有被保存
    let response = client
        .get(test_server_url("/api/v1/rules/temporary_test_rule"))
        .send()
        .await?;

    // 应该返回错误，因为规则不存在
    assert_ne!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_group_operations_via_api() -> Result<()> {
    let _server = start_test_server().await?;
    let client = create_test_client();

    // 创建规则组
    let group = RuleGroup {
        id: "group_ops_test".to_string(),
        name: "Group Operations Test".to_string(),
        description: Some("Test group operations".to_string()),
        enabled: true,
    };

    let response = client
        .post(test_server_url("/api/v1/groups"))
        .json(&json!({ "group": group }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 创建规则在组内
    let rule = Rule {
        id: "group_rule_test".to_string(),
        name: "Group Rule Test".to_string(),
        description: Some("Rule in group".to_string()),
        group_id: Some("group_ops_test".to_string()),
        enabled: true,
        priority: 10,
        trigger: TriggerType::Manual,
        conditions: vec![],
        actions: vec![],
        metadata: Default::default(),
    };

    let response = client
        .post(test_server_url("/api/v1/rules"))
        .json(&json!({ "rule": rule }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    // 获取组内规则
    let response = client
        .get(test_server_url("/api/v1/groups/group_ops_test/rules"))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value = response.json().await?;
    assert_eq!(data["data"].as_array().unwrap().len(), 1);

    // 清理
    let _ = client
        .delete(test_server_url("/api/v1/rules/group_rule_test"))
        .send()
        .await?;

    let _ = client
        .delete(test_server_url("/api/v1/groups/group_ops_test"))
        .send()
        .await?;

    Ok(())
}
