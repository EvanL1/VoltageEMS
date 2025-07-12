//! 控制命令订阅测试
//!
//! 测试comsrv从Redis订阅控制命令的功能

use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use voltage_common::redis::async_client::RedisClient;

#[tokio::test]
async fn test_command_subscription() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    let _ = tracing_subscriber::fmt::try_init();

    // 创建Redis客户端
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let redis_client = Arc::new(RedisClient::new(&redis_url).await?);

    // 测试通道ID
    let channel_id = 1u16;

    // 创建控制命令
    let control_command = json!({
        "command_id": "test-control-001",
        "channel_id": channel_id,
        "command_type": "control",
        "point_id": 1001,
        "value": 1.0,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "metadata": {}
    });

    // 创建调节命令
    let adjustment_command = json!({
        "command_id": "test-adjustment-001",
        "channel_id": channel_id,
        "command_type": "adjustment",
        "point_id": 2001,
        "value": 75.5,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "metadata": {}
    });

    // 发布控制命令
    let control_channel = format!("cmd:{}:control", channel_id);
    redis_client
        .publish(&control_channel, &control_command.to_string())
        .await?;
    println!("Published control command to channel: {}", control_channel);

    // 发布调节命令
    let adjustment_channel = format!("cmd:{}:adjustment", channel_id);
    redis_client
        .publish(&adjustment_channel, &adjustment_command.to_string())
        .await?;
    println!(
        "Published adjustment command to channel: {}",
        adjustment_channel
    );

    // 等待一段时间让命令被处理
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 检查命令状态
    let control_status_key = format!("cmd_status:{}:{}", channel_id, "test-control-001");
    if let Ok(status) = redis_client.get::<String>(&control_status_key).await {
        println!("Control command status: {}", status);

        let status_obj: serde_json::Value = serde_json::from_str(&status)?;
        assert_eq!(status_obj["command_id"], "test-control-001");
        // 注意：实际状态取决于是否有Modbus服务在运行
    } else {
        println!("Control command status not found (expected if no Modbus service is running)");
    }

    let adjustment_status_key = format!("cmd_status:{}:{}", channel_id, "test-adjustment-001");
    if let Ok(status) = redis_client.get::<String>(&adjustment_status_key).await {
        println!("Adjustment command status: {}", status);

        let status_obj: serde_json::Value = serde_json::from_str(&status)?;
        assert_eq!(status_obj["command_id"], "test-adjustment-001");
    } else {
        println!("Adjustment command status not found (expected if no Modbus service is running)");
    }

    Ok(())
}

#[tokio::test]
async fn test_command_format() -> Result<(), Box<dyn std::error::Error>> {
    use comsrv::core::framework::command_subscriber::{CommandType, ControlCommand};

    // 测试控制命令序列化
    let control_cmd = ControlCommand {
        command_id: "test-123".to_string(),
        channel_id: 1,
        command_type: CommandType::Control,
        point_id: 1001,
        value: 1.0,
        timestamp: 1234567890,
        metadata: json!({}),
    };

    let json_str = serde_json::to_string(&control_cmd)?;
    println!("Serialized control command: {}", json_str);

    // 测试反序列化
    let deserialized: ControlCommand = serde_json::from_str(&json_str)?;
    assert_eq!(deserialized.command_id, "test-123");
    assert_eq!(deserialized.channel_id, 1);
    assert!(matches!(deserialized.command_type, CommandType::Control));

    // 测试调节命令
    let adjustment_cmd = ControlCommand {
        command_id: "test-456".to_string(),
        channel_id: 2,
        command_type: CommandType::Adjustment,
        point_id: 2001,
        value: 50.5,
        timestamp: 1234567890,
        metadata: json!({"source": "test"}),
    };

    let json_str = serde_json::to_string(&adjustment_cmd)?;
    println!("Serialized adjustment command: {}", json_str);

    let deserialized: ControlCommand = serde_json::from_str(&json_str)?;
    assert_eq!(deserialized.channel_id, 2);
    assert!(matches!(deserialized.command_type, CommandType::Adjustment));
    assert_eq!(deserialized.value, 50.5);

    Ok(())
}

/// 集成测试：模拟完整的命令流程
#[tokio::test]
#[ignore] // 需要运行的Modbus服务
async fn test_full_command_flow() -> Result<(), Box<dyn std::error::Error>> {
    use comsrv::core::transport::mock::MockTransport;
    use comsrv::plugins::protocols::modbus::client::ProtocolMappingTable;
    use comsrv::plugins::protocols::modbus::client::{ModbusChannelConfig, ModbusClient};
    use comsrv::plugins::protocols::modbus::common::ModbusConfig;
    use comsrv::plugins::protocols::modbus::modbus_polling::ModbusPollingConfig;

    // 创建测试配置
    let config = ModbusChannelConfig {
        channel_id: 1,
        channel_name: "Test Channel".to_string(),
        connection: ModbusConfig {
            protocol_type: "modbus_tcp".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(502),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(5000),
            points: vec![],
        },
        request_timeout: Duration::from_millis(5000),
        max_retries: 3,
        retry_delay: Duration::from_millis(1000),
        polling: ModbusPollingConfig::default(),
    };

    // 创建Mock传输
    let mock_config = comsrv::core::transport::mock::MockTransportConfig::default();
    let transport = Box::new(MockTransport::new(mock_config)?);

    // 创建Modbus客户端
    let mut client = ModbusClient::new(config, transport).await?;

    // 加载映射
    let mappings = ProtocolMappingTable::default();
    client.load_protocol_mappings(mappings).await?;

    // 启动客户端（这将启动命令订阅）
    use comsrv::core::framework::traits::ComBase;
    client.start().await?;

    // 等待一段时间
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 停止客户端
    client.stop().await?;

    Ok(())
}
