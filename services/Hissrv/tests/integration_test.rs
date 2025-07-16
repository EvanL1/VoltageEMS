use hissrv::config::Config;
use hissrv::error::Result;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use voltage_common::redis::RedisClient;
use voltage_common::types::{PointData, PointValue};

/// 测试配置
fn get_test_config() -> Config {
    let mut config = Config::default();
    
    // Redis 配置
    config.redis.connection.host = "127.0.0.1".to_string();
    config.redis.connection.port = 6379;
    config.redis.subscription.channels = vec![
        "*:m:*".to_string(),
        "*:s:*".to_string(),
        "event:*".to_string(),
    ];
    
    // InfluxDB 配置
    config.storage.backends.influxdb.enabled = true;
    config.storage.backends.influxdb.url = "http://localhost:8086".to_string();
    config.storage.backends.influxdb.database = "hissrv_test".to_string();
    config.storage.backends.influxdb.batch_size = 10;
    config.storage.backends.influxdb.flush_interval = 1;
    
    // API 配置
    config.api.enabled = true;
    config.service.port = 8089;
    
    config
}

#[tokio::test]
async fn test_redis_subscription() -> Result<()> {
    let config = get_test_config();
    
    // 创建 Redis 客户端
    let redis_client = RedisClient::new(&format!(
        "redis://{}:{}",
        config.redis.connection.host,
        config.redis.connection.port
    ))
    .await?;
    
    // 创建订阅器
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut subscriber = hissrv::redis_subscriber::EnhancedRedisSubscriber::new(
        config.redis.clone(),
        hissrv::redis_subscriber::SubscriberConfig::default(),
        tx,
    );
    
    // 连接并订阅
    subscriber.connect().await?;
    subscriber.subscribe_channels(vec!["test:m:*".to_string()]).await?;
    
    // 启动监听（在后台）
    let subscriber_handle = tokio::spawn(async move {
        subscriber.start_listening().await
    });
    
    // 等待订阅建立
    sleep(Duration::from_millis(100)).await;
    
    // 发布测试数据
    let test_data = PointData {
        id: 10001,
        value: PointValue::Float(123.45),
        quality: 192,
        timestamp: chrono::Utc::now(),
        source: Some("test".to_string()),
    };
    
    // 发布到 Redis
    let mut conn = redis_client.get_connection().await?;
    conn.publish::<_, _, ()>(
        "test:m:10001",
        serde_json::to_string(&test_data).unwrap(),
    )
    .await?;
    
    // 接收消息
    let received = timeout(Duration::from_secs(1), rx.recv()).await;
    assert!(received.is_ok(), "应该接收到消息");
    
    let msg = received.unwrap().unwrap();
    assert_eq!(msg.channel, "test:m:10001");
    assert!(msg.point_data.is_some());
    assert_eq!(msg.point_data.unwrap().id, 10001);
    
    // 清理
    subscriber_handle.abort();
    
    Ok(())
}

#[tokio::test]
async fn test_batch_writing() -> Result<()> {
    use hissrv::batch_writer::{BatchWriteBuffer, BatchWriterConfig};
    use hissrv::storage::influxdb_storage::{InfluxDBBatchWriter, InfluxDBStorage};
    use hissrv::storage::{DataPoint, DataValue};
    
    let config = get_test_config();
    
    // 创建 InfluxDB 存储
    let influxdb_storage = Arc::new(tokio::sync::Mutex::new(
        InfluxDBStorage::new(config.storage.backends.influxdb.clone())
    ));
    
    // 创建批量写入器
    let batch_config = BatchWriterConfig {
        max_batch_size: 5,
        flush_interval_secs: 1,
        max_retries: 2,
        retry_delay_ms: 100,
        enable_wal: false,
        wal_path: "/tmp/test_wal".to_string(),
    };
    
    let batch_writer = InfluxDBBatchWriter::new(influxdb_storage.clone());
    let batch_buffer = Arc::new(BatchWriteBuffer::new(batch_writer, batch_config)?);
    
    // 启动刷新任务
    let buffer_clone = batch_buffer.clone();
    let flush_handle = tokio::spawn(async move {
        buffer_clone.start_flush_task().await;
    });
    
    // 添加测试数据
    for i in 1..=10 {
        let data_point = DataPoint {
            key: format!("test:m:{}", 20000 + i),
            timestamp: chrono::Utc::now(),
            value: DataValue::Float(100.0 + i as f64),
            tags: HashMap::from([
                ("test".to_string(), "true".to_string()),
                ("batch_id".to_string(), "test_batch".to_string()),
            ]),
            metadata: HashMap::new(),
        };
        
        batch_buffer.add(data_point).await?;
    }
    
    // 等待刷新
    sleep(Duration::from_secs(2)).await;
    
    // 验证统计信息
    let stats = batch_buffer.get_stats().await;
    assert_eq!(stats.total_items_processed, 10);
    assert_eq!(stats.total_batches_written, 2); // 5个一批，共2批
    
    // 清理
    flush_handle.abort();
    batch_buffer.shutdown().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_data_retention_policy() -> Result<()> {
    use hissrv::retention_policy::{RetentionPolicy, RetentionPolicyManager, RetentionRule};
    use hissrv::storage::redis_storage::RedisStorage;
    use hissrv::storage::{DataPoint, DataValue, Storage};
    
    let config = get_test_config();
    
    // 创建 Redis 存储
    let mut redis_storage = RedisStorage::new(config.redis.connection.clone());
    redis_storage.connect().await?;
    
    // 创建保留策略
    let mut policy_manager = RetentionPolicyManager::new();
    
    // 添加测试策略：保留1秒的数据
    let test_policy = RetentionPolicy {
        name: "test_policy".to_string(),
        description: "测试策略".to_string(),
        rules: vec![
            RetentionRule {
                pattern: "test:retention:*".to_string(),
                retention_days: 0,    // 0天
                retention_hours: 0,   // 0小时
                retention_minutes: 0, // 0分钟
                retention_seconds: 1, // 1秒
                enabled: true,
            },
        ],
        enabled: true,
        run_interval_secs: 1,
    };
    
    policy_manager.add_policy(test_policy);
    
    // 写入测试数据
    let old_data = DataPoint {
        key: "test:retention:old".to_string(),
        timestamp: chrono::Utc::now() - chrono::Duration::seconds(2), // 2秒前
        value: DataValue::Float(100.0),
        tags: HashMap::new(),
        metadata: HashMap::new(),
    };
    
    let new_data = DataPoint {
        key: "test:retention:new".to_string(),
        timestamp: chrono::Utc::now(), // 现在
        value: DataValue::Float(200.0),
        tags: HashMap::new(),
        metadata: HashMap::new(),
    };
    
    redis_storage.store(&old_data).await?;
    redis_storage.store(&new_data).await?;
    
    // 应用保留策略
    let redis_storage_arc = Arc::new(RwLock::new(Box::new(redis_storage) as Box<dyn Storage>));
    policy_manager.apply_all_policies(redis_storage_arc.clone()).await?;
    
    // 验证：旧数据应该被删除，新数据应该保留
    let redis_storage = redis_storage_arc.read().await;
    
    // 查询旧数据（应该不存在）
    let old_result = redis_storage
        .query("test:retention:old", None, None, None)
        .await?;
    assert!(old_result.is_empty(), "旧数据应该被删除");
    
    // 查询新数据（应该存在）
    let new_result = redis_storage
        .query("test:retention:new", None, None, None)
        .await?;
    assert!(!new_result.is_empty(), "新数据应该保留");
    
    Ok(())
}

#[tokio::test]
async fn test_api_endpoints() -> Result<()> {
    use reqwest;
    use serde_json::json;
    
    // 等待服务启动（如果需要的话）
    // 这个测试假设 HisSrv 已经在运行
    
    let base_url = "http://localhost:8089";
    let client = reqwest::Client::new();
    
    // 测试健康检查
    let health_resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await?;
    assert_eq!(health_resp.status(), 200);
    
    // 测试获取最新数据
    let latest_resp = client
        .get(format!("{}/api/v1/data/latest", base_url))
        .query(&[("channels", "1001,1002")])
        .send()
        .await?;
    assert_eq!(latest_resp.status(), 200);
    
    // 测试聚合查询
    let aggregate_body = json!({
        "channel_id": 1001,
        "point_ids": ["10001", "10002"],
        "start_time": chrono::Utc::now().timestamp() - 3600,
        "end_time": chrono::Utc::now().timestamp(),
        "aggregation": "mean",
        "interval": "5m"
    });
    
    let aggregate_resp = client
        .post(format!("{}/api/v1/data/aggregate", base_url))
        .json(&aggregate_body)
        .send()
        .await?;
    assert_eq!(aggregate_resp.status(), 200);
    
    Ok(())
}

#[tokio::test]
async fn test_message_flow_integration() -> Result<()> {
    use hissrv::enhanced_message_processor::EnhancedMessageProcessor;
    use hissrv::monitoring::MetricsCollector;
    use hissrv::storage::StorageManager;
    
    let config = get_test_config();
    
    // 创建存储管理器
    let mut storage_manager = StorageManager::new();
    
    // 添加存储后端
    let influxdb_storage = hissrv::storage::influxdb_storage::InfluxDBStorage::new(
        config.storage.backends.influxdb.clone()
    );
    storage_manager.add_backend("influxdb".to_string(), Box::new(influxdb_storage));
    
    let redis_storage = hissrv::storage::redis_storage::RedisStorage::new(
        config.redis.connection.clone()
    );
    storage_manager.add_backend("redis".to_string(), Box::new(redis_storage));
    
    storage_manager.set_default_backend("influxdb".to_string());
    storage_manager.connect_all().await?;
    
    let storage_manager = Arc::new(RwLock::new(storage_manager));
    
    // 创建消息通道
    let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();
    
    // 创建消息处理器
    let metrics = MetricsCollector::new();
    let mut processor = EnhancedMessageProcessor::new(
        storage_manager.clone(),
        msg_rx,
        metrics,
        10,   // batch_size
        100,  // batch_timeout_ms
    );
    
    // 启动处理器
    let processor_handle = tokio::spawn(async move {
        processor.start_processing().await
    });
    
    // 发送测试消息
    use hissrv::redis_subscriber::{ChannelInfo, MessageType, SubscriptionMessage};
    
    for i in 1..=5 {
        let msg = SubscriptionMessage {
            id: format!("test-msg-{}", i),
            channel: format!("1001:m:{}", 30000 + i),
            channel_info: Some(ChannelInfo {
                channel_id: 1001,
                message_type: MessageType::Telemetry,
                point_id: 30000 + i,
            }),
            timestamp: chrono::Utc::now(),
            point_data: Some(PointData {
                id: 30000 + i,
                value: PointValue::Float(50.0 + i as f64),
                quality: 192,
                timestamp: chrono::Utc::now(),
                source: Some("integration_test".to_string()),
            }),
            raw_data: None,
            metadata: HashMap::new(),
        };
        
        msg_tx.send(msg)?;
    }
    
    // 等待处理
    sleep(Duration::from_secs(2)).await;
    
    // 关闭通道并等待处理器完成
    drop(msg_tx);
    let _ = timeout(Duration::from_secs(5), processor_handle).await;
    
    // 验证数据已写入存储
    let storage_manager = storage_manager.read().await;
    if let Some(influxdb) = storage_manager.get_backend(Some("influxdb")) {
        // 查询验证
        let results = influxdb
            .query("1001:m:30001", None, None, Some(10))
            .await?;
        assert!(!results.is_empty(), "应该能查询到写入的数据");
    }
    
    Ok(())
}

/// 测试扁平化存储格式
#[tokio::test]
async fn test_flat_storage_format() -> Result<()> {
    let redis_client = RedisClient::new("redis://localhost:6379").await?;
    let mut conn = redis_client.get_connection().await?;
    
    // 测试数据
    let test_cases = vec![
        ("1001:m:10001", json!({"value": 220.5, "quality": 192, "timestamp": 1234567890})),
        ("1001:s:20001", json!({"value": true, "quality": 192, "timestamp": 1234567890})),
        ("1001:c:30001", json!({"value": 1, "quality": 192, "timestamp": 1234567890})),
        ("1001:a:40001", json!({"value": 75.5, "quality": 192, "timestamp": 1234567890})),
    ];
    
    // 写入测试数据
    for (key, value) in &test_cases {
        conn.set::<_, _, ()>(key, value.to_string()).await?;
    }
    
    // 验证数据
    for (key, expected_value) in &test_cases {
        let result: String = conn.get(key).await?;
        let parsed: serde_json::Value = serde_json::from_str(&result)?;
        assert_eq!(&parsed, expected_value, "键 {} 的值不匹配", key);
    }
    
    // 清理
    for (key, _) in &test_cases {
        conn.del::<_, ()>(key).await?;
    }
    
    Ok(())
}