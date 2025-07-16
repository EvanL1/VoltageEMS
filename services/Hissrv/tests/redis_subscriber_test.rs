use hissrv::config::{RedisConfig, RedisConnection, RedisSubscriptionConfig};
use hissrv::redis_subscriber::{
    ChannelInfo, EnhancedRedisSubscriber, MessageType, SubscriberConfig, SubscriberState,
    SubscriptionMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use voltage_common::redis::RedisClient;
use voltage_common::types::{PointData, PointValue};

/// 创建测试用的 Redis 配置
fn create_test_redis_config() -> RedisConfig {
    RedisConfig {
        connection: RedisConnection {
            host: "127.0.0.1".to_string(),
            port: 6379,
            password: String::new(),
            socket: String::new(),
            database: 0,
            pool_size: 5,
            timeout: 5,
            timeout_seconds: 5,
            max_retries: 3,
        },
        subscription: RedisSubscriptionConfig {
            channels: vec!["test:*".to_string()],
            key_patterns: vec![],
            channel_ids: vec![],
        },
    }
}

#[tokio::test]
async fn test_message_type_parsing() {
    // 测试新的扁平化格式
    assert!(matches!(
        MessageType::from_channel("1001:m:10001"),
        Some(MessageType::Telemetry)
    ));
    assert!(matches!(
        MessageType::from_channel("1001:s:20001"),
        Some(MessageType::Signal)
    ));
    assert!(matches!(
        MessageType::from_channel("1001:c:30001"),
        Some(MessageType::Control)
    ));
    assert!(matches!(
        MessageType::from_channel("1001:a:40001"),
        Some(MessageType::Adjustment)
    ));
    assert!(matches!(
        MessageType::from_channel("1001:calc:50001"),
        Some(MessageType::Calculated)
    ));
    
    // 测试事件和系统状态格式
    assert!(matches!(
        MessageType::from_channel("event:alarm"),
        Some(MessageType::Event)
    ));
    assert!(matches!(
        MessageType::from_channel("system:hissrv:status"),
        Some(MessageType::SystemStatus)
    ));
    
    // 测试无效格式
    assert!(MessageType::from_channel("invalid").is_none());
    assert!(MessageType::from_channel("1001:x:10001").is_none());
}

#[tokio::test]
async fn test_channel_info_parsing() {
    // 测试有效的通道信息
    let info = ChannelInfo::from_channel("1001:m:10001").unwrap();
    assert_eq!(info.channel_id, 1001);
    assert!(matches!(info.message_type, MessageType::Telemetry));
    assert_eq!(info.point_id, 10001);
    
    // 测试不同类型
    let signal_info = ChannelInfo::from_channel("2002:s:20002").unwrap();
    assert_eq!(signal_info.channel_id, 2002);
    assert!(matches!(signal_info.message_type, MessageType::Signal));
    assert_eq!(signal_info.point_id, 20002);
    
    // 测试无效格式
    assert!(ChannelInfo::from_channel("invalid").is_none());
    assert!(ChannelInfo::from_channel("1001:10001").is_none());
    assert!(ChannelInfo::from_channel("abc:m:10001").is_none());
    assert!(ChannelInfo::from_channel("1001:m:abc").is_none());
}

#[tokio::test]
async fn test_subscriber_state_transitions() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig::default();
    let (tx, _rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    // 初始状态应该是 Disconnected
    assert_eq!(subscriber.get_state().await, SubscriberState::Disconnected);
    
    // 连接后应该是 Connected
    if subscriber.connect().await.is_ok() {
        assert_eq!(subscriber.get_state().await, SubscriberState::Connected);
        
        // 订阅时应该是 Subscribing
        let _ = subscriber.subscribe_channels(vec!["test:*".to_string()]).await;
        
        // 断开连接
        subscriber.disconnect().await.unwrap();
        assert_eq!(subscriber.get_state().await, SubscriberState::Stopped);
    }
}

#[tokio::test]
async fn test_basic_subscription() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig {
        batch_size: 10,
        batch_timeout_ms: 100,
        ..Default::default()
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    // 连接并订阅
    if subscriber.connect().await.is_ok() {
        subscriber
            .subscribe_channels(vec!["test_basic:*".to_string()])
            .await
            .unwrap();
        
        // 启动监听（在后台）
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber.start_listening().await;
        });
        
        // 等待订阅建立
        sleep(Duration::from_millis(100)).await;
        
        // 创建 Redis 客户端发布消息
        let redis_client = RedisClient::new("redis://127.0.0.1:6379").await.unwrap();
        let mut conn = redis_client.get_connection().await.unwrap();
        
        // 发布测试消息
        use redis::AsyncCommands;
        let test_channel = "test_basic:m:10001";
        let test_data = serde_json::json!({
            "value": 123.45,
            "quality": 192,
            "timestamp": chrono::Utc::now().timestamp_millis()
        });
        
        conn.publish::<_, _, ()>(test_channel, test_data.to_string())
            .await
            .unwrap();
        
        // 接收消息
        let received = timeout(Duration::from_secs(1), rx.recv()).await;
        
        if let Ok(Some(msg)) = received {
            assert_eq!(msg.channel, test_channel);
            assert!(msg.raw_data.is_some());
        }
        
        // 清理
        subscriber_handle.abort();
    }
}

#[tokio::test]
async fn test_pattern_subscription() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig {
        enable_pattern_subscribe: true,
        ..Default::default()
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    if subscriber.connect().await.is_ok() {
        // 订阅模式
        subscriber
            .subscribe_channels(vec!["test_pattern:*:*".to_string()])
            .await
            .unwrap();
        
        // 启动监听
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber.start_listening().await;
        });
        
        sleep(Duration::from_millis(100)).await;
        
        // 发布到多个匹配模式的通道
        let redis_client = RedisClient::new("redis://127.0.0.1:6379").await.unwrap();
        let mut conn = redis_client.get_connection().await.unwrap();
        
        use redis::AsyncCommands;
        let channels = vec![
            "test_pattern:m:10001",
            "test_pattern:s:20001",
            "test_pattern:c:30001",
        ];
        
        for channel in &channels {
            conn.publish::<_, _, ()>(channel, "test_data")
                .await
                .unwrap();
        }
        
        // 接收多个消息
        let mut received_channels = vec![];
        for _ in 0..3 {
            if let Ok(Some(msg)) = timeout(Duration::from_secs(1), rx.recv()).await {
                received_channels.push(msg.channel);
            }
        }
        
        // 验证接收到所有消息
        assert_eq!(received_channels.len(), 3);
        
        subscriber_handle.abort();
    }
}

#[tokio::test]
async fn test_message_batching() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig {
        batch_size: 5,
        batch_timeout_ms: 200,
        ..Default::default()
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    if subscriber.connect().await.is_ok() {
        subscriber
            .subscribe_channels(vec!["test_batch:*".to_string()])
            .await
            .unwrap();
        
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber.start_listening().await;
        });
        
        sleep(Duration::from_millis(100)).await;
        
        // 快速发布多条消息
        let redis_client = RedisClient::new("redis://127.0.0.1:6379").await.unwrap();
        let mut conn = redis_client.get_connection().await.unwrap();
        
        use redis::AsyncCommands;
        for i in 1..=10 {
            let channel = format!("test_batch:m:{}", 10000 + i);
            conn.publish::<_, _, ()>(&channel, format!("data_{}", i))
                .await
                .unwrap();
        }
        
        // 接收消息
        let mut received_count = 0;
        let start = tokio::time::Instant::now();
        
        while received_count < 10 && start.elapsed() < Duration::from_secs(2) {
            if timeout(Duration::from_millis(300), rx.recv())
                .await
                .is_ok()
            {
                received_count += 1;
            }
        }
        
        assert_eq!(received_count, 10, "应该接收到所有10条消息");
        
        subscriber_handle.abort();
    }
}

#[tokio::test]
async fn test_point_data_parsing() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig::default();
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    if subscriber.connect().await.is_ok() {
        subscriber
            .subscribe_channels(vec!["test_point:*".to_string()])
            .await
            .unwrap();
        
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber.start_listening().await;
        });
        
        sleep(Duration::from_millis(100)).await;
        
        // 发布 PointData 格式的消息
        let redis_client = RedisClient::new("redis://127.0.0.1:6379").await.unwrap();
        let mut conn = redis_client.get_connection().await.unwrap();
        
        let point_data = PointData {
            id: 10001,
            value: PointValue::Float(220.5),
            quality: 192,
            timestamp: chrono::Utc::now(),
            source: Some("test_subscriber".to_string()),
        };
        
        use redis::AsyncCommands;
        conn.publish::<_, _, ()>(
            "test_point:m:10001",
            serde_json::to_string(&point_data).unwrap(),
        )
        .await
        .unwrap();
        
        // 接收并验证
        if let Ok(Some(msg)) = timeout(Duration::from_secs(1), rx.recv()).await {
            assert!(msg.point_data.is_some());
            let received_point = msg.point_data.unwrap();
            assert_eq!(received_point.id, 10001);
            match received_point.value {
                PointValue::Float(v) => assert_eq!(v, 220.5),
                _ => panic!("错误的值类型"),
            }
        }
        
        subscriber_handle.abort();
    }
}

#[tokio::test]
async fn test_reconnection() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig {
        max_reconnect_attempts: 3,
        reconnect_delay_ms: 100,
        ..Default::default()
    };
    let (tx, _rx) = mpsc::unbounded_channel();
    
    let reconnect_attempts = Arc::new(RwLock::new(0u32));
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    // 这个测试主要验证重连逻辑的结构是否正确
    // 实际的重连测试需要能够模拟 Redis 断开连接
    
    // 连接
    if subscriber.connect().await.is_ok() {
        // 获取初始重连次数
        let initial_attempts = *subscriber.reconnect_attempts.read().await;
        assert_eq!(initial_attempts, 0);
        
        // 断开连接
        subscriber.disconnect().await.unwrap();
        
        // 验证状态
        assert_eq!(subscriber.get_state().await, SubscriberState::Stopped);
    }
}

#[tokio::test]
async fn test_metadata_enrichment() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig::default();
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    if subscriber.connect().await.is_ok() {
        subscriber
            .subscribe_channels(vec!["test_meta:*:*".to_string()])
            .await
            .unwrap();
        
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber.start_listening().await;
        });
        
        sleep(Duration::from_millis(100)).await;
        
        // 发布到特定格式的通道
        let redis_client = RedisClient::new("redis://127.0.0.1:6379").await.unwrap();
        let mut conn = redis_client.get_connection().await.unwrap();
        
        use redis::AsyncCommands;
        conn.publish::<_, _, ()>("test_meta:m:10001", "123.45")
            .await
            .unwrap();
        
        // 接收并验证元数据
        if let Ok(Some(msg)) = timeout(Duration::from_secs(1), rx.recv()).await {
            assert!(msg.channel_info.is_some());
            let channel_info = msg.channel_info.unwrap();
            assert_eq!(channel_info.channel_id, 0); // test_meta 不能解析为数字
            assert_eq!(channel_info.point_id, 10001);
            
            // 验证元数据
            assert!(msg.metadata.contains_key("source"));
            assert_eq!(msg.metadata.get("source").unwrap(), "redis_subscriber");
        }
        
        subscriber_handle.abort();
    }
}

#[tokio::test]
async fn test_graceful_shutdown() {
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig::default();
    let (tx, rx) = mpsc::unbounded_channel();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    if subscriber.connect().await.is_ok() {
        subscriber
            .subscribe_channels(vec!["test_shutdown:*".to_string()])
            .await
            .unwrap();
        
        // 启动监听
        let mut subscriber_clone = subscriber;
        let subscriber_handle = tokio::spawn(async move {
            let _ = subscriber_clone.start_listening().await;
            subscriber_clone
        });
        
        sleep(Duration::from_millis(100)).await;
        
        // 优雅关闭
        if let Ok(mut sub) = timeout(Duration::from_secs(1), subscriber_handle).await {
            sub.shutdown().await.unwrap();
            assert_eq!(sub.get_state().await, SubscriberState::Stopped);
        }
        
        // 验证通道已关闭
        drop(rx);
    }
}