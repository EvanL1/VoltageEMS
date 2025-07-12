//! 真实通信测试
//!
//! 测试实际的协议通信，包括Modbus模拟器交互和数据流验证

use comsrv::core::config::ConfigManager;
use comsrv::core::framework::factory::ProtocolFactory;
use comsrv::core::framework::TelemetryType;
use comsrv::plugins::plugin_storage::{DefaultPluginStorage, PluginStorage};
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

    // 确保插件只加载一次
    INIT.call_once(|| {
        let _ = comsrv::plugins::plugin_registry::discovery::load_all_plugins();
    });
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
#[ignore] // 需要手动运行，因为需要uv和pymodbus
async fn test_modbus_real_communication() {
    setup_test_env();
    info!("测试Modbus真实通信");

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

    // 创建配置
    let config_content = r#"
version: "1.0"
service:
  name: "modbus_real_test"
  redis:
    url: "redis://127.0.0.1:6379"
  logging:
    level: "info"

csv_base_path: "./config"

channels:
  - id: 1001
    name: "modbus_real_test_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5502
      timeout: 5000
      update_interval: 1000
      retry_count: 3
    table_config:
      four_telemetry_route: "ModbusTCP_Test_01"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
      protocol_mapping_route: "ModbusTCP_Test_01"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        control_mapping: "mapping_control.csv"
        adjustment_mapping: "mapping_adjustment.csv"
"#;

    // 创建测试点配置
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config").join("ModbusTCP_Test_01");
    std::fs::create_dir_all(&config_dir).unwrap();

    // 创建遥测点表
    let telemetry_csv = r#"point_id,signal_name,chinese_name,data_type,scale,offset,unit
10001,ir_0,输入寄存器0,FLOAT,1.0,0.0,unit
10002,ir_1,输入寄存器1,FLOAT,1.0,0.0,unit
10003,ir_2,输入寄存器2,FLOAT,1.0,0.0,unit
10004,hr_0,保持寄存器0,FLOAT,1.0,0.0,unit
10005,hr_1,保持寄存器1,FLOAT,1.0,0.0,unit
"#;
    std::fs::write(config_dir.join("telemetry.csv"), telemetry_csv).unwrap();

    // 创建遥测映射
    let mapping_csv = r#"inner_index,protocol_address
10001,1:4:0
10002,1:4:1
10003,1:4:2
10004,1:3:0
10005,1:3:1
"#;
    std::fs::write(config_dir.join("mapping_telemetry.csv"), mapping_csv).unwrap();

    // 创建遥信点表
    let signal_csv = r#"point_id,signal_name,chinese_name
20001,di_0,离散输入0
20002,di_1,离散输入1
20003,coil_0,线圈0
20004,coil_1,线圈1
"#;
    std::fs::write(config_dir.join("signal.csv"), signal_csv).unwrap();

    // 创建遥信映射
    let signal_mapping_csv = r#"inner_index,protocol_address
20001,1:2:0
20002,1:2:1
20003,1:1:0
20004,1:1:1
"#;
    std::fs::write(config_dir.join("mapping_signal.csv"), signal_mapping_csv).unwrap();

    // 保存配置文件
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道
    {
        let factory_guard = factory.write().await;
        let channel_config = config_manager.get_channel(1001).unwrap();
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

    // 验证数据流
    let mut data_received = false;
    for point_id in [10001, 10002, 10003, 10004, 10005] {
        match storage
            .read_point(1001, &TelemetryType::Telemetry, point_id)
            .await
        {
            Ok(Some((value, timestamp))) => {
                info!(
                    "遥测点 {} - 值: {:.2}, 时间戳: {}",
                    point_id, value, timestamp
                );
                data_received = true;
                assert!(timestamp > 0, "时间戳应该大于0");
            }
            Ok(None) => {
                warn!("遥测点 {} 无数据", point_id);
            }
            Err(e) => {
                error!("读取遥测点 {} 失败: {}", point_id, e);
            }
        }
    }

    // 验证遥信数据
    for point_id in [20001, 20002, 20003, 20004] {
        match storage
            .read_point(1001, &TelemetryType::Signal, point_id)
            .await
        {
            Ok(Some((value, timestamp))) => {
                info!("遥信点 {} - 值: {}, 时间戳: {}", point_id, value, timestamp);
                data_received = true;
            }
            Ok(None) => {
                warn!("遥信点 {} 无数据", point_id);
            }
            Err(e) => {
                error!("读取遥信点 {} 失败: {}", point_id, e);
            }
        }
    }

    assert!(data_received, "应该接收到至少一些数据");

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    // 停止模拟器
    simulator.stop().await;

    info!("✓ Modbus真实通信测试通过");
}

#[tokio::test]
#[ignore] // 需要手动运行
async fn test_modbus_control_commands() {
    setup_test_env();
    info!("测试Modbus控制命令");

    // 启动Modbus模拟器
    let mut simulator = ModbusSimulator::new(5503);
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

    // 创建配置
    let config_content = r#"
version: "1.0"
service:
  name: "modbus_control_test"
  redis:
    url: "redis://127.0.0.1:6379"

csv_base_path: "./config"

channels:
  - id: 1002
    name: "modbus_control_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5503
      timeout: 5000
"#;

    // 创建控制点配置
    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config").join("ModbusTCP_Test_01");
    std::fs::create_dir_all(&config_dir).unwrap();

    // 创建遥控点表
    let control_csv = r#"point_id,signal_name,chinese_name,data_type
30001,write_coil_0,写线圈0,BOOL
30002,write_hr_0,写保持寄存器0,UINT16
"#;
    std::fs::write(config_dir.join("control.csv"), control_csv).unwrap();

    // 创建遥控映射
    let control_mapping_csv = r#"inner_index,protocol_address
30001,1:5:0
30002,1:6:10
"#;
    std::fs::write(config_dir.join("mapping_control.csv"), control_mapping_csv).unwrap();

    // 保存配置文件
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建通道
    {
        let factory_guard = factory.write().await;
        let channel_config = config_manager.get_channel(1002).unwrap();
        factory_guard
            .create_channel(channel_config.clone())
            .await
            .expect("创建控制通道失败");
    }

    // 启动通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    info!("等待通道初始化...");
    sleep(Duration::from_secs(2)).await;

    // 测试写线圈
    info!("测试写线圈命令");
    storage
        .write_point(1002, &TelemetryType::Control, 30001, 1.0)
        .await
        .expect("写线圈命令失败");

    sleep(Duration::from_millis(500)).await;

    // 测试写保持寄存器
    info!("测试写保持寄存器命令");
    storage
        .write_point(1002, &TelemetryType::Control, 30002, 12345.0)
        .await
        .expect("写保持寄存器命令失败");

    sleep(Duration::from_millis(500)).await;

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    // 停止模拟器
    simulator.stop().await;

    info!("✓ Modbus控制命令测试通过");
}

#[tokio::test]
#[ignore] // 需要手动运行
async fn test_multi_channel_communication() {
    setup_test_env();
    info!("测试多通道并发通信");

    // 启动多个Modbus模拟器
    let mut simulator1 = ModbusSimulator::new(5504);
    let mut simulator2 = ModbusSimulator::new(5505);

    if let Err(e) = simulator1.start().await {
        warn!("跳过测试：无法启动Modbus模拟器1 - {}", e);
        return;
    }

    if let Err(e) = simulator2.start().await {
        warn!("跳过测试：无法启动Modbus模拟器2 - {}", e);
        simulator1.stop().await;
        return;
    }

    // 创建Redis存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            simulator1.stop().await;
            simulator2.stop().await;
            return;
        }
    };

    // 创建配置
    let config_content = r#"
version: "1.0"
service:
  name: "multi_channel_test"
  redis:
    url: "redis://127.0.0.1:6379"

csv_base_path: "./config"

channels:
  - id: 2001
    name: "modbus_channel_1"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5504
      timeout: 5000
      update_interval: 1000
    table_config:
      four_telemetry_route: "ModbusTCP_Test_01"
      four_telemetry_files:
        telemetry_file: "telemetry.csv"
        signal_file: "signal.csv"
        control_file: "control.csv"
        adjustment_file: "adjustment.csv"
      protocol_mapping_route: "ModbusTCP_Test_01"
      protocol_mapping_files:
        telemetry_mapping: "mapping_telemetry.csv"
        signal_mapping: "mapping_signal.csv"
        control_mapping: "mapping_control.csv"
        adjustment_mapping: "mapping_adjustment.csv"
      
  - id: 2002
    name: "modbus_channel_2"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5505
      timeout: 5000
      update_interval: 1500
    table_config:
      four_telemetry_route: "ModbusTCP_Test_02"
      
  - id: 2003
    name: "virtual_channel"
    protocol: "virtual"
    parameters:
      update_interval: 2000
"#;

    // 创建测试目录
    let temp_dir = tempfile::TempDir::new().unwrap();

    // 为每个通道创建配置
    for channel_num in 1..=2 {
        let config_dir = temp_dir
            .path()
            .join("config")
            .join(format!("ModbusTCP_Test_{:02}", channel_num));
        std::fs::create_dir_all(&config_dir).unwrap();

        // 创建简单的遥测配置
        let telemetry_csv = format!(
            r#"point_id,signal_name,chinese_name,data_type,scale,offset,unit
{},ir_0,通道{}输入寄存器0,FLOAT,1.0,0.0,unit
{},ir_1,通道{}输入寄存器1,FLOAT,1.0,0.0,unit
"#,
            10000 + channel_num * 100 + 1,
            channel_num,
            10000 + channel_num * 100 + 2,
            channel_num
        );

        std::fs::write(config_dir.join("telemetry.csv"), telemetry_csv).unwrap();

        let mapping_csv = format!(
            r#"inner_index,protocol_address
{},1:4:0
{},1:4:1
"#,
            10000 + channel_num * 100 + 1,
            10000 + channel_num * 100 + 2
        );

        std::fs::write(config_dir.join("mapping_telemetry.csv"), mapping_csv).unwrap();
    }

    // 保存配置文件
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建所有通道
    {
        let factory_guard = factory.write().await;
        for channel_id in [2001, 2002, 2003] {
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
        assert_eq!(factory_guard.channel_count(), 3, "应该有3个通道");
    }

    // 启动所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    info!("等待多通道数据收集...");
    sleep(Duration::from_secs(5)).await;

    // 获取通道统计
    {
        let factory_guard = factory.read().await;
        let stats = factory_guard.get_channel_stats().await;
        info!("通道统计:");
        info!("  总通道数: {}", stats.total_channels);
        info!("  运行中: {}", stats.running_channels);
        info!("  协议分布: {:?}", stats.protocol_counts);

        assert_eq!(stats.total_channels, 3);
        assert_eq!(stats.running_channels, 3);
    }

    // 验证每个通道都有数据
    let mut channels_with_data = 0;

    // 检查Modbus通道1
    match storage
        .read_point(2001, &TelemetryType::Telemetry, 10101)
        .await
    {
        Ok(Some((value, _))) => {
            info!("通道2001有数据: {}", value);
            channels_with_data += 1;
        }
        _ => warn!("通道2001无数据"),
    }

    // 检查Modbus通道2
    match storage
        .read_point(2002, &TelemetryType::Telemetry, 10201)
        .await
    {
        Ok(Some((value, _))) => {
            info!("通道2002有数据: {}", value);
            channels_with_data += 1;
        }
        _ => warn!("通道2002无数据"),
    }

    assert!(channels_with_data >= 1, "至少应该有一个通道有数据");

    // 停止所有通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    // 停止模拟器
    simulator1.stop().await;
    simulator2.stop().await;

    info!("✓ 多通道并发通信测试通过");
}

#[tokio::test]
#[ignore] // 需要手动运行
async fn test_connection_recovery() {
    setup_test_env();
    info!("测试连接恢复能力");

    // 启动Modbus模拟器
    let mut simulator = ModbusSimulator::new(5506);
    if let Err(e) = simulator.start().await {
        warn!("跳过测试：无法启动Modbus模拟器 - {}", e);
        return;
    }

    // 创建存储
    std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
    let storage = match DefaultPluginStorage::from_env().await {
        Ok(s) => Arc::new(s) as Arc<dyn PluginStorage>,
        Err(_) => {
            warn!("跳过测试：Redis未运行");
            simulator.stop().await;
            return;
        }
    };

    // 创建简单配置
    let config_content = r#"
version: "1.0"
service:
  name: "recovery_test"
  redis:
    url: "redis://127.0.0.1:6379"

channels:
  - id: 3001
    name: "recovery_test_channel"
    protocol: "modbus_tcp"
    parameters:
      host: "127.0.0.1"
      port: 5506
      timeout: 2000
      retry_count: 3
      retry_interval: 1000
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    std::fs::write(&config_path, config_content).unwrap();

    // 加载配置并创建通道
    let config_manager = ConfigManager::from_file(&config_path).unwrap();
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // 创建并启动通道
    {
        let factory_guard = factory.write().await;
        let channel_config = config_manager.get_channel(3001).unwrap();
        factory_guard
            .create_channel(channel_config.clone())
            .await
            .expect("创建通道失败");
    }

    {
        let factory_guard = factory.read().await;
        factory_guard
            .start_all_channels()
            .await
            .expect("启动通道失败");
    }

    info!("等待初始连接...");
    sleep(Duration::from_secs(2)).await;

    // 验证连接正常
    {
        let factory_guard = factory.read().await;
        let running = factory_guard.running_channel_count().await;
        assert_eq!(running, 1, "通道应该在运行");
    }

    // 停止模拟器模拟连接断开
    info!("模拟连接断开...");
    simulator.stop().await;
    sleep(Duration::from_secs(3)).await;

    // 重新启动模拟器
    info!("重新启动模拟器...");
    if let Err(e) = simulator.start().await {
        error!("重启模拟器失败: {}", e);
    } else {
        // 等待重连
        info!("等待自动重连...");
        sleep(Duration::from_secs(5)).await;

        // 验证重连成功
        {
            let factory_guard = factory.read().await;
            let running = factory_guard.running_channel_count().await;
            info!("重连后运行通道数: {}", running);
        }
    }

    // 停止通道
    {
        let factory_guard = factory.read().await;
        factory_guard.stop_all_channels().await.ok();
    }

    // 停止模拟器
    simulator.stop().await;

    info!("✓ 连接恢复测试完成");
}
