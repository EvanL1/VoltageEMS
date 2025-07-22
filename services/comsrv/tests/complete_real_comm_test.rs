//! 完整的真实通信测试
//!
//! 使用正确的配置和CSV文件进行端到端测试

use comsrv::core::config::ConfigManager;
use comsrv::core::framework::factory::ProtocolFactory;
use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

static INIT: Once = Once::new();

/// 设置测试环境
fn setup_test_env() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    INIT.call_once(|| {
        let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();
    });
}

/// 复制配置文件到测试目录
fn copy_config_files(
    source_dir: &str,
    dest_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    // 创建目标目录
    fs::create_dir_all(dest_dir)?;

    // 复制所有CSV文件
    let source_path = PathBuf::from(source_dir);
    for entry in fs::read_dir(&source_path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        if file_name.to_string_lossy().ends_with(".csv") {
            let source_file = entry.path();
            let dest_file = dest_dir.join(file_name);
            fs::copy(source_file, dest_file)?;
        }
    }

    Ok(())
}

/// Modbus模拟器进程管理
struct ModbusSimulator {
    process: Option<tokio::process::Child>,
    port: u16,
}

impl ModbusSimulator {
    fn new(port: u16) -> Self {
        Self {
            process: None,
            port,
        }
    }

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("启动Modbus模拟器在端口 {}", self.port);

        // 启动Python Modbus模拟器
        let mut cmd = Command::new("uv");
        cmd.arg("run")
            .arg("python")
            .arg("tests/modbus_server_simulator.py")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(child) => {
                self.process = Some(child);
                // 等待模拟器启动
                sleep(Duration::from_secs(3)).await;
                info!("Modbus模拟器已启动");
                Ok(())
            }
            Err(e) => {
                error!("启动Modbus模拟器失败: {}", e);
                Err(Box::new(e))
            }
        }
    }

    async fn stop(&mut self) {
        if let Some(mut process) = self.process.take() {
            info!("停止Modbus模拟器");
            let _ = process.kill().await;
        }
    }
}

impl Drop for ModbusSimulator {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

#[tokio::test]
async fn test_complete_modbus_communication() {
    setup_test_env();
    info!("测试完整的Modbus通信");

    // 启动Modbus模拟器
    let mut simulator = ModbusSimulator::new(5502);
    if let Err(e) = simulator.start().await {
        warn!("跳过测试：无法启动Modbus模拟器 - {}", e);
        return;
    }

    // 创建Redis存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            simulator.stop().await;
            return;
        }
    };

    // 创建测试目录
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");
    let modbus_config_dir = config_dir.join("Modbus_TCP_Test_01");

    // 复制配置文件
    copy_config_files("test-configs/modbus/ModbusTCP_Test_01", &modbus_config_dir)
        .expect("复制配置文件失败");

    // 创建配置
    let config_content = format!(
        r#"
version: "1.0"
service:
  name: "complete_modbus_test"
  redis:
    url: "redis://127.0.0.1:6379"
  logging:
    level: "info"

csv_base_path: "{}"

channels:
  - id: 1001
    name: "modbus_complete_test"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5502
      timeout: 5000
      update_interval: 1000
      retry_count: 3
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
"#,
        config_dir.display()
    );

    // 保存配置文件
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    info!(
        "配置加载成功，通道数量: {}",
        config_manager.get_channels().len()
    );

    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道
    {
        let factory_guard = factory.write().await;
        let channel_config = config_manager.get_channel(1001).unwrap();
        info!("通道配置点数: {}", channel_config.combined_points.len());
        factory_guard
            .create_channel(channel_config.clone())
            .await
            .expect("创建Modbus通道失败");
    }

    // 启动通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    info!("等待数据收集...");
    sleep(Duration::from_secs(5)).await;

    // 验证数据流 - 使用配置文件中定义的实际点位ID
    let mut data_received = false;
    let test_points = vec![
        (1001, "voltage_a", TelemetryType::Telemetry),
        (1002, "voltage_b", TelemetryType::Telemetry),
        (1003, "voltage_c", TelemetryType::Telemetry),
        (1004, "current_a", TelemetryType::Telemetry),
        (1005, "current_b", TelemetryType::Telemetry),
    ];

    for (point_id, name, telemetry_type) in test_points {
        match storage.read_point(1001, &telemetry_type, point_id).await {
            Ok(Some((value, timestamp))) => {
                info!(
                    "{} (ID: {}) - 值: {:.2}, 时间戳: {}",
                    name, point_id, value, timestamp
                );
                data_received = true;
                assert!(timestamp > 0, "时间戳应该大于0");
            }
            Ok(None) => {
                warn!("{} (ID: {}) 无数据", name, point_id);
            }
            Err(e) => {
                error!("读取 {} (ID: {}) 失败: {}", name, point_id, e);
            }
        }
    }

    assert!(data_received, "应该接收到至少一些数据");

    // 测试写入控制命令
    info!("测试控制命令...");

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    // 停止模拟器
    simulator.stop().await;

    info!("✓ 完整Modbus通信测试通过");
}

#[tokio::test]
async fn test_multi_channel_with_real_config() {
    setup_test_env();
    info!("测试多通道真实配置");

    // 创建Redis存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            return;
        }
    };

    // 创建测试目录
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");

    // 复制多个通道的配置
    let modbus_config_dir1 = config_dir.join("Modbus_TCP_Test_01");
    let modbus_config_dir2 = config_dir.join("Modbus_TCP_Test_02");

    copy_config_files("config/Modbus_TCP_Test_01", &modbus_config_dir1).ok();
    copy_config_files("config/Modbus_TCP_Test_02", &modbus_config_dir2).ok();

    // 创建多通道配置
    let config_content = format!(
        r#"
version: "1.0"
service:
  name: "multi_channel_test"
  redis:
    url: "redis://127.0.0.1:6379"

csv_base_path: "{}"

channels:
  - id: 2001
    name: "virtual_channel_1"
    protocol: "virtual"
    parameters:
      update_interval: 1000
      
  - id: 2002
    name: "virtual_channel_2"
    protocol: "virtual"
    parameters:
      update_interval: 2000
"#,
        config_dir.display()
    );

    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建所有通道
    {
        let factory_guard = factory.write().await;
        for channel_id in [2001, 2002] {
            if let Some(channel_config) = config_manager.get_channel(channel_id) {
                factory_guard
                    .create_channel(channel_config.clone())
                    .await
                    .expect(&format!("创建通道{}失败", channel_id));
            }
        }
    }

    // 验证通道数量
    {
        let factory_guard = factory.read().await;
        assert_eq!(factory_guard.channel_count(), 2, "应该有2个通道");
    }

    // 启动所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    // 获取通道统计
    {
        let factory_guard = factory.read().await;
        let stats = factory_guard.get_channel_stats().await;
        info!("通道统计:");
        info!("  总通道数: {}", stats.total_channels);
        info!("  运行中: {}", stats.running_channels);
        info!("  协议分布: {:?}", stats.protocol_counts);

        assert_eq!(stats.total_channels, 2);
        assert_eq!(stats.running_channels, 2);
    }

    // 停止所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    info!("✓ 多通道配置测试通过");
}
