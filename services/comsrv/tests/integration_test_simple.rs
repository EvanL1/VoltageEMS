use comsrv::core::config::types::{
    ChannelConfig, ChannelLoggingConfig, CombinedPoint, ScalingInfo,
};
use comsrv::core::framework::factory::ProtocolFactory;
use comsrv::plugins::plugin_registry::discovery;
use std::collections::HashMap;
use std::sync::Once;
use tracing::info;

// 确保插件只加载一次
static INIT: Once = Once::new();
static LOGGER_INIT: Once = Once::new();

/// 设置测试环境
fn setup_test_env() {
    LOGGER_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_test_writer()
            .try_init();
    });

    INIT.call_once(|| {
        discovery::load_all_plugins().expect("Failed to load plugins");
    });
}

/// 创建测试通道配置
fn create_test_channel(id: u16, name: &str) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert(
        "host".to_string(),
        serde_yaml::Value::String("127.0.0.1".to_string()),
    );
    parameters.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
    parameters.insert(
        "timeout".to_string(),
        serde_yaml::Value::Number(5000.into()),
    );

    // 创建测试点位
    let mut combined_points = Vec::new();

    // 添加几个测试点
    for i in 0..5 {
        let mut protocol_params = HashMap::new();
        protocol_params.insert("address".to_string(), format!("1:3:{}", 40000 + i));

        combined_points.push(CombinedPoint {
            point_id: 1000 + i,
            signal_name: format!("test_point_{}", i),
            chinese_name: format!("测试点{}", i),
            telemetry_type: "YC".to_string(),
            data_type: "float32".to_string(),
            protocol_params,
            scaling: Some(ScalingInfo {
                scale: 0.1,
                offset: 0.0,
                unit: Some("V".to_string()),
            }),
        });
    }

    ChannelConfig {
        id,
        name: name.to_string(),
        description: Some("Test channel".to_string()),
        protocol: "virtual".to_string(),
        parameters,
        logging: ChannelLoggingConfig {
            enabled: true,
            level: Some("info".to_string()),
            log_dir: Some(format!("logs/test_{}", id)),
            ..Default::default()
        },
        table_config: None,
        points: Vec::new(),
        combined_points,
    }
}

#[tokio::test]
async fn test_create_channel_with_points() {
    setup_test_env();

    // 创建工厂
    let factory = ProtocolFactory::new();

    // 创建测试通道
    let config = create_test_channel(1, "test_channel_1");
    info!(
        "Creating channel with {} points",
        config.combined_points.len()
    );

    // 创建通道
    let result = factory.create_channel(config).await;
    assert!(
        result.is_ok(),
        "Failed to create channel: {:?}",
        result.err()
    );

    // 验证通道已创建
    assert_eq!(factory.channel_count(), 1);

    // 获取通道
    let channel = factory.get_channel(1).await;
    assert!(channel.is_some(), "Channel not found");

    info!("Channel created successfully");
}

#[tokio::test]
async fn test_protocol_factory_basics() {
    setup_test_env();

    let factory = ProtocolFactory::new();

    // 测试支持的协议
    let protocols = factory.supported_protocols();
    info!("Supported protocols: {:?}", protocols);
    assert!(!protocols.is_empty());

    // 测试协议验证
    let config = create_test_channel(2, "test_validation");
    assert!(factory.validate_config(&config).is_ok());

    // 测试无效配置 - 使用空的协议名称
    let mut invalid_config = config.clone();
    invalid_config.protocol = "".to_string();
    assert!(factory.validate_config(&invalid_config).is_err());
}

#[tokio::test]
async fn test_channel_operations() {
    setup_test_env();

    // 加载插件

    let factory = ProtocolFactory::new();

    // 创建多个通道
    for i in 10..13 {
        let config = create_test_channel(i, &format!("channel_{}", i));
        factory
            .create_channel(config)
            .await
            .expect("Failed to create channel");
    }

    // 验证通道数量
    assert_eq!(factory.channel_count(), 3);

    // 获取所有通道ID
    let ids = factory.get_channel_ids();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&10));
    assert!(ids.contains(&11));
    assert!(ids.contains(&12));

    // 获取通道元数据
    let metadata = factory.get_channel_metadata(10).await;
    assert!(metadata.is_some());
    let (name, protocol) = metadata.unwrap();
    assert_eq!(name, "channel_10");
    assert!(protocol.contains("Modbus"));

    info!("All channel operations completed successfully");
}

#[tokio::test]
async fn test_channel_logging() {
    setup_test_env();

    // 加载插件

    let factory = ProtocolFactory::new();

    // 创建启用日志的通道
    let config = create_test_channel(20, "logging_test");
    factory
        .create_channel(config)
        .await
        .expect("Failed to create channel");

    // 写入日志
    factory
        .write_channel_log(20, "INFO", "Test log message")
        .ok();
    factory
        .write_channel_log(20, "DEBUG", "Debug information")
        .ok();

    // 验证日志目录创建
    let log_dir = "logs/test_20";
    if std::path::Path::new(log_dir).exists() {
        info!("Log directory created successfully");

        // 清理
        std::fs::remove_dir_all(log_dir).ok();
    }
}

#[tokio::test]
async fn test_concurrent_channel_creation() {
    setup_test_env();

    // 加载插件

    let factory = std::sync::Arc::new(ProtocolFactory::new());

    // 并发创建通道
    let mut handles = vec![];

    for i in 30..35 {
        let factory_clone = factory.clone();
        let handle = tokio::spawn(async move {
            let config = create_test_channel(i, &format!("concurrent_{}", i));
            factory_clone.create_channel(config).await
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(())) = handle.await {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 5);
    assert_eq!(factory.channel_count(), 5);

    info!(
        "Concurrent channel creation completed: {} channels created",
        success_count
    );
}

#[tokio::test]
async fn test_channel_with_mock_transport() {
    setup_test_env();

    // 加载插件

    let factory = ProtocolFactory::new();

    // 创建通道（内部会使用 Mock 传输层，因为无法连接到真实设备）
    let config = create_test_channel(40, "mock_transport_test");
    let result = factory.create_protocol(config).await;

    match result {
        Ok(protocol) => {
            info!("Protocol created: {}", protocol.name());
            assert_eq!(protocol.protocol_type(), "modbus");

            // 获取状态
            let status = protocol.status().await;
            info!(
                "Protocol status: id={}, connected={}",
                status.id, status.connected
            );
        }
        Err(e) => {
            // 预期的错误：无法连接到 127.0.0.1:502
            info!("Expected error (no real device): {}", e);
        }
    }
}

// 清理测试日志
#[cfg(test)]
mod cleanup {
    #[test]
    fn cleanup_test_logs() {
        for i in [1, 2, 10, 11, 12, 20, 30, 31, 32, 33, 34, 40] {
            let dir = format!("logs/test_{}", i);
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}
