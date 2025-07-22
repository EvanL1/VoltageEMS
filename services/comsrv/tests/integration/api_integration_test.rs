//! API集成测试
//!
//! 测试comsrv的REST API接口
//! 包括：
//! 1. 健康检查端点
//! 2. 通道管理API
//! 3. 点位数据查询API
//! 4. OpenAPI文档验证

use std::collections::HashMap;
use std::time::Duration;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// API响应基础结构
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: i64,
}

/// 健康检查响应
#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
    redis_connected: bool,
    active_channels: u32,
}

/// 通道信息
#[derive(Debug, Serialize, Deserialize)]
struct ChannelInfo {
    id: u16,
    name: String,
    protocol: String,
    enabled: bool,
    status: String,
    points_count: u32,
    last_update: Option<i64>,
}

/// 点位数据
#[derive(Debug, Serialize, Deserialize)]
struct PointValue {
    channel_id: u16,
    point_id: u32,
    value: f64,
    quality: u8,
    timestamp: i64,
}

/// 测试配置
struct TestConfig {
    base_url: String,
    timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("TEST_COMSRV_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            timeout: Duration::from_secs(30),
        }
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_health_check_endpoint() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing health check endpoint");
    
    // 等待服务启动
    wait_for_service(&client, &config).await;
    
    // 测试健康检查端点
    let response = client
        .get(format!("{}/api/health", config.base_url))
        .timeout(config.timeout)
        .send()
        .await
        .expect("Failed to send health check request");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let health: HealthResponse = response.json().await
        .expect("Failed to parse health response");
    
    assert_eq!(health.status, "healthy");
    assert!(health.redis_connected, "Redis should be connected");
    assert!(health.uptime_seconds > 0, "Uptime should be greater than 0");
    
    info!("Health check passed: {:?}", health);
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_channel_management_api() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing channel management API");
    
    wait_for_service(&client, &config).await;
    
    // 获取所有通道
    let response = client
        .get(format!("{}/api/channels", config.base_url))
        .send()
        .await
        .expect("Failed to get channels");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let api_response: ApiResponse<Vec<ChannelInfo>> = response.json().await
        .expect("Failed to parse channels response");
    
    assert!(api_response.success);
    let channels = api_response.data.expect("No channel data");
    
    info!("Found {} channels", channels.len());
    
    // 获取单个通道详情
    if let Some(first_channel) = channels.first() {
        let response = client
            .get(format!("{}/api/channels/{}", config.base_url, first_channel.id))
            .send()
            .await
            .expect("Failed to get channel details");
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let api_response: ApiResponse<ChannelInfo> = response.json().await
            .expect("Failed to parse channel details");
        
        assert!(api_response.success);
        let channel = api_response.data.expect("No channel details");
        
        assert_eq!(channel.id, first_channel.id);
        info!("Channel {} details: {:?}", channel.id, channel);
        
        // 测试启用/禁用通道
        let response = client
            .post(format!("{}/api/channels/{}/disable", config.base_url, channel.id))
            .send()
            .await
            .expect("Failed to disable channel");
        
        assert_eq!(response.status(), StatusCode::OK);
        
        // 重新启用
        let response = client
            .post(format!("{}/api/channels/{}/enable", config.base_url, channel.id))
            .send()
            .await
            .expect("Failed to enable channel");
        
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_point_data_query_api() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing point data query API");
    
    wait_for_service(&client, &config).await;
    
    // 首先获取可用的通道
    let channels = get_channels(&client, &config).await;
    
    if let Some(channel) = channels.first() {
        // 查询通道的所有点位值
        let response = client
            .get(format!("{}/api/channels/{}/points", config.base_url, channel.id))
            .send()
            .await
            .expect("Failed to get channel points");
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let api_response: ApiResponse<Vec<PointValue>> = response.json().await
            .expect("Failed to parse points response");
        
        assert!(api_response.success);
        
        if let Some(points) = api_response.data {
            info!("Channel {} has {} points", channel.id, points.len());
            
            // 查询单个点位的值
            if let Some(first_point) = points.first() {
                let response = client
                    .get(format!(
                        "{}/api/channels/{}/points/{}", 
                        config.base_url, 
                        channel.id, 
                        first_point.point_id
                    ))
                    .send()
                    .await
                    .expect("Failed to get point value");
                
                assert_eq!(response.status(), StatusCode::OK);
                
                let api_response: ApiResponse<PointValue> = response.json().await
                    .expect("Failed to parse point value");
                
                assert!(api_response.success);
                let point = api_response.data.expect("No point data");
                
                assert_eq!(point.point_id, first_point.point_id);
                info!("Point {} value: {}", point.point_id, point.value);
            }
        }
        
        // 测试批量查询
        let point_ids = vec![10001, 10002, 20001];
        let response = client
            .post(format!("{}/api/channels/{}/points/batch", config.base_url, channel.id))
            .json(&json!({ "point_ids": point_ids }))
            .send()
            .await
            .expect("Failed to batch query points");
        
        if response.status() == StatusCode::OK {
            let api_response: ApiResponse<Vec<PointValue>> = response.json().await
                .expect("Failed to parse batch query response");
            
            assert!(api_response.success);
            if let Some(points) = api_response.data {
                info!("Batch query returned {} points", points.len());
            }
        }
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_control_command_api() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing control command API");
    
    wait_for_service(&client, &config).await;
    
    // 获取第一个可用通道
    let channels = get_channels(&client, &config).await;
    
    if let Some(channel) = channels.first() {
        // 发送控制命令
        let control_request = json!({
            "point_id": 30001,
            "value": 1.0,
            "command_type": "control"
        });
        
        let response = client
            .post(format!("{}/api/channels/{}/control", config.base_url, channel.id))
            .json(&control_request)
            .send()
            .await
            .expect("Failed to send control command");
        
        // 控制命令可能返回404如果点位不存在，这是正常的
        if response.status() == StatusCode::OK {
            let api_response: ApiResponse<String> = response.json().await
                .expect("Failed to parse control response");
            
            assert!(api_response.success);
            info!("Control command sent successfully");
        } else if response.status() == StatusCode::NOT_FOUND {
            info!("Control point not found (expected in test environment)");
        } else {
            panic!("Unexpected status code: {}", response.status());
        }
        
        // 发送调节命令
        let adjustment_request = json!({
            "point_id": 40001,
            "value": 75.5,
            "command_type": "adjustment"
        });
        
        let response = client
            .post(format!("{}/api/channels/{}/control", config.base_url, channel.id))
            .json(&adjustment_request)
            .send()
            .await
            .expect("Failed to send adjustment command");
        
        if response.status() == StatusCode::OK {
            info!("Adjustment command sent successfully");
        }
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_openapi_documentation() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing OpenAPI documentation");
    
    wait_for_service(&client, &config).await;
    
    // 获取OpenAPI规范
    let response = client
        .get(format!("{}/api/openapi.json", config.base_url))
        .send()
        .await
        .expect("Failed to get OpenAPI spec");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let openapi: serde_json::Value = response.json().await
        .expect("Failed to parse OpenAPI spec");
    
    // 验证基本结构
    assert_eq!(openapi["openapi"], "3.0.0");
    assert!(openapi["info"]["title"].is_string());
    assert!(openapi["info"]["version"].is_string());
    assert!(openapi["paths"].is_object());
    
    // 验证必要的端点存在
    let paths = openapi["paths"].as_object().expect("Paths should be object");
    
    assert!(paths.contains_key("/api/health"), "Health endpoint missing");
    assert!(paths.contains_key("/api/channels"), "Channels endpoint missing");
    assert!(paths.contains_key("/api/channels/{channel_id}"), "Channel detail endpoint missing");
    
    info!("OpenAPI documentation is valid");
    
    // 测试Swagger UI（如果启用）
    let response = client
        .get(format!("{}/swagger-ui/", config.base_url))
        .send()
        .await;
    
    if let Ok(response) = response {
        if response.status() == StatusCode::OK {
            info!("Swagger UI is available");
        }
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_api_error_handling() {
    init_logging();
    let config = TestConfig::default();
    let client = create_http_client();
    
    info!("Testing API error handling");
    
    wait_for_service(&client, &config).await;
    
    // 测试不存在的端点
    let response = client
        .get(format!("{}/api/nonexistent", config.base_url))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // 测试无效的通道ID
    let response = client
        .get(format!("{}/api/channels/99999", config.base_url))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    let api_response: ApiResponse<()> = response.json().await
        .expect("Failed to parse error response");
    
    assert!(!api_response.success);
    assert!(api_response.error.is_some());
    
    // 测试无效的请求体
    let response = client
        .post(format!("{}/api/channels/1/control", config.base_url))
        .json(&json!({ "invalid": "data" }))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    info!("Error handling tests passed");
}

// 辅助函数

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("comsrv=debug,api_integration_test=info")
        .try_init();
}

fn create_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
}

async fn wait_for_service(client: &Client, config: &TestConfig) {
    let max_retries = 30;
    let mut retries = 0;
    
    info!("Waiting for service to be ready...");
    
    while retries < max_retries {
        match client
            .get(format!("{}/api/health", config.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) if response.status() == StatusCode::OK => {
                info!("Service is ready");
                return;
            }
            Ok(response) => {
                warn!("Service returned status: {}", response.status());
            }
            Err(e) => {
                warn!("Service not ready: {}", e);
            }
        }
        
        retries += 1;
        sleep(Duration::from_secs(1)).await;
    }
    
    panic!("Service did not become ready after {} seconds", max_retries);
}

async fn get_channels(client: &Client, config: &TestConfig) -> Vec<ChannelInfo> {
    let response = client
        .get(format!("{}/api/channels", config.base_url))
        .send()
        .await
        .expect("Failed to get channels");
    
    let api_response: ApiResponse<Vec<ChannelInfo>> = response.json().await
        .expect("Failed to parse channels response");
    
    api_response.data.unwrap_or_default()
}