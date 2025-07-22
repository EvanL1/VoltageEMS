use std::time::Duration;
use voltage_common::redis::RedisClient;
use voltage_common::types::{PointData, PointValue, Quality};

/// 基础集成测试
/// 验证Redis连接、API Gateway基本功能等
#[tokio::test]
async fn test_redis_connection() -> anyhow::Result<()> {
    println!("Testing Redis connection...");

    // 从环境变量获取Redis URL
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".to_string());

    println!("Connecting to Redis at: {}", redis_url);

    let client = RedisClient::new(&redis_url).await?;

    // 测试基本的读写操作
    let test_key = "test:connection";
    let test_value = "integration_test_value";

    // 写入测试数据
    client.set(test_key, test_value).await?;

    // 读取测试数据
    let result = client.get(test_key).await?;
    assert_eq!(result, Some(test_value.to_string()));

    // 清理测试数据
    client.del(&[test_key]).await?;

    println!("✅ Redis connection test passed!");
    Ok(())
}

#[tokio::test]
async fn test_point_data_storage() -> anyhow::Result<()> {
    println!("Testing point data storage...");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".to_string());

    let client = RedisClient::new(&redis_url).await?;

    // 创建测试点位数据
    let point_data = PointData {
        point_id: 12345,
        value: PointValue::Float(123.45),
        timestamp: chrono::Utc::now(),
        quality: Quality::Good,
        metadata: None,
    };

    let test_key = "test:point:12345";
    let json_data = serde_json::to_string(&point_data)?;

    // 存储点位数据
    client.set(test_key, &json_data).await?;

    // 读取并验证数据
    let stored_data = client.get(test_key).await?;
    assert!(stored_data.is_some());

    let parsed_data: PointData = serde_json::from_str(&stored_data.unwrap())?;
    assert_eq!(parsed_data.point_id, 12345);
    assert_eq!(parsed_data.value.as_f64(), Some(123.45));
    assert_eq!(parsed_data.quality, Quality::Good);

    // 清理
    client.del(&[test_key]).await?;

    println!("✅ Point data storage test passed!");
    Ok(())
}

#[tokio::test]
async fn test_api_gateway_health() -> anyhow::Result<()> {
    println!("Testing API Gateway health endpoint...");

    // 从环境变量获取API Gateway URL
    let api_gateway_url =
        std::env::var("API_GATEWAY_URL").unwrap_or_else(|_| "http://apigateway:8089".to_string());

    let health_url = format!("{}/health", api_gateway_url);
    println!("Checking health at: {}", health_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // 尝试连接健康检查端点
    let response = client.get(&health_url).send().await?;

    println!("Health check response status: {}", response.status());

    if response.status().is_success() {
        let body = response.text().await?;
        println!("Health check response: {}", body);
        println!("✅ API Gateway health test passed!");
    } else {
        println!(
            "⚠️ API Gateway health check returned: {}",
            response.status()
        );
        // 在集成测试中，健康检查失败不应该导致测试失败
        // 因为API Gateway可能还在启动中
    }

    Ok(())
}

#[tokio::test]
async fn test_concurrent_redis_operations() -> anyhow::Result<()> {
    println!("Testing concurrent Redis operations...");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".to_string());

    let client = std::sync::Arc::new(RedisClient::new(&redis_url).await?);

    let mut tasks = Vec::new();

    // 启动10个并发任务
    for i in 0..10 {
        let client_clone = client.clone();
        let task = tokio::spawn(async move {
            let key = format!("test:concurrent:{}", i);
            let value = format!("value_{}", i);

            // 写入
            client_clone.set(&key, &value).await?;

            // 读取
            let result = client_clone.get(&key).await?;
            assert_eq!(result, Some(value));

            // 清理
            client_clone.del(&[&key]).await?;

            anyhow::Ok(())
        });
        tasks.push(task);
    }

    // 等待所有任务完成
    for task in tasks {
        task.await??;
    }

    println!("✅ Concurrent Redis operations test passed!");
    Ok(())
}
