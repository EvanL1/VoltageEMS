//! API integration tests

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::util::ServiceExt;

mod common;
use common::{cleanup_test_data, create_test_router};

/// Helper to make JSON requests
async fn json_request(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = if let Some(json) = body {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&json).unwrap()))
            .unwrap()
    } else {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap()
    };

    let response = app.clone().oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let body: Value = if body_bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap()
    };

    (status, body)
}

#[tokio::test]
async fn test_health_check() {
    let app = create_test_router().await.unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_status_endpoint() {
    let app = create_test_router().await.unwrap();

    let (status, body) = json_request(&app, "GET", "/api/v1/status", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["service"], "alarmsrv");
    assert_eq!(body["status"], "running");
    assert!(body["redis_connected"].as_bool().unwrap());
}

#[tokio::test]
async fn test_create_alarm() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    let create_request = json!({
        "title": "Test Alarm",
        "description": "This is a test alarm",
        "level": "Warning"
    });

    let (status, body) = json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "Test Alarm");
    assert_eq!(body["description"], "This is a test alarm");
    assert_eq!(body["level"], "Warning");
    assert_eq!(body["status"], "New");
    assert!(body["id"].is_string());
    assert!(body["classification"]["category"].is_string());

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_list_alarms() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create some test alarms
    for i in 0..3 {
        let create_request = json!({
            "title": format!("Test Alarm {}", i),
            "description": "Test description",
            "level": "Warning"
        });
        json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;
    }

    // List all alarms
    let (status, body) = json_request(&app, "GET", "/api/v1/alarms", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);
    assert_eq!(body["alarms"].as_array().unwrap().len(), 3);

    // Test with limit
    let (status, body) = json_request(&app, "GET", "/api/v1/alarms?limit=2", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 3);
    assert_eq!(body["alarms"].as_array().unwrap().len(), 2);
    assert_eq!(body["limit"], 2);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_acknowledge_alarm() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create an alarm
    let create_request = json!({
        "title": "Test Alarm to Acknowledge",
        "description": "This alarm will be acknowledged",
        "level": "Major"
    });

    let (_, created) = json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;
    let alarm_id = created["id"].as_str().unwrap();

    // Acknowledge the alarm
    let (status, body) = json_request(
        &app,
        "POST",
        &format!("/api/v1/alarms/{}/ack", alarm_id),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "Acknowledged");
    assert!(body["acknowledged_at"].is_string());
    assert_eq!(body["acknowledged_by"], "system");

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_resolve_alarm() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create an alarm
    let create_request = json!({
        "title": "Test Alarm to Resolve",
        "description": "This alarm will be resolved",
        "level": "Minor"
    });

    let (_, created) = json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;
    let alarm_id = created["id"].as_str().unwrap();

    // Resolve the alarm
    let (status, body) = json_request(
        &app,
        "POST",
        &format!("/api/v1/alarms/{}/resolve", alarm_id),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "Resolved");
    assert!(body["resolved_at"].is_string());
    assert_eq!(body["resolved_by"], "system");

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_get_statistics() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create alarms with different levels
    let levels = ["Critical", "Major", "Minor", "Warning", "Info"];
    for level in &levels {
        let create_request = json!({
            "title": format!("{} Alarm", level),
            "description": "Test alarm",
            "level": level
        });
        json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;
    }

    // Get statistics
    let (status, body) = json_request(&app, "GET", "/api/v1/stats", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 5);
    assert_eq!(body["by_status"]["new"], 5);
    assert_eq!(body["by_level"]["critical"], 1);
    assert_eq!(body["by_level"]["major"], 1);
    assert_eq!(body["by_level"]["minor"], 1);
    assert_eq!(body["by_level"]["warning"], 1);
    assert_eq!(body["by_level"]["info"], 1);

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_classify_alarms() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create an unclassified alarm (simulate)
    let create_request = json!({
        "title": "High Temperature Alert",
        "description": "Temperature is too high",
        "level": "Warning"
    });
    json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;

    // Trigger classification
    let (status, body) = json_request(&app, "POST", "/api/v1/alarms/classify", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["classified_count"].is_number());
    assert!(body["failed_count"].is_number());

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}

#[tokio::test]
async fn test_get_alarm_categories() {
    let app = create_test_router().await.unwrap();

    let (status, body) = json_request(&app, "GET", "/api/v1/alarms/categories", None).await;

    assert_eq!(status, StatusCode::OK);
    let categories = body.as_array().unwrap();
    assert!(!categories.is_empty());

    // Check that we have expected categories
    let category_names: Vec<&str> = categories
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();

    assert!(category_names.contains(&"environmental"));
    assert!(category_names.contains(&"power"));
    assert!(category_names.contains(&"communication"));
    assert!(category_names.contains(&"system"));
    assert!(category_names.contains(&"unclassified"));
}

#[tokio::test]
async fn test_alarm_filtering() {
    let app = create_test_router().await.unwrap();

    // Clean up before test
    cleanup_test_data("ems:alarms:*").await.unwrap();

    // Create alarms with different levels
    let test_data = [
        ("Critical Alert", "Critical"),
        ("Major Issue", "Major"),
        ("Minor Problem", "Minor"),
        ("Warning Notice", "Warning"),
        ("Info Message", "Info"),
    ];

    for (title, level) in &test_data {
        let create_request = json!({
            "title": title,
            "description": "Test alarm",
            "level": level
        });
        json_request(&app, "POST", "/api/v1/alarms", Some(create_request)).await;
    }

    // Filter by level
    let (status, body) = json_request(&app, "GET", "/api/v1/alarms?level=Critical", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["alarms"][0]["title"], "Critical Alert");

    // Filter by keyword
    let (status, body) = json_request(&app, "GET", "/api/v1/alarms?keyword=Issue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["alarms"][0]["title"], "Major Issue");

    // Clean up after test
    cleanup_test_data("ems:alarms:*").await.unwrap();
}
