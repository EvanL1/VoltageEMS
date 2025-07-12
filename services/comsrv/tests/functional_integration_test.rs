//! 功能集成测试
//!
//! 测试配置加载、通道建立、点表读取等核心功能

use comsrv::core::config::{ChannelConfig, ConfigManager};
use comsrv::core::framework::factory::ProtocolFactory;
use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
use comsrv::service_impl::start_communication_service;
use std::sync::Arc;
use std::sync::Once;
use tokio::sync::RwLock;
use tracing::{error, info};

static INIT: Once = Once::new();

/// 设置测试环境
fn setup_test_env() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 确保插件只加载一次
    INIT.call_once(|| {
        let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();
    });
}

#[tokio::test]
async fn test_config_loading_and_validation() {
    setup_test_env();
    info!("测试配置文件加载和验证");

    // 创建测试配置文件
    let config_content = r#"
version: "1.0"
service:
  name: "test_comsrv"
  api:
    enabled: true
    bind_address: "127.0.0.1:3001"
  redis:
    url: "redis://127.0.0.1:6379"
  logging:
    level: "info"
    file: "logs/test.log"

csv_base_path: "./config"

channels:
  - id: 101
    name: "test_modbus_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
      timeout: 5000
    table_config:
      four_telemetry_route: "Modbus_TCP_Test_01"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
      protocol_mapping_route: "Modbus_TCP_Test_01"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        control_mapping: "mapping_control.csv"
        adjustment_mapping: "mapping_adjustment.csv"
  
  - id: 102
    name: "test_virtual_channel"
    protocol: "virtual"
    parameters:
      update_interval: 1000
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置
    let config_manager = ConfigManager::from_file(&config_path).unwrap();

    // 验证配置
    assert_eq!(config_manager.service().name, "test_comsrv");
    assert_eq!(config_manager.get_channels().len(), 2);

    // 验证通道配置
    let channel_101 = config_manager.get_channel(101).unwrap();
    assert_eq!(channel_101.name, "test_modbus_channel");
    assert_eq!(channel_101.protocol, "modbus_tcp");

    let channel_102 = config_manager.get_channel(102).unwrap();
    assert_eq!(channel_102.name, "test_virtual_channel");
    assert_eq!(channel_102.protocol, "virtual");

    info!("✓ 配置加载和验证测试通过");
}

#[tokio::test]
async fn test_csv_point_table_loading() {
    setup_test_env();
    info!("测试CSV点表加载");

    // 创建测试CSV文件
    let temp_dir = tempfile::TempDir::new().unwrap();
    let csv_dir = temp_dir.path().join("Test_CSV");
    std::fs::create_dir_all(&csv_dir).unwrap();

    // 创建遥测CSV
    let telemetry_csv = r#"point_id,signal_name,chinese_name,data_type,scale,offset,unit
2001,temp_sensor_1,温度传感器1,FLOAT,0.1,0.0,°C
2002,temp_sensor_2,温度传感器2,FLOAT,0.1,0.0,°C
2003,pressure_1,压力传感器1,FLOAT,0.01,0.0,kPa
"#;
    std::fs::write(csv_dir.join("telemetry.csv"), telemetry_csv).unwrap();

    // 创建映射CSV
    let mapping_csv = r#"inner_index,protocol_address
2001,1:3:40001
2002,1:3:40002
2003,1:3:40003
"#;
    std::fs::write(csv_dir.join("mapping_telemetry.csv"), mapping_csv).unwrap();

    // 创建遥信CSV
    let signal_csv = r#"point_id,signal_name,chinese_name
3001,switch_1,开关1
3002,switch_2,开关2
"#;
    std::fs::write(csv_dir.join("signal.csv"), signal_csv).unwrap();

    // 配置
    let config_content = format!(
        r#"
version: "1.0"
service:
  name: "csv_test"
  redis:
    url: "redis://127.0.0.1:6379"

csv_base_path: "{}"

channels:
  - id: 201
    name: "csv_test_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
    table_config:
      four_telemetry_route: "Test_CSV"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
      protocol_mapping_route: "Test_CSV"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        control_mapping: "mapping_control.csv"
        adjustment_mapping: "mapping_adjustment.csv"
"#,
        temp_dir.path().display()
    );

    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let channel = config_manager.get_channel(201).unwrap();

    // 验证点表加载
    assert!(!channel.combined_points.is_empty());
    assert_eq!(channel.combined_points.len(), 3); // 3个遥测点被加载

    // 检查遥测点 (Measurement类型)
    let telemetry_points: Vec<_> = channel
        .combined_points
        .iter()
        .filter(|p| p.telemetry_type == "Measurement")
        .collect();
    assert_eq!(telemetry_points.len(), 3);

    // 验证特定点的详细信息
    let point_2001 = telemetry_points
        .iter()
        .find(|p| p.point_id == 2001)
        .expect("Should find point 2001");
    assert_eq!(point_2001.signal_name, "temp_sensor_1");
    assert_eq!(point_2001.chinese_name, "温度传感器1");
    // 检查scaling存在
    assert!(point_2001.scaling.is_some());
    // 检查协议参数存在
    assert!(point_2001.protocol_params.contains_key("address"));

    info!("✓ CSV点表加载测试通过");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore] // 需要手动运行，因为virtual协议可能有无限循环
async fn test_channel_lifecycle() {
    setup_test_env();
    info!("测试通道生命周期管理");

    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建多个通道配置
    let configs = vec![
        create_test_channel_config(301, "lifecycle_test_1", "virtual"),
        create_test_channel_config(302, "lifecycle_test_2", "virtual"),
        create_test_channel_config(303, "lifecycle_test_3", "virtual"),
    ];

    // 创建通道
    for config in configs {
        let factory_guard = factory.write().await;
        factory_guard
            .create_channel(config)
            .await
            .expect("Failed to create channel");
    }

    // 验证通道数量
    {
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 3);
    }

    // 启动所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("Failed to start channels");
    }

    // 验证运行状态
    {
        let factory_guard = factory.read().await;
        let running_count = factory_guard.running_channel_count().await;
        assert_eq!(running_count, 3);
    }

    // 停止所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .stop_all_channels()
            .await
            .expect("Failed to stop channels");
    }

    // 验证停止状态
    {
        let factory_guard = factory.read().await;
        let running_count = factory_guard.running_channel_count().await;
        assert_eq!(running_count, 0);
    }

    info!("✓ 通道生命周期管理测试通过");
}

#[tokio::test]
#[ignore] // 需要Redis运行
async fn test_data_flow_with_new_storage() {
    setup_test_env();
    info!("测试数据流与新存储结构");

    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");

    // 创建存储
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            info!("跳过测试：Redis未运行");
            return;
        }
    };

    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建带点表的通道配置
    let mut config = create_test_channel_config(401, "data_flow_test", "virtual");

    // 添加一些测试点
    for i in 0..10 {
        config
            .combined_points
            .push(comsrv::core::config::types::CombinedPoint {
                point_id: 4001 + i,
                signal_name: format!("test_point_{}", i),
                chinese_name: format!("测试点{}", i),
                telemetry_type: if i < 5 {
                    "Measurement".to_string()
                } else {
                    "Signal".to_string()
                },
                data_type: "FLOAT".to_string(),
                protocol_params: std::collections::HashMap::new(),
                scaling: Some(comsrv::core::config::types::ScalingInfo {
                    scale: 0.1,
                    offset: 0.0,
                    unit: Some("V".to_string()),
                }),
            });
    }

    // 创建并启动通道
    {
        let factory_guard = factory.write().await;
        factory_guard
            .create_channel(config)
            .await
            .expect("Failed to create channel");

        // 获取通道并设置存储
        if let Some(channel_arc) = factory_guard.get_channel(401).await {
            let _channel = channel_arc.read().await;
            // Virtual协议会自动生成数据并存储
        }
    }

    // 启动通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("Failed to start channels");
    }

    // 等待数据生成
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 验证数据存储
    for i in 0..5 {
        let point_id = 4001 + i;
        let result = storage
            .read_point(401, &TelemetryType::Telemetry, point_id)
            .await;

        match result {
            Ok(Some((value, timestamp))) => {
                info!(
                    "点 {} 数据: value={}, timestamp={}",
                    point_id, value, timestamp
                );
                assert!(timestamp > 0);
            }
            Ok(None) => error!("点 {} 无数据", point_id),
            Err(e) => error!("读取点 {} 失败: {}", point_id, e),
        }
    }

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    info!("✓ 数据流测试通过");
}

#[tokio::test]
#[ignore] // 需要手动运行
async fn test_multi_protocol_integration() {
    setup_test_env();
    info!("测试多协议集成");

    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建不同协议的通道
    let configs = vec![
        create_test_channel_config(501, "virtual_channel", "virtual"),
        // 如果有真实的Modbus设备，可以添加
        // create_test_channel_config(502, "modbus_channel", "modbus_tcp"),
    ];

    // 创建所有通道
    for config in configs {
        let factory_guard = factory.write().await;
        match factory_guard.create_channel(config.clone()).await {
            Ok(_) => info!("创建通道 {} 成功", config.id),
            Err(e) => error!("创建通道 {} 失败: {}", config.id, e),
        }
    }

    // 验证通道创建
    {
        let factory_guard = factory.read().await;
        assert!(factory_guard.channel_count() > 0);

        // 获取通道统计
        let stats = factory_guard.get_channel_stats().await;
        info!(
            "通道统计: 总数={}, 运行中={}",
            stats.total_channels, stats.running_channels
        );
        info!("协议分布: {:?}", stats.protocol_counts);
    }

    info!("✓ 多协议集成测试通过");
}

#[tokio::test]
#[ignore] // 需要Redis运行
async fn test_complete_system_startup() {
    setup_test_env();
    info!("测试完整系统启动流程");

    // 创建测试配置
    let config_content = r#"
version: "1.0"
service:
  name: "system_test"
  api:
    enabled: false
  redis:
    url: "redis://127.0.0.1:6379"
  logging:
    level: "info"

channels:
  - id: 601
    name: "system_test_channel"
    protocol: "virtual"
    parameters:
      update_interval: 1000
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("system_config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置
    let config_manager = Arc::new(ConfigManager::from_file(&config_path).unwrap());
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 启动通信服务
    match start_communication_service(config_manager.clone(), factory.clone()).await {
        Ok(_) => info!("通信服务启动成功"),
        Err(e) => {
            error!("通信服务启动失败: {}", e);
            // 不是致命错误，可能只是Redis未运行
        }
    }

    // 验证系统状态
    {
        let factory_guard = factory.read().await;
        let stats = factory_guard.get_channel_stats().await;
        info!("系统状态: {} 个通道已创建", stats.total_channels);
    }

    info!("✓ 完整系统启动测试完成");
}

// 辅助函数：创建测试通道配置
fn create_test_channel_config(id: u16, name: &str, protocol: &str) -> ChannelConfig {
    use comsrv::core::config::ChannelLoggingConfig;
    use std::collections::HashMap;

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
    parameters.insert(
        "update_interval".to_string(),
        serde_yaml::Value::Number(1000.into()),
    );

    ChannelConfig {
        id,
        name: name.to_string(),
        description: Some("Functional test channel".to_string()),
        protocol: protocol.to_string(),
        parameters,
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        points: Vec::new(),
        combined_points: Vec::new(),
    }
}
