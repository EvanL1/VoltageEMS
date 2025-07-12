//! 简化的真实通信测试
//!
//! 测试基本的Modbus通信功能

use comsrv::core::config::ConfigManager;
use comsrv::core::framework::factory::ProtocolFactory;
use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn};

static INIT: Once = Once::new();

fn setup_test_env() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    INIT.call_once(|| {
        let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();
    });
}

#[tokio::test]
async fn test_basic_modbus_setup() {
    setup_test_env();
    info!("测试基本Modbus设置");

    // 创建简单配置
    let config_content = r#"
version: "1.0"
service:
  name: "simple_modbus_test"
  redis:
    url: "redis://127.0.0.1:6379"

channels:
  - id: 5001
    name: "simple_modbus_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 502
      timeout: 5000
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道
    {
        let factory_guard = factory.write().await;
        let channel_config = config_manager.get_channel(5001).unwrap();
        factory_guard
            .create_channel(channel_config.clone())
            .await
            .expect("创建通道失败");
    }

    // 验证通道创建
    {
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 1);
        assert!(factory_guard.get_channel(5001).await.is_some());
    }

    info!("✓ 基本Modbus设置测试通过");
}

#[tokio::test]
async fn test_virtual_protocol_communication() {
    setup_test_env();
    info!("测试虚拟协议通信");

    // 创建Redis存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            return;
        }
    };

    // 创建虚拟协议配置
    let config_content = r#"
version: "1.0"
service:
  name: "virtual_protocol_test"
  redis:
    url: "redis://127.0.0.1:6379"

channels:
  - id: 6001
    name: "virtual_test_channel"
    protocol: "virtual"
    parameters:
      update_interval: 100
"#;

    // 创建点表配置
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config").join("Virtual_Test");
    std::fs::create_dir_all(&config_dir).unwrap();

    // 创建遥测点表
    let telemetry_csv = r#"point_id,signal_name,chinese_name,data_type,scale,offset,unit
60001,test_point_1,测试点1,FLOAT,1.0,0.0,unit
60002,test_point_2,测试点2,FLOAT,1.0,0.0,unit
"#;
    std::fs::write(config_dir.join("telemetry.csv"), telemetry_csv).unwrap();

    // 保存配置文件
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道
    {
        let factory_guard = factory.write().await;
        let mut channel_config = config_manager.get_channel(6001).unwrap().clone();

        // 手动添加点位
        channel_config
            .combined_points
            .push(comsrv::core::config::types::CombinedPoint {
                point_id: 60001,
                signal_name: "test_point_1".to_string(),
                chinese_name: "测试点1".to_string(),
                telemetry_type: "Measurement".to_string(),
                data_type: "FLOAT".to_string(),
                protocol_params: std::collections::HashMap::new(),
                scaling: None,
            });

        factory_guard
            .create_channel(channel_config)
            .await
            .expect("创建虚拟通道失败");
    }

    // 启动通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    info!("等待数据生成...");
    sleep(Duration::from_millis(500)).await;

    // 验证数据
    match storage
        .read_point(6001, &TelemetryType::Telemetry, 60001)
        .await
    {
        Ok(Some((value, timestamp))) => {
            info!("虚拟协议数据: 值={}, 时间戳={}", value, timestamp);
            assert!(timestamp > 0, "时间戳应该大于0");
        }
        Ok(None) => {
            warn!("没有数据");
        }
        Err(e) => {
            warn!("读取数据失败: {}", e);
        }
    }

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    info!("✓ 虚拟协议通信测试完成");
}

#[tokio::test]
async fn test_storage_integration() {
    setup_test_env();
    info!("测试存储集成");

    // 创建Redis存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            return;
        }
    };

    // 测试基本读写
    let channel_id = 7001;
    let point_id = 70001;
    let test_value = 42.5;

    // 写入数据
    storage
        .write_point(channel_id, &TelemetryType::Telemetry, point_id, test_value)
        .await
        .expect("写入数据失败");

    // 读取数据
    match storage
        .read_point(channel_id, &TelemetryType::Telemetry, point_id)
        .await
    {
        Ok(Some((value, timestamp))) => {
            info!("读取数据: 值={}, 时间戳={}", value, timestamp);
            assert_eq!(value, test_value, "值应该匹配");
            assert!(timestamp > 0, "时间戳应该大于0");
        }
        Ok(None) => panic!("数据不存在"),
        Err(e) => panic!("读取失败: {}", e),
    }

    // 测试批量写入
    use comsrv::plugins::plugin_storage::PluginPointUpdate;
    let updates = vec![
        PluginPointUpdate {
            channel_id,
            telemetry_type: TelemetryType::Telemetry,
            point_id: 70002,
            value: 10.0,
        },
        PluginPointUpdate {
            channel_id,
            telemetry_type: TelemetryType::Signal,
            point_id: 70003,
            value: 1.0,
        },
    ];

    storage.write_points(updates).await.expect("批量写入失败");

    // 验证批量写入
    match storage
        .read_point(channel_id, &TelemetryType::Telemetry, 70002)
        .await
    {
        Ok(Some((value, _))) => {
            assert_eq!(value, 10.0, "遥测值应该为10.0");
        }
        _ => panic!("批量写入的遥测数据不存在"),
    }

    match storage
        .read_point(channel_id, &TelemetryType::Signal, 70003)
        .await
    {
        Ok(Some((value, _))) => {
            assert_eq!(value, 1.0, "遥信值应该为1.0");
        }
        _ => panic!("批量写入的遥信数据不存在"),
    }

    info!("✓ 存储集成测试通过");
}
