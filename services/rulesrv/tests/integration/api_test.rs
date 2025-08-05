use axum::http::StatusCode;
use reqwest;
use rulesrv::api::{create_routes, ApiState};
use rulesrv::engine::{
    ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator, Rule,
    RuleAction, RuleEngine,
};
use rulesrv::redis::RedisStore;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

async fn setup_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let store = Arc::new(
        RedisStore::new("redis://localhost:6379", Some("api_test"))
            .expect("Failed to create Redis store"),
    );
    let engine = Arc::new(RwLock::new(RuleEngine::new(store.clone())));

    let api_state = Arc::new(ApiState {
        engine: engine.clone(),
        store: store.clone(),
    });

    let app = create_routes(api_state);

    // Find available port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for server to start
    sleep(Duration::from_millis(100)).await;

    (base_url, handle)
}

async fn cleanup_test_data(prefix: &str) {
    let store = RedisStore::new("redis://localhost:6379", Some("api_test"))
        .expect("Failed to create Redis store");

    let keys = store.scan_keys(&format!("{}*", prefix)).await.unwrap();
    for key in keys {
        store.delete(&key).await.ok();
    }
}

#[tokio::test]
async fn test_health_endpoint() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["engine"], "rule_engine");
    assert!(body["redis_connected"].is_boolean());
}

#[tokio::test]
async fn test_example_rules_endpoint() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/examples", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["examples"].is_array());
    let examples = body["examples"].as_array().unwrap();
    assert!(examples.len() > 0);

    // Verify example rule structure
    let first_example = &examples[0];
    assert!(first_example["id"].is_string());
    assert!(first_example["name"].is_string());
    assert!(first_example["conditions"].is_object());
    assert!(first_example["actions"].is_array());
}

#[tokio::test]
async fn test_create_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    let rule = Rule {
        id: "test_api_rule".to_string(),
        name: "Test API Rule".to_string(),
        description: Some("Rule created via API".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.value".to_string(),
                operator: ComparisonOperator::GreaterThan,
                value: json!(100.0),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.result".to_string(),
                value: json!("triggered"),
                ttl: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 1,
        cooldown_seconds: Some(60),
    };

    let response = client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["id"], "test_api_rule");
    assert_eq!(body["data"]["name"], "Test API Rule");

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_api_rule").await;
}

#[tokio::test]
async fn test_list_rules() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create a test rule first
    let rule = Rule {
        id: "test_list_rule".to_string(),
        name: "Test List Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // List rules
    let response = client
        .get(&format!("{}/rules", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["data"].is_array());

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_list_rule").await;
}

#[tokio::test]
async fn test_get_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create a test rule
    let rule = Rule {
        id: "test_get_rule".to_string(),
        name: "Test Get Rule".to_string(),
        description: Some("Test description".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 2,
        cooldown_seconds: Some(120),
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Get the rule
    let response = client
        .get(&format!("{}/rules/test_get_rule", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["id"], "test_get_rule");
    assert_eq!(body["data"]["name"], "Test Get Rule");
    assert_eq!(body["data"]["priority"], 2);

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_get_rule").await;
}

#[tokio::test]
async fn test_update_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create initial rule
    let rule = Rule {
        id: "test_update_rule".to_string(),
        name: "Original Name".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Update the rule
    let updated_rule = Rule {
        id: "test_update_rule".to_string(),
        name: "Updated Name".to_string(),
        description: Some("Updated description".to_string()),
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: false,
        priority: 5,
        cooldown_seconds: Some(300),
    };

    let response = client
        .put(&format!("{}/rules/test_update_rule", base_url))
        .json(&json!({ "rule": updated_rule }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["name"], "Updated Name");
    assert_eq!(body["data"]["enabled"], false);
    assert_eq!(body["data"]["priority"], 5);

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_update_rule").await;
}

#[tokio::test]
async fn test_delete_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create a rule to delete
    let rule = Rule {
        id: "test_delete_rule".to_string(),
        name: "Rule to Delete".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Delete the rule
    let response = client
        .delete(&format!("{}/rules/test_delete_rule", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["deleted"], true);

    // Verify it's deleted
    let get_response = client
        .get(&format!("{}/rules/test_delete_rule", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_execute_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Setup test data in Redis
    let store = RedisStore::new("redis://localhost:6379", Some("api_test"))
        .expect("Failed to create Redis store");
    store.set_string("test.execute.value", "150").await.unwrap();

    // Create a rule
    let rule = Rule {
        id: "test_execute_rule".to_string(),
        name: "Test Execute Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.execute.value".to_string(),
                operator: ComparisonOperator::GreaterThan,
                value: json!(100.0),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::SetValue,
            config: ActionConfig::SetValue {
                key: "test.execute.result".to_string(),
                value: json!("executed"),
                ttl: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Execute the rule
    let response = client
        .post(&format!("{}/rules/test_execute_rule/execute", base_url))
        .json(&json!({ "context": null }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["rule_id"], "test_execute_rule");
    assert_eq!(body["data"]["conditions_met"], true);
    assert_eq!(body["data"]["success"], true);
    assert!(body["data"]["actions_executed"].as_array().unwrap().len() > 0);

    // Verify action was executed
    let result = store.get_string("test.execute.result").await.unwrap();
    assert_eq!(result, Some("\"executed\"".to_string()));

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_execute_rule").await;
    cleanup_test_data("test.execute").await;
}

#[tokio::test]
async fn test_test_rule() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Setup test data
    let store = RedisStore::new("redis://localhost:6379", Some("api_test"))
        .expect("Failed to create Redis store");
    store.set_string("test.test.value", "75").await.unwrap();

    // Test a rule without saving it
    let rule = Rule {
        id: "test_temp_rule".to_string(),
        name: "Temporary Test Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![Condition {
                source: "test.test.value".to_string(),
                operator: ComparisonOperator::LessThan,
                value: json!(100.0),
                description: None,
            }],
        },
        actions: vec![RuleAction {
            action_type: ActionType::Notify,
            config: ActionConfig::Notify {
                level: "info".to_string(),
                message: "Test notification".to_string(),
                recipients: None,
            },
            description: None,
        }],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    let response = client
        .post(&format!("{}/rules/test", base_url))
        .json(&json!({ "rule": rule, "context": null }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["data"]["rule_id"]
        .as_str()
        .unwrap()
        .starts_with("test_"));
    assert_eq!(body["data"]["conditions_met"], true);
    assert_eq!(body["data"]["success"], true);

    // Verify the rule was not saved
    let get_response = client
        .get(&format!("{}/rules/test_temp_rule", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Cleanup
    cleanup_test_data("test.test").await;
}

#[tokio::test]
async fn test_get_rule_stats() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create and execute a rule first
    let rule = Rule {
        id: "test_stats_rule".to_string(),
        name: "Test Stats Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Execute the rule
    client
        .post(&format!("{}/rules/test_stats_rule/execute", base_url))
        .json(&json!({ "context": null }))
        .send()
        .await
        .unwrap();

    // Get stats
    let response = client
        .get(&format!("{}/rules/test_stats_rule/stats", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["data"]["last_execution"].is_string());
    assert!(body["data"]["last_result"].is_boolean());
    assert!(body["data"]["conditions_met"].is_boolean());

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_stats_rule").await;
}

#[tokio::test]
async fn test_get_rule_history() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Create a rule
    let rule = Rule {
        id: "test_history_rule".to_string(),
        name: "Test History Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": rule }))
        .send()
        .await
        .unwrap();

    // Get history
    let response = client
        .get(&format!("{}/rules/test_history_rule/history", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["rule_id"], "test_history_rule");
    assert!(body["data"]["history"].is_array());

    // Cleanup
    cleanup_test_data("rulesrv:rule:test_history_rule").await;
}

#[tokio::test]
async fn test_error_handling() {
    let (base_url, _handle) = setup_test_server().await;
    let client = reqwest::Client::new();

    // Test creating rule with empty ID
    let invalid_rule = Rule {
        id: "".to_string(),
        name: "Invalid Rule".to_string(),
        description: None,
        conditions: ConditionGroup {
            operator: LogicOperator::And,
            conditions: vec![],
        },
        actions: vec![],
        enabled: true,
        priority: 1,
        cooldown_seconds: None,
    };

    let response = client
        .post(&format!("{}/rules", base_url))
        .json(&json!({ "rule": invalid_rule }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Rule ID cannot be empty"));

    // Test getting non-existent rule
    let response = client
        .get(&format!("{}/rules/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
