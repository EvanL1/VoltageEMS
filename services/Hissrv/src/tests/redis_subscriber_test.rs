//! Redis 订阅器测试

use crate::config::{RedisConfig, RedisConnection};
use crate::redis_subscriber::{
    ChannelInfo, EnhancedRedisSubscriber, MessageType, SubscriberConfig, SubscriberState,
    SubscriptionMessage,
};
use crate::tests::test_utils::{create_test_point_data, generate_test_channel};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use voltage_common::data::PointData;

/// 创建测试用的 Redis 配置
fn create_test_redis_config() -> RedisConfig {
    RedisConfig {
        connection: RedisConnection {
            host: "localhost".to_string(),
            port: 6379,
            password: String::new(),
            socket: String::new(),
            database: 15,
            timeout_seconds: 5,
            max_retries: 3,
        },
        subscription: vec!["test:*".to_string(), "1001:*:*".to_string()],
        batch_size: 100,
        flush_interval_ms: 100,
    }
}

#[test]
fn test_message_type_parsing() {
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
    
    // 测试事件和系统状态
    assert!(matches!(
        MessageType::from_channel("event:alarm:critical"),
        Some(MessageType::Event)
    ));
    assert!(matches!(
        MessageType::from_channel("system:status:health"),
        Some(MessageType::SystemStatus)
    ));
    
    // 测试无效格式
    assert!(MessageType::from_channel("invalid").is_none());
    assert!(MessageType::from_channel("1001:unknown:10001").is_none());
}

#[test]
fn test_channel_info_parsing() {
    // 测试有效的通道信息
    let info = ChannelInfo::from_channel("1001:m:10001").unwrap();
    assert_eq!(info.channel_id, 1001);
    assert!(matches!(info.message_type, MessageType::Telemetry));
    assert_eq!(info.point_id, 10001);
    
    let info = ChannelInfo::from_channel("2002:s:20002").unwrap();
    assert_eq!(info.channel_id, 2002);
    assert!(matches!(info.message_type, MessageType::Signal));
    assert_eq!(info.point_id, 20002);
    
    // 测试无效格式
    assert!(ChannelInfo::from_channel("invalid").is_none());
    assert!(ChannelInfo::from_channel("1001:10001").is_none());
    assert!(ChannelInfo::from_channel("1001:x:10001").is_none());
    assert!(ChannelInfo::from_channel("abc:m:10001").is_none());
    assert!(ChannelInfo::from_channel("1001:m:abc").is_none());
}

#[tokio::test]
async fn test_subscriber_state_transitions() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let config = create_test_redis_config();
    let subscriber_config = SubscriberConfig::default();
    
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);
    
    // 初始状态应该是断开连接
    assert_eq!(subscriber.get_state().await, SubscriberState::Disconnected);
    
    // 注意：以下测试需要实际的 Redis 连接，在 CI 环境中可能需要跳过
    // 这里只测试状态机逻辑
}

#[test]
fn test_message_generation() {
    // 测试通道名称生成
    assert_eq!(generate_test_channel(1001, "m", 10001), "1001:m:10001");
    assert_eq!(generate_test_channel(2002, "s", 20002), "2002:s:20002");
    assert_eq!(generate_test_channel(3003, "c", 30003), "3003:c:30003");
    assert_eq!(generate_test_channel(4004, "a", 40004), "4004:a:40004");
}

#[tokio::test]
async fn test_subscriber_config() {
    let config = SubscriberConfig {
        max_reconnect_attempts: 5,
        reconnect_delay_ms: 500,
        batch_size: 50,
        batch_timeout_ms: 200,
        enable_pattern_subscribe: false,
    };
    
    assert_eq!(config.max_reconnect_attempts, 5);
    assert_eq!(config.reconnect_delay_ms, 500);
    assert_eq!(config.batch_size, 50);
    assert_eq!(config.batch_timeout_ms, 200);
    assert!(!config.enable_pattern_subscribe);
    
    // 测试默认配置
    let default_config = SubscriberConfig::default();
    assert_eq!(default_config.max_reconnect_attempts, 10);
    assert_eq!(default_config.reconnect_delay_ms, 1000);
    assert_eq!(default_config.batch_size, 100);
    assert_eq!(default_config.batch_timeout_ms, 100);
    assert!(default_config.enable_pattern_subscribe);
}

#[tokio::test]
async fn test_subscription_message_creation() {
    let channel = "1001:m:10001";
    let point_data = create_test_point_data(1001, 10001, 42.5);
    let payload = serde_json::to_string(&point_data).unwrap();
    
    // 模拟解析消息
    let channel_info = ChannelInfo::from_channel(channel);
    assert!(channel_info.is_some());
    
    let msg = SubscriptionMessage {
        id: "test-id".to_string(),
        channel: channel.to_string(),
        channel_info: channel_info.clone(),
        timestamp: Utc::now(),
        point_data: Some(point_data.clone()),
        raw_data: None,
        metadata: std::collections::HashMap::new(),
    };
    
    assert_eq!(msg.channel, "1001:m:10001");
    assert!(msg.channel_info.is_some());
    
    let info = msg.channel_info.unwrap();
    assert_eq!(info.channel_id, 1001);
    assert_eq!(info.point_id, 10001);
    assert!(matches!(info.message_type, MessageType::Telemetry));
}

#[tokio::test]
async fn test_message_batch_collection() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubscriptionMessage>();
    
    // 发送多个消息
    for i in 0..10 {
        let msg = SubscriptionMessage {
            id: format!("msg-{}", i),
            channel: generate_test_channel(1001, "m", 10001 + i),
            channel_info: ChannelInfo::from_channel(&generate_test_channel(1001, "m", 10001 + i)),
            timestamp: Utc::now(),
            point_data: Some(create_test_point_data(1001, 10001 + i, i as f64)),
            raw_data: None,
            metadata: std::collections::HashMap::new(),
        };
        tx.send(msg).unwrap();
    }
    
    // 收集批量消息
    let mut batch = Vec::new();
    let timeout = tokio::time::sleep(Duration::from_millis(100));
    tokio::pin!(timeout);
    
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                batch.push(msg);
                if batch.len() >= 5 {
                    break;
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }
    
    assert_eq!(batch.len(), 5);
}

#[tokio::test]
async fn test_channel_pattern_matching() {
    let patterns = vec![
        ("1001:*:*", "1001:m:10001", true),
        ("1001:*:*", "1002:m:10001", false),
        ("*:m:*", "1001:m:10001", true),
        ("*:m:*", "1001:s:10001", false),
        ("test:*", "test:data", true),
        ("test:*", "prod:data", false),
    ];
    
    for (pattern, channel, should_match) in patterns {
        // 简单的模式匹配测试
        if pattern.contains('*') {
            let pattern_prefix = pattern.split('*').next().unwrap();
            let matches = channel.starts_with(pattern_prefix);
            assert_eq!(
                matches, should_match,
                "Pattern {} should {} match channel {}",
                pattern,
                if should_match { "" } else { "not" },
                channel
            );
        }
    }
}

#[tokio::test]
async fn test_subscriber_metrics() {
    use std::sync::atomic::{AtomicU64, Ordering};
    
    // 模拟订阅器指标
    let messages_received = Arc::new(AtomicU64::new(0));
    let messages_processed = Arc::new(AtomicU64::new(0));
    let messages_failed = Arc::new(AtomicU64::new(0));
    
    // 模拟处理消息
    for i in 0..100 {
        messages_received.fetch_add(1, Ordering::Relaxed);
        
        if i % 10 == 0 {
            // 模拟失败
            messages_failed.fetch_add(1, Ordering::Relaxed);
        } else {
            messages_processed.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    assert_eq!(messages_received.load(Ordering::Relaxed), 100);
    assert_eq!(messages_processed.load(Ordering::Relaxed), 90);
    assert_eq!(messages_failed.load(Ordering::Relaxed), 10);
    
    // 计算成功率
    let success_rate = messages_processed.load(Ordering::Relaxed) as f64
        / messages_received.load(Ordering::Relaxed) as f64
        * 100.0;
    assert!((success_rate - 90.0).abs() < 0.01);
}

#[tokio::test]
async fn test_concurrent_message_processing() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubscriptionMessage>();
    let processed = Arc::new(RwLock::new(Vec::new()));
    
    // 启动处理任务
    let processed_clone = processed.clone();
    let processor_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let mut p = processed_clone.write().await;
            p.push(msg.id.clone());
            if p.len() >= 50 {
                break;
            }
        }
    });
    
    // 并发发送消息
    let mut send_tasks = vec![];
    for i in 0..5 {
        let tx_clone = tx.clone();
        let task = tokio::spawn(async move {
            for j in 0..10 {
                let msg = SubscriptionMessage {
                    id: format!("msg-{}-{}", i, j),
                    channel: generate_test_channel(1000 + i, "m", 10000 + j),
                    channel_info: None,
                    timestamp: Utc::now(),
                    point_data: None,
                    raw_data: Some(format!("data-{}-{}", i, j)),
                    metadata: std::collections::HashMap::new(),
                };
                tx_clone.send(msg).unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        send_tasks.push(task);
    }
    
    // 等待所有发送任务完成
    for task in send_tasks {
        task.await.unwrap();
    }
    
    // 等待处理完成
    processor_task.await.unwrap();
    
    // 验证所有消息都被处理
    let processed_msgs = processed.read().await;
    assert_eq!(processed_msgs.len(), 50);
}

#[tokio::test]
async fn test_message_metadata() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("source".to_string(), "test_subscriber".to_string());
    metadata.insert("channel_id".to_string(), "1001".to_string());
    metadata.insert("point_id".to_string(), "10001".to_string());
    metadata.insert("message_type".to_string(), "Telemetry".to_string());
    
    assert_eq!(metadata.get("source").unwrap(), "test_subscriber");
    assert_eq!(metadata.get("channel_id").unwrap(), "1001");
    assert_eq!(metadata.get("point_id").unwrap(), "10001");
    assert_eq!(metadata.get("message_type").unwrap(), "Telemetry");
}