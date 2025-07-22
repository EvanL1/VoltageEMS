//! REST API 测试

use crate::api::{
    handlers::{QueryRequest, QueryResponse, WriteRequest},
    start_api_server,
};
use crate::config::Config;
use crate::storage::{DataPoint, DataValue, QueryOptions, Storage, StorageManager};
use crate::tests::mock_storage::create_memory_storage;
use crate::tests::test_utils::{create_test_config, create_test_data_point};
use axum::http::{header, StatusCode};
use chrono::{Duration, Utc};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// 创建测试服务器
async fn create_test_server() -> (SocketAddr, Arc<RwLock<StorageManager>>) {
    let mut config = create_test_config();

    // 使用随机端口
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    config.service.port = addr.port();

    // 创建存储管理器
    let mut storage_manager = StorageManager::new();
    let mock_storage = create_memory_storage();
    storage_manager.add_backend("memory".to_string(), Box::new(mock_storage));
    storage_manager.set_default_backend("memory".to_string());
    storage_manager.connect_all().await.unwrap();

    let storage_manager_arc = Arc::new(RwLock::new(storage_manager));
    let storage_manager_clone = storage_manager_arc.clone();

    // 启动服务器
    tokio::spawn(async move {
        start_api_server(config, storage_manager_clone).await.ok();
    });

    // 等待服务器启动
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (addr, storage_manager_arc)
}

#[tokio::test]
async fn test_api_health_check() {
    let (addr, _) = create_test_server().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/api/v1/health", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert!(body["version"].is_string());
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn test_api_write_single_point() {
    let (addr, storage_manager) = create_test_server().await;

    let client = reqwest::Client::new();

    // 写入单个数据点
    let write_request = WriteRequest {
        key: "test_metric".to_string(),
        value: 42.5,
        timestamp: Some(Utc::now()),
        tags: Some(json!({
            "sensor": "temp01",
            "location": "room1"
        })),
    };

    let response = client
        .post(format!("http://{}/api/v1/write", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&write_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["points_written"], 1);

    // 验证数据被写入
    let storage = storage_manager.read().await;
    let result = storage
        .query(
            "test_metric",
            QueryOptions {
                start_time: Utc::now() - Duration::hours(1),
                end_time: Utc::now(),
                limit: None,
                aggregate: None,
                group_by: None,
                fill: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].points.len(), 1);
    assert!(matches!(
        result[0].points[0].value,
        DataValue::Float(v) if v == 42.5
    ));
}

#[tokio::test]
async fn test_api_write_batch() {
    let (addr, storage_manager) = create_test_server().await;

    let client = reqwest::Client::new();

    // 批量写入数据点
    let batch_request = json!({
        "points": [
            {
                "key": "cpu_usage",
                "value": 75.5,
                "tags": {"host": "server1"}
            },
            {
                "key": "memory_usage",
                "value": 60.2,
                "tags": {"host": "server1"}
            },
            {
                "key": "disk_usage",
                "value": 45.8,
                "tags": {"host": "server1"}
            }
        ]
    });

    let response = client
        .post(format!("http://{}/api/v1/write/batch", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&batch_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["points_written"], 3);
}

#[tokio::test]
async fn test_api_query_single_key() {
    let (addr, storage_manager) = create_test_server().await;

    // 先写入测试数据
    let mut storage = storage_manager.write().await;
    let now = Utc::now();
    for i in 0..10 {
        let point = DataPoint {
            key: "temperature".to_string(),
            value: DataValue::Float(20.0 + i as f64),
            timestamp: now - Duration::minutes(10 - i),
            tags: Default::default(),
            metadata: Default::default(),
        };
        storage.write(point).await.unwrap();
    }
    drop(storage);

    let client = reqwest::Client::new();

    // 查询数据
    let query_request = QueryRequest {
        key: "temperature".to_string(),
        start_time: now - Duration::hours(1),
        end_time: now,
        limit: Some(5),
        aggregate: None,
        group_by: None,
        fill: None,
    };

    let response = client
        .post(format!("http://{}/api/v1/query", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&query_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: QueryResponse = response.json().await.unwrap();
    assert_eq!(body.results.len(), 1);
    assert_eq!(body.results[0].key, "temperature");
    assert_eq!(body.results[0].points.len(), 5); // 限制为5个点
    assert!(body.metadata.query_time_ms > 0.0);
}

#[tokio::test]
async fn test_api_query_with_aggregation() {
    let (addr, storage_manager) = create_test_server().await;

    // 写入测试数据
    let mut storage = storage_manager.write().await;
    let now = Utc::now();
    for i in 0..100 {
        let point = DataPoint {
            key: "cpu_usage".to_string(),
            value: DataValue::Float(50.0 + (i as f64).sin() * 20.0),
            timestamp: now - Duration::seconds(100 - i),
            tags: Default::default(),
            metadata: Default::default(),
        };
        storage.write(point).await.unwrap();
    }
    drop(storage);

    let client = reqwest::Client::new();

    // 查询平均值
    let query_request = json!({
        "key": "cpu_usage",
        "start_time": (now - Duration::minutes(5)).to_rfc3339(),
        "end_time": now.to_rfc3339(),
        "aggregate": "mean"
    });

    let response = client
        .post(format!("http://{}/api/v1/query", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&query_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["results"][0]["points"].as_array().unwrap().len(), 1);

    let avg_value = body["results"][0]["points"][0]["value"].as_f64().unwrap();
    assert!(avg_value > 40.0 && avg_value < 60.0);
}

#[tokio::test]
async fn test_api_query_batch() {
    let (addr, storage_manager) = create_test_server().await;

    // 写入多个指标的数据
    let mut storage = storage_manager.write().await;
    let now = Utc::now();
    let metrics = vec!["metric1", "metric2", "metric3"];

    for metric in &metrics {
        for i in 0..5 {
            let point = DataPoint {
                key: metric.to_string(),
                value: DataValue::Float(i as f64),
                timestamp: now - Duration::minutes(5 - i),
                tags: Default::default(),
                metadata: Default::default(),
            };
            storage.write(point).await.unwrap();
        }
    }
    drop(storage);

    let client = reqwest::Client::new();

    // 批量查询
    let batch_query = json!({
        "queries": [
            {
                "key": "metric1",
                "start_time": (now - Duration::hours(1)).to_rfc3339(),
                "end_time": now.to_rfc3339()
            },
            {
                "key": "metric2",
                "start_time": (now - Duration::hours(1)).to_rfc3339(),
                "end_time": now.to_rfc3339()
            },
            {
                "key": "metric3",
                "start_time": (now - Duration::hours(1)).to_rfc3339(),
                "end_time": now.to_rfc3339()
            }
        ]
    });

    let response = client
        .post(format!("http://{}/api/v1/query/batch", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&batch_query)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result["key"], metrics[i]);
        assert_eq!(result["points"].as_array().unwrap().len(), 5);
    }
}

#[tokio::test]
async fn test_api_error_handling() {
    let (addr, _) = create_test_server().await;

    let client = reqwest::Client::new();

    // 测试无效的请求体
    let invalid_request = json!({
        "invalid_field": "test"
    });

    let response = client
        .post(format!("http://{}/api/v1/write", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&invalid_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 测试查询不存在的数据
    let query_request = json!({
        "key": "non_existent_metric",
        "start_time": (Utc::now() - Duration::hours(1)).to_rfc3339(),
        "end_time": Utc::now().to_rfc3339()
    });

    let response = client
        .post(format!("http://{}/api/v1/query", addr))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&query_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["results"][0]["points"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_metrics_endpoint() {
    let (addr, _) = create_test_server().await;

    let client = reqwest::Client::new();

    // 先进行一些操作以生成指标
    for i in 0..5 {
        let write_request = json!({
            "key": format!("test_metric_{}", i),
            "value": i as f64
        });

        client
            .post(format!("http://{}/api/v1/write", addr))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&write_request)
            .send()
            .await
            .unwrap();
    }

    // 获取指标
    let response = client
        .get(format!("http://{}/api/v1/metrics", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.unwrap();

    // 验证Prometheus格式的指标
    assert!(body.contains("# HELP"));
    assert!(body.contains("# TYPE"));
    assert!(body.contains("hissrv_api_requests_total"));
}

#[tokio::test]
async fn test_api_rate_limiting() {
    let (addr, _) = create_test_server().await;

    let client = reqwest::Client::new();

    // 注意：由于测试配置中rate_limit未启用，这里只测试基本功能
    // 在实际环境中，应该测试速率限制功能

    let mut handles = vec![];

    // 并发发送多个请求
    for i in 0..10 {
        let client = client.clone();
        let addr = addr.clone();

        let handle = tokio::spawn(async move {
            let response = client
                .get(format!("http://{}/api/v1/health", addr))
                .send()
                .await
                .unwrap();

            response.status()
        });

        handles.push(handle);
    }

    // 收集结果
    let mut success_count = 0;
    for handle in handles {
        let status = handle.await.unwrap();
        if status == StatusCode::OK {
            success_count += 1;
        }
    }

    // 在没有速率限制的情况下，所有请求都应该成功
    assert_eq!(success_count, 10);
}

#[tokio::test]
async fn test_api_swagger_ui() {
    let (addr, _) = create_test_server().await;

    let client = reqwest::Client::new();

    // 访问Swagger UI
    let response = client
        .get(format!("http://{}/api/v1/swagger-ui/", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.unwrap();
    assert!(body.contains("swagger"));
    assert!(body.contains("openapi"));
}
