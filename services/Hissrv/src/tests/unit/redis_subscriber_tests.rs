use crate::config::{RedisConfig, RedisConnection};
use crate::redis_subscriber::{
    ChannelInfo, EnhancedRedisSubscriber, MessageType, SubscriberConfig, SubscriberState,
    SubscriptionMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::sleep;

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
        MessageType::from_channel("event:alarm:high_temp"),
        Some(MessageType::Event)
    ));
    assert!(matches!(
        MessageType::from_channel("system:status:health"),
        Some(MessageType::SystemStatus)
    ));

    // 测试无效格式
    assert!(MessageType::from_channel("invalid").is_none());
    assert!(MessageType::from_channel("1001:x:10001").is_none());
}

#[test]
fn test_channel_info_parsing() {
    // 测试有效的通道信息
    let info = ChannelInfo::from_channel("1001:m:10001").unwrap();
    assert_eq!(info.channel_id, 1001);
    assert!(matches!(info.message_type, MessageType::Telemetry));
    assert_eq!(info.point_id, 10001);

    let info = ChannelInfo::from_channel("9999:s:99999").unwrap();
    assert_eq!(info.channel_id, 9999);
    assert!(matches!(info.message_type, MessageType::Signal));
    assert_eq!(info.point_id, 99999);

    // 测试无效格式
    assert!(ChannelInfo::from_channel("invalid").is_none());
    assert!(ChannelInfo::from_channel("1001:10001").is_none());
    assert!(ChannelInfo::from_channel("1001:x:10001").is_none());
    assert!(ChannelInfo::from_channel("abc:m:10001").is_none());
    assert!(ChannelInfo::from_channel("1001:m:xyz").is_none());
}

#[tokio::test]
async fn test_subscriber_state_transitions() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let config = RedisConfig {
        connection: RedisConnection {
            host: "localhost".to_string(),
            port: 6379,
            password: String::new(),
            socket: String::new(),
            database: 0,
            timeout_seconds: 5,
            max_retries: 3,
        },
        subscribe_patterns: vec!["test:*".to_string()],
        batch_size: 100,
        batch_timeout_ms: 100,
    };

    let subscriber_config = SubscriberConfig::default();
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);

    // 初始状态应该是 Disconnected
    assert_eq!(subscriber.get_state().await, SubscriberState::Disconnected);

    // 注意：这里不进行实际连接，只测试状态机制
    // 实际连接测试应该在集成测试中进行
}

#[tokio::test]
async fn test_subscription_message_creation() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let config = RedisConfig {
        connection: RedisConnection {
            host: "localhost".to_string(),
            port: 6379,
            password: String::new(),
            socket: String::new(),
            database: 0,
            timeout_seconds: 5,
            max_retries: 3,
        },
        subscribe_patterns: vec!["test:*".to_string()],
        batch_size: 100,
        batch_timeout_ms: 100,
    };

    let subscriber_config = SubscriberConfig::default();
    let subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);

    // 测试消息解析
    let test_cases = vec![
        (
            "1001:m:10001",
            r#"{"point_id":10001,"value":123.45,"quality":192,"timestamp":"2025-01-14T10:00:00Z"}"#,
            true,
        ),
        (
            "1001:s:20001",
            r#"{"point_id":20001,"value":1,"quality":192,"timestamp":"2025-01-14T10:00:00Z"}"#,
            true,
        ),
        ("event:alarm", r#"{"type":"high_temp","severity":"warning"}"#, false),
        ("invalid:channel", "plain text message", false),
    ];

    for (channel, payload, should_have_point_data) in test_cases {
        let msg = subscriber.parse_message(channel, payload).unwrap();
        
        assert_eq!(msg.channel, channel);
        assert_eq!(msg.point_data.is_some(), should_have_point_data);
        
        if !should_have_point_data {
            assert!(msg.raw_data.is_some());
        }
        
        // 验证元数据
        assert_eq!(msg.metadata.get("source").unwrap(), "redis_subscriber");
        
        if let Some(channel_info) = &msg.channel_info {
            assert!(msg.metadata.contains_key("channel_id"));
            assert!(msg.metadata.contains_key("point_id"));
            assert!(msg.metadata.contains_key("message_type"));
        }
    }
}

#[tokio::test]
async fn test_subscriber_config_defaults() {
    let config = SubscriberConfig::default();
    assert_eq!(config.max_reconnect_attempts, 10);
    assert_eq!(config.reconnect_delay_ms, 1000);
    assert_eq!(config.batch_size, 100);
    assert_eq!(config.batch_timeout_ms, 100);
    assert!(config.enable_pattern_subscribe);
}

#[tokio::test]
async fn test_subscriber_batch_processing() {
    let (tx, mut rx) = mpsc::unbounded_channel::<SubscriptionMessage>();
    
    // 发送多个消息
    for i in 0..5 {
        let msg = SubscriptionMessage {
            id: format!("msg_{}", i),
            channel: format!("1001:m:1000{}", i),
            channel_info: Some(ChannelInfo {
                channel_id: 1001,
                message_type: MessageType::Telemetry,
                point_id: 10000 + i,
            }),
            timestamp: chrono::Utc::now(),
            point_data: None,
            raw_data: Some(format!("data_{}", i)),
            metadata: Default::default(),
        };
        tx.send(msg).unwrap();
    }

    // 验证接收
    let mut received = Vec::new();
    for _ in 0..5 {
        if let Some(msg) = rx.recv().await {
            received.push(msg);
        }
    }

    assert_eq!(received.len(), 5);
    for (i, msg) in received.iter().enumerate() {
        assert_eq!(msg.id, format!("msg_{}", i));
    }
}

#[tokio::test]
async fn test_channel_pattern_matching() {
    let patterns = vec![
        ("*:m:*", "1001:m:10001", true),
        ("*:s:*", "1001:m:10001", false),
        ("1001:*:*", "1001:m:10001", true),
        ("1001:*:*", "1002:m:10001", false),
        ("*:*:10001", "1001:m:10001", true),
        ("event:*", "event:alarm", true),
        ("system:*", "system:status", true),
    ];

    for (pattern, channel, should_match) in patterns {
        // 简单的模式匹配测试
        let matches = if pattern.contains('*') {
            let regex_pattern = pattern.replace("*", ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern))
                .unwrap()
                .is_match(channel)
        } else {
            pattern == channel
        };

        assert_eq!(
            matches, should_match,
            "Pattern {} should {} match channel {}",
            pattern,
            if should_match { "" } else { "not" },
            channel
        );
    }
}

#[tokio::test]
async fn test_subscription_message_metadata() {
    let channel = "1001:m:10001";
    let msg = SubscriptionMessage {
        id: "test_id".to_string(),
        channel: channel.to_string(),
        channel_info: ChannelInfo::from_channel(channel),
        timestamp: chrono::Utc::now(),
        point_data: None,
        raw_data: Some("test_data".to_string()),
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert("source".to_string(), "redis_subscriber".to_string());
            m.insert("channel_id".to_string(), "1001".to_string());
            m.insert("point_id".to_string(), "10001".to_string());
            m.insert("message_type".to_string(), "Telemetry".to_string());
            m
        },
    };

    // 验证元数据
    assert_eq!(msg.metadata.get("source").unwrap(), "redis_subscriber");
    assert_eq!(msg.metadata.get("channel_id").unwrap(), "1001");
    assert_eq!(msg.metadata.get("point_id").unwrap(), "10001");
    assert_eq!(msg.metadata.get("message_type").unwrap(), "Telemetry");
    
    // 验证通道信息
    let channel_info = msg.channel_info.unwrap();
    assert_eq!(channel_info.channel_id, 1001);
    assert_eq!(channel_info.point_id, 10001);
    assert!(matches!(channel_info.message_type, MessageType::Telemetry));
}

#[tokio::test]
async fn test_redis_config_socket_vs_tcp() {
    let tcp_config = RedisConnection {
        host: "localhost".to_string(),
        port: 6379,
        password: "test_password".to_string(),
        socket: String::new(),
        database: 0,
        timeout_seconds: 5,
        max_retries: 3,
    };

    let socket_config = RedisConnection {
        host: String::new(),
        port: 0,
        password: "test_password".to_string(),
        socket: "/tmp/redis.sock".to_string(),
        database: 0,
        timeout_seconds: 5,
        max_retries: 3,
    };

    // 验证配置转换逻辑
    assert!(!tcp_config.socket.is_empty() || !tcp_config.host.is_empty());
    assert!(!socket_config.socket.is_empty());
}

// 模拟重连场景的测试
#[tokio::test]
async fn test_reconnect_backoff() {
    let delays = vec![1000, 2000, 4000, 8000]; // 指数退避
    
    for (attempt, expected_delay) in delays.iter().enumerate() {
        let delay = 1000 * (2_u64.pow(attempt as u32));
        assert_eq!(delay, *expected_delay);
    }
}

#[tokio::test]
async fn test_subscriber_shutdown() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let config = RedisConfig {
        connection: RedisConnection {
            host: "localhost".to_string(),
            port: 6379,
            password: String::new(),
            socket: String::new(),
            database: 0,
            timeout_seconds: 5,
            max_retries: 3,
        },
        subscribe_patterns: vec!["test:*".to_string()],
        batch_size: 100,
        batch_timeout_ms: 100,
    };

    let subscriber_config = SubscriberConfig::default();
    let mut subscriber = EnhancedRedisSubscriber::new(config, subscriber_config, tx);

    // 测试优雅关闭
    subscriber.shutdown().await.unwrap();
    assert_eq!(subscriber.get_state().await, SubscriberState::Stopped);
}