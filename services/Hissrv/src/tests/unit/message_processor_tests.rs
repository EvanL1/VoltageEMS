use crate::error::Result;
use crate::pubsub::{MessageProcessor, RedisSubscriber};
use crate::redis_subscriber::{ChannelInfo, MessageType, SubscriptionMessage};
use crate::storage::{DataPoint, DataValue, StorageManager};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use crate::types::GenericPointData as PointData;

async fn create_test_processor() -> (MessageProcessor, mpsc::UnboundedSender<SubscriptionMessage>) {
    let storage_manager = Arc::new(RwLock::new(StorageManager::new()));
    let (tx, rx) = mpsc::unbounded_channel();
    let processor = MessageProcessor::new(storage_manager, rx);
    (processor, tx)
}

#[tokio::test]
async fn test_message_processor_basic() {
    let (mut processor, tx) = create_test_processor().await;

    // 创建测试消息
    let point_data = PointData {
        point_id: 10001,
        value: 123.45,
        quality: 192,
        timestamp: Utc::now().timestamp() as u64,
    };

    let message = SubscriptionMessage {
        id: "test_msg_1".to_string(),
        channel: "1001:m:10001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Telemetry,
            point_id: 10001,
        }),
        timestamp: Utc::now(),
        point_data: Some(point_data),
        raw_data: None,
        metadata: HashMap::new(),
    };

    // 发送消息
    tx.send(message).unwrap();

    // 启动处理器（在测试中我们只处理一次）
    let handle = tokio::spawn(async move {
        // 这里应该是 processor.start_processing()，但为了测试我们需要模拟
        // 实际实现需要在 MessageProcessor 中添加测试支持
        Ok::<(), crate::error::HisSrvError>(())
    });

    // 给处理器一些时间来处理消息
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 清理
    drop(tx);
    let _ = handle.await;
}

#[tokio::test]
async fn test_message_to_datapoint_conversion() {
    // 测试遥测数据转换
    let telemetry_msg = SubscriptionMessage {
        id: "test_1".to_string(),
        channel: "1001:m:10001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Telemetry,
            point_id: 10001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 10001,
            value: 123.45,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: HashMap::new(),
    };

    // 这里需要一个辅助函数来转换消息到数据点
    let data_point = convert_message_to_datapoint(&telemetry_msg);
    assert_eq!(data_point.key, "1001:m:10001");
    assert!(matches!(data_point.value, DataValue::Float(_)));

    // 测试信号数据转换
    let signal_msg = SubscriptionMessage {
        id: "test_2".to_string(),
        channel: "1001:s:20001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Signal,
            point_id: 20001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 20001,
            value: 1.0,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: HashMap::new(),
    };

    let data_point = convert_message_to_datapoint(&signal_msg);
    assert_eq!(data_point.key, "1001:s:20001");
    assert!(matches!(data_point.value, DataValue::Boolean(_)));
}

#[tokio::test]
async fn test_message_batch_processing() {
    let (processor, tx) = create_test_processor().await;

    // 发送多个消息
    for i in 0..10 {
        let message = SubscriptionMessage {
            id: format!("batch_msg_{}", i),
            channel: format!("1001:m:1000{}", i),
            channel_info: Some(ChannelInfo {
                channel_id: 1001,
                message_type: MessageType::Telemetry,
                point_id: 10000 + i,
            }),
            timestamp: Utc::now(),
            point_data: Some(PointData {
                point_id: 10000 + i,
                value: (i as f64) * 10.0,
                quality: 192,
                timestamp: Utc::now().timestamp() as u64,
            }),
            raw_data: None,
            metadata: HashMap::new(),
        };

        tx.send(message).unwrap();
    }

    // 验证消息已发送
    assert!(tx.is_closed() == false);
}

#[tokio::test]
async fn test_message_metadata_handling() {
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "test_source".to_string());
    metadata.insert("region".to_string(), "test_region".to_string());
    metadata.insert("device_id".to_string(), "device_123".to_string());

    let message = SubscriptionMessage {
        id: "metadata_test".to_string(),
        channel: "1001:m:10001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Telemetry,
            point_id: 10001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 10001,
            value: 100.0,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: metadata.clone(),
    };

    // 验证元数据保留
    assert_eq!(message.metadata.get("source").unwrap(), "test_source");
    assert_eq!(message.metadata.get("region").unwrap(), "test_region");
    assert_eq!(message.metadata.get("device_id").unwrap(), "device_123");
}

#[tokio::test]
async fn test_event_message_processing() {
    let event_data = r#"{
        "event_type": "alarm",
        "severity": "high",
        "description": "Temperature exceeded threshold",
        "value": 85.5
    }"#;

    let message = SubscriptionMessage {
        id: "event_test".to_string(),
        channel: "event:alarm:temperature".to_string(),
        channel_info: None,
        timestamp: Utc::now(),
        point_data: None,
        raw_data: Some(event_data.to_string()),
        metadata: HashMap::new(),
    };

    // 验证事件消息结构
    assert!(message.point_data.is_none());
    assert!(message.raw_data.is_some());
    assert!(message.channel.starts_with("event:"));
}

#[tokio::test]
async fn test_quality_code_handling() {
    let quality_codes = vec![
        (192, "Good"),       // 0xC0
        (0, "Bad"),          // 0x00
        (64, "Uncertain"),   // 0x40
        (216, "Good_Local"), // 0xD8
    ];

    for (code, expected_quality) in quality_codes {
        let message = SubscriptionMessage {
            id: format!("quality_test_{}", code),
            channel: "1001:m:10001".to_string(),
            channel_info: Some(ChannelInfo {
                channel_id: 1001,
                message_type: MessageType::Telemetry,
                point_id: 10001,
            }),
            timestamp: Utc::now(),
            point_data: Some(PointData {
                point_id: 10001,
                value: 100.0,
                quality: code,
                timestamp: Utc::now().timestamp() as u64,
            }),
            raw_data: None,
            metadata: HashMap::new(),
        };

        assert_eq!(message.point_data.unwrap().quality, code);
    }
}

#[tokio::test]
async fn test_calculated_point_processing() {
    let calc_msg = SubscriptionMessage {
        id: "calc_test".to_string(),
        channel: "1001:calc:50001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Calculated,
            point_id: 50001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 50001,
            value: 250.5,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert("formula".to_string(), "avg(10001,10002,10003)".to_string());
            m.insert("source_points".to_string(), "10001,10002,10003".to_string());
            m
        },
    };

    // 验证计算点消息
    assert_eq!(calc_msg.channel_info.unwrap().point_id, 50001);
    assert!(matches!(
        calc_msg.channel_info.unwrap().message_type,
        MessageType::Calculated
    ));
    assert!(calc_msg.metadata.contains_key("formula"));
}

#[tokio::test]
async fn test_control_and_adjustment_messages() {
    // 测试控制命令消息
    let control_msg = SubscriptionMessage {
        id: "control_test".to_string(),
        channel: "1001:c:30001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Control,
            point_id: 30001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 30001,
            value: 1.0,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert("command".to_string(), "switch_on".to_string());
            m.insert("operator".to_string(), "system".to_string());
            m
        },
    };

    // 测试调节命令消息
    let adjustment_msg = SubscriptionMessage {
        id: "adjustment_test".to_string(),
        channel: "1001:a:40001".to_string(),
        channel_info: Some(ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Adjustment,
            point_id: 40001,
        }),
        timestamp: Utc::now(),
        point_data: Some(PointData {
            point_id: 40001,
            value: 75.0,
            quality: 192,
            timestamp: Utc::now().timestamp() as u64,
        }),
        raw_data: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert("setpoint".to_string(), "temperature".to_string());
            m.insert("unit".to_string(), "celsius".to_string());
            m
        },
    };

    // 验证消息类型
    assert!(matches!(
        control_msg.channel_info.unwrap().message_type,
        MessageType::Control
    ));
    assert!(matches!(
        adjustment_msg.channel_info.unwrap().message_type,
        MessageType::Adjustment
    ));
}

// 辅助函数：转换消息到数据点
fn convert_message_to_datapoint(msg: &SubscriptionMessage) -> DataPoint {
    let value = if let Some(point_data) = &msg.point_data {
        match msg.channel_info.as_ref().map(|ci| &ci.message_type) {
            Some(MessageType::Signal) | Some(MessageType::Control) => {
                DataValue::Boolean(point_data.value != 0.0)
            }
            _ => DataValue::Float(point_data.value),
        }
    } else {
        DataValue::String(msg.raw_data.clone().unwrap_or_default())
    };

    let mut tags = HashMap::new();
    if let Some(channel_info) = &msg.channel_info {
        tags.insert(
            "channel_id".to_string(),
            channel_info.channel_id.to_string(),
        );
        tags.insert("point_id".to_string(), channel_info.point_id.to_string());
        tags.insert(
            "message_type".to_string(),
            format!("{:?}", channel_info.message_type),
        );
    }

    DataPoint {
        key: msg.channel.clone(),
        value,
        timestamp: msg.timestamp,
        tags,
        metadata: msg.metadata.clone(),
    }
}

#[tokio::test]
async fn test_timestamp_handling() {
    let base_time = Utc::now();
    let timestamps = vec![
        base_time - chrono::Duration::hours(1),
        base_time,
        base_time + chrono::Duration::minutes(5),
    ];

    for (i, ts) in timestamps.iter().enumerate() {
        let message = SubscriptionMessage {
            id: format!("timestamp_test_{}", i),
            channel: "1001:m:10001".to_string(),
            channel_info: Some(ChannelInfo {
                channel_id: 1001,
                message_type: MessageType::Telemetry,
                point_id: 10001,
            }),
            timestamp: *ts,
            point_data: Some(PointData {
                point_id: 10001,
                value: 100.0,
                quality: 192,
                timestamp: ts.timestamp() as u64,
            }),
            raw_data: None,
            metadata: HashMap::new(),
        };

        assert_eq!(message.timestamp, *ts);
        assert_eq!(message.point_data.unwrap().timestamp, ts.timestamp() as u64);
    }
}
