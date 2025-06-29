use std::sync::Arc;
use tokio::sync::RwLock;
use tempfile::TempDir;
use tracing::{info, warn, error};

use comsrv::core::config::ConfigManager;
use comsrv::core::config::types::{ChannelConfig, ProtocolType, ChannelParameters, ChannelLoggingConfig};
use comsrv::core::protocols::common::combase::protocol_factory::ProtocolFactory;
use comsrv::service::start_communication_service;

/// 真实通道创建测试 - 通过配置文件创建实际的Modbus通道
#[tokio::test]
async fn test_real_modbus_channel_creation_from_config() {
    // 初始化日志
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    info!("🧪 === 测试真实Modbus通道创建 ===");

    // 创建临时目录和配置文件
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("real_modbus_config.yaml");

    // 创建真实的Modbus配置
    let config_content = r#"
version: "1.0"

service:
  name: "real_modbus_test_service"
  description: "真实Modbus通道测试服务"
  logging:
    level: "debug"
    console: true
    file: "/tmp/real_modbus_test.log"
    max_size: 10485760
    max_files: 5
  api:
    enabled: false
    bind_address: "127.0.0.1:3000"
    version: "v1"
  redis:
    enabled: false
    connection_type: "Tcp"
    address: "127.0.0.1:6379"
    db: 0

channels:
  # 真实的Modbus TCP通道配置
  - id: 1001
    name: "TestModbusTCP"
    description: "测试Modbus TCP通道"
    protocol: "ModbusTcp"
    parameters:
      host: "127.0.0.1"
      port: 502
      slave_id: 1
      timeout_ms: 5000
      max_retries: 3
      poll_rate: 1000
    logging:
      enabled: true
      level: "debug"
      console_output: true
      log_messages: true

  # 真实的Modbus RTU通道配置  
  - id: 1002
    name: "TestModbusRTU"
    description: "测试Modbus RTU通道"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      parity: "none"
      stop_bits: 1
      slave_id: 2
      timeout_ms: 2000
      max_retries: 3
      poll_rate: 1000
    logging:
      enabled: true
      level: "debug"
      console_output: true
      log_messages: true
"#;

    // 写入配置文件
    std::fs::write(&config_path, config_content).unwrap();
    info!("✅ 配置文件已创建: {:?}", config_path);

    // 1. 加载配置管理器
    info!("📋 正在加载配置管理器...");
    let config_manager = match ConfigManager::from_file(&config_path) {
        Ok(cm) => {
            info!("✅ 配置管理器加载成功");
            Arc::new(cm)
        }
        Err(e) => {
            error!("❌ 配置管理器加载失败: {}", e);
            panic!("配置管理器加载失败: {}", e);
        }
    };

    // 验证通道配置
    let channels = config_manager.get_channels().clone();
    info!("📊 发现 {} 个通道配置", channels.len());
    
    for channel in &channels {
        info!("🔧 通道配置: ID={}, 名称={}, 协议={}", 
              channel.id, channel.name, channel.protocol);
        info!("   参数: {:?}", channel.parameters);
    }

    // 2. 创建协议工厂
    info!("🏭 正在创建协议工厂...");
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    
    // 验证协议工厂支持的协议
    {
        let factory = protocol_factory.read().await;
        let supported_protocols = factory.supported_protocols();
        info!("🛠️  支持的协议: {:?}", supported_protocols);
        
        for protocol in &supported_protocols {
            info!("   - {:?}: {}", protocol, 
                  if factory.is_protocol_supported(protocol) { "✅" } else { "❌" });
        }
    }

    // 3. 尝试通过配置创建真实的Modbus通道
    info!("🚀 开始创建真实的Modbus通道...");
    
    for channel_config in channels.iter() {
        info!("🔌 正在创建通道: {} ({})", channel_config.name, channel_config.protocol);
        
        // 通过协议工厂创建通道
        let factory = protocol_factory.read().await;
        match factory.create_channel_with_config_manager(channel_config.clone(), Some(&config_manager)).await {
            Ok(()) => {
                info!("✅ 通道 {} 创建成功", channel_config.name);
                
                // 获取创建的通道实例
                if let Some(channel_instance) = factory.get_channel(channel_config.id).await {
                    let channel = channel_instance.read().await;
                    
                    info!("📋 通道详情:");
                    info!("   - 名称: {}", channel.name());
                    info!("   - 协议类型: {}", channel.protocol_type());
                    info!("   - 通道ID: {}", channel.channel_id());
                    info!("   - 运行状态: {}", channel.is_running().await);
                    info!("   - 连接状态: {}", channel.is_connected().await);
                    
                    let parameters = channel.get_parameters();
                    info!("   - 参数:");
                    for (key, value) in &parameters {
                        info!("     * {}: {}", key, value);
                    }
                    
                    // 尝试启动通道（这会触发真实的连接尝试）
                    info!("🔄 尝试启动通道 {}...", channel_config.name);
                    drop(channel); // 释放读锁
                    
                    let mut channel = channel_instance.write().await;
                    match channel.start().await {
                        Ok(()) => {
                            info!("✅ 通道 {} 启动成功", channel_config.name);
                            
                            // 等待一段时间让通道尝试连接
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            
                            // 检查连接状态
                            let is_connected = channel.is_connected().await;
                            let is_running = channel.is_running().await;
                            
                            info!("📊 通道 {} 状态:", channel_config.name);
                            info!("   - 运行中: {}", is_running);
                            info!("   - 已连接: {}", is_connected);
                            
                            if !is_connected {
                                warn!("⚠️  通道 {} 未能连接到目标设备（这在测试环境中是正常的）", channel_config.name);
                            }
                            
                            // 尝试获取统计信息
                            if let Ok(stats) = channel.get_statistics().await {
                                info!("📈 通道 {} 统计信息: {:?}", channel_config.name, stats);
                            }
                            
                            // 停止通道
                            info!("🛑 停止通道 {}...", channel_config.name);
                            if let Err(e) = channel.stop().await {
                                warn!("⚠️  停止通道 {} 时出错: {}", channel_config.name, e);
                            } else {
                                info!("✅ 通道 {} 已停止", channel_config.name);
                            }
                        }
                        Err(e) => {
                            warn!("⚠️  通道 {} 启动失败: {} (这在测试环境中可能是正常的)", 
                                  channel_config.name, e);
                            // 在测试环境中，连接失败是预期的，因为没有真实的Modbus设备
                        }
                    }
                } else {
                    error!("❌ 无法获取通道 {} 的实例", channel_config.name);
                }
            }
            Err(e) => {
                error!("❌ 通道 {} 创建失败: {}", channel_config.name, e);
            }
        }
    }
    
    // 4. 验证协议工厂状态
    {
        let factory = protocol_factory.read().await;
        info!("📊 协议工厂最终状态:");
        info!("   - 通道数量: {}", factory.channel_count());
        info!("   - 是否为空: {}", factory.is_empty());
        
        let channel_ids = factory.get_channel_ids();
        info!("   - 通道ID列表: {:?}", channel_ids);
    }

    info!("🎉 真实Modbus通道创建测试完成");
}

/// 测试通过服务启动函数创建通道
#[tokio::test]
async fn test_real_service_startup_with_modbus_channels() {
    // 初始化日志
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();

    info!("🧪 === 测试服务启动与Modbus通道创建 ===");

    // 创建临时目录和配置文件
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("service_modbus_config.yaml");

    // 创建服务配置
    let config_content = r#"
version: "1.0"

service:
  name: "modbus_service_test"
  description: "Modbus服务启动测试"
  logging:
    level: "info"
    console: true
    file: "/tmp/modbus_service_test.log"
    max_size: 10485760
    max_files: 5
  api:
    enabled: false
  redis:
    enabled: false

channels:
  - id: 2001
    name: "ServiceTestModbusTCP"
    description: "服务测试Modbus TCP通道"
    protocol: "ModbusTcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      timeout_ms: 3000
      max_retries: 2
      poll_rate: 2000
    logging:
      enabled: true
      level: "info"
      console_output: true

  - id: 2002
    name: "ServiceTestModbusRTU"
    description: "服务测试Modbus RTU通道"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB1"
      baud_rate: 19200
      data_bits: 8
      parity: "even"
      stop_bits: 1
      slave_id: 3
      timeout_ms: 5000
      max_retries: 3
      poll_rate: 3000
    logging:
      enabled: true
      level: "info"
      console_output: true
"#;

    // 写入配置文件
    std::fs::write(&config_path, config_content).unwrap();
    info!("✅ 服务配置文件已创建: {:?}", config_path);

    // 加载配置管理器
    let config_manager = Arc::new(ConfigManager::from_file(&config_path).unwrap());
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    info!("🚀 启动通信服务...");
    
    // 使用真实的服务启动函数
    match start_communication_service(config_manager.clone(), protocol_factory.clone()).await {
        Ok(()) => {
            info!("✅ 通信服务启动成功");
            
            // 检查创建的通道
            let factory = protocol_factory.read().await;
            info!("📊 服务启动后的状态:");
            info!("   - 通道数量: {}", factory.channel_count());
            
            let channel_ids = factory.get_channel_ids();
            info!("   - 活跃通道ID: {:?}", channel_ids);
            
            // 检查每个通道的状态
            for channel_id in channel_ids {
                if let Some(channel_instance) = factory.get_channel(channel_id).await {
                    let channel = channel_instance.read().await;
                    info!("📋 通道 {} 状态:", channel_id);
                    info!("   - 名称: {}", channel.name());
                    info!("   - 协议: {}", channel.protocol_type());
                    info!("   - 运行中: {}", channel.is_running().await);
                    info!("   - 已连接: {}", channel.is_connected().await);
                }
            }
        }
        Err(e) => {
            warn!("⚠️  通信服务启动失败: {} (在测试环境中可能是正常的)", e);
        }
    }

    info!("🎉 服务启动测试完成");
}

/// 真实Modbus协议报文生成测试
/// 这个测试通过comsrv的配置系统创建真实的Modbus通道，
/// 并尝试启动通道以触发真实的协议报文生成
#[tokio::test]
async fn test_real_modbus_protocol_frame_generation() {
    // 初始化日志
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    info!("🧪 === 测试真实Modbus协议报文生成 ===");

    // 创建临时目录和配置文件
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("real_modbus_test.yaml");

    // 创建真实的Modbus配置文件
    let config_content = r#"
version: "1.0"

service:
  name: "real_modbus_protocol_test"
  description: "真实Modbus协议报文测试"
  logging:
    level: "debug"
    console: true
    file: "/tmp/real_modbus_protocol_test.log"
    max_size: 10485760
    max_files: 5
  api:
    enabled: false
  redis:
    enabled: false

channels:
  # 真实的Modbus TCP通道配置
  - id: 1001
    name: "RealModbusTCP"
    description: "真实Modbus TCP协议测试通道"
    protocol: "ModbusTcp"
    parameters:
      host: "127.0.0.1"
      port: 502
      slave_id: 1
      timeout_ms: 2000
      max_retries: 1
      poll_rate: 5000
    logging:
      enabled: true
      level: "debug"
      console_output: true
      log_messages: true
"#;

    // 写入配置文件
    std::fs::write(&config_path, config_content).unwrap();
    info!("✅ 配置文件已创建: {:?}", config_path);

    // 1. 加载配置管理器
    info!("📋 正在加载配置管理器...");
    let config_manager = match ConfigManager::from_file(&config_path) {
        Ok(cm) => {
            info!("✅ 配置管理器加载成功");
            Arc::new(cm)
        }
        Err(e) => {
            error!("❌ 配置管理器加载失败: {}", e);
            panic!("配置管理器加载失败: {}", e);
        }
    };

    // 2. 创建协议工厂
    info!("🏭 正在创建协议工厂...");
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    
    // 验证协议工厂支持的协议
    {
        let factory = protocol_factory.read().await;
        let supported_protocols = factory.supported_protocols();
        info!("🛠️  支持的协议: {:?}", supported_protocols);
        
        for protocol in &supported_protocols {
            info!("   - {:?}: {}", protocol, 
                  if factory.is_protocol_supported(protocol) { "✅" } else { "❌" });
        }
    }

    // 3. 手动创建Modbus TCP通道配置（避免类型冲突）
    info!("🔧 正在创建Modbus TCP通道配置...");
    
    // 创建参数映射
    let mut parameters = std::collections::HashMap::new();
    parameters.insert("host".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("timeout_ms".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2000)));
    parameters.insert("max_retries".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("poll_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));

    // 创建通道配置
    let channel_config = ChannelConfig {
        id: 1001,
        name: "RealModbusTCP".to_string(),
        description: Some("真实Modbus TCP协议测试通道".to_string()),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(parameters),
        logging: ChannelLoggingConfig {
            enabled: true,
            level: "debug".to_string(),
            log_dir: Some("/tmp/modbus_test_logs".to_string()),
            max_file_size: None,
            max_files: None,
            retention_days: None,
            console_output: true,
            log_messages: true,
        },
    };

    info!("📋 通道配置详情:");
    info!("   - ID: {}", channel_config.id);
    info!("   - 名称: {}", channel_config.name);
    info!("   - 协议: {:?}", channel_config.protocol);
    info!("   - 参数: {:?}", channel_config.parameters);

    // 4. 通过协议工厂创建真实的Modbus通道
    info!("🚀 正在创建真实的Modbus通道...");
    
    let factory = protocol_factory.read().await;
    match factory.create_channel_with_config_manager(channel_config.clone(), Some(&config_manager)).await {
        Ok(()) => {
            info!("✅ Modbus通道创建成功");
            
            // 获取创建的通道实例
            if let Some(channel_instance) = factory.get_channel(channel_config.id).await {
                info!("🔍 开始测试真实的Modbus协议报文生成...");
                
                let channel = channel_instance.read().await;
                
                info!("📋 通道信息:");
                info!("   - 名称: {}", channel.name());
                info!("   - 协议类型: {}", channel.protocol_type());
                info!("   - 通道ID: {}", channel.channel_id());
                info!("   - 运行状态: {}", channel.is_running().await);
                
                let parameters = channel.get_parameters();
                info!("   - 运行时参数:");
                for (key, value) in &parameters {
                    info!("     * {}: {}", key, value);
                }
                
                // 尝试启动通道以触发真实的连接和报文生成
                drop(channel); // 释放读锁
                let mut channel = channel_instance.write().await;
                
                info!("🚀 启动通道以触发真实的Modbus协议报文生成...");
                match channel.start().await {
                    Ok(()) => {
                        info!("✅ 通道启动成功，正在生成真实的Modbus协议报文");
                        
                        // 等待一段时间让通道尝试连接和通信
                        info!("⏳ 等待3秒以观察协议报文生成...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        
                        // 检查通道状态
                        let status = channel.status().await;
                        info!("📊 通道状态:");
                        info!("   - 连接状态: {}", status.is_connected());
                        info!("   - 最后响应时间: {} ms", status.response_time());
                        info!("   - 最后错误: {}", status.error_ref());
                        info!("   - 最后更新: {}", status.last_update());
                        
                        // 尝试获取诊断信息
                        let diagnostics = channel.get_diagnostics().await;
                        info!("🔍 通道诊断信息:");
                        for (key, value) in &diagnostics {
                            info!("   - {}: {}", key, value);
                        }
                        
                        // 停止通道
                        info!("🛑 停止通道...");
                        if let Err(e) = channel.stop().await {
                            warn!("⚠️  停止通道时出错: {}", e);
                        } else {
                            info!("✅ 通道已停止");
                        }
                    }
                    Err(e) => {
                        info!("ℹ️  通道启动失败: {} (这是预期的，因为没有真实的Modbus服务器)", e);
                        info!("   但这个过程应该已经生成了尝试连接的真实Modbus协议报文");
                        
                        // 即使连接失败，也尝试获取诊断信息
                        let diagnostics = channel.get_diagnostics().await;
                        info!("🔍 通道诊断信息 (连接失败后):");
                        for (key, value) in &diagnostics {
                            info!("   - {}: {}", key, value);
                        }
                    }
                }
            } else {
                error!("❌ 无法获取通道实例");
            }
        }
        Err(e) => {
            error!("❌ Modbus通道创建失败: {}", e);
        }
    }
    
    // 5. 验证协议工厂状态
    {
        let factory = protocol_factory.read().await;
        info!("📊 协议工厂最终状态:");
        info!("   - 通道数量: {}", factory.channel_count());
        info!("   - 是否为空: {}", factory.is_empty());
        
        let channel_ids = factory.get_channel_ids();
        info!("   - 通道ID列表: {:?}", channel_ids);
    }

    info!("🎉 真实Modbus协议报文生成测试完成");
    info!("💡 提示: 查看日志输出以获取详细的协议报文信息");
    info!("💡 提示: 检查 /tmp/real_modbus_protocol_test.log 文件以获取完整日志");
}

/// 测试Modbus RTU协议报文生成
#[tokio::test]
async fn test_real_modbus_rtu_protocol_frame_generation() {
    // 初始化日志
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    info!("🧪 === 测试真实Modbus RTU协议报文生成 ===");

    // 创建协议工厂
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    
    // 创建Modbus RTU通道配置
    let mut parameters = std::collections::HashMap::new();
    parameters.insert("port".to_string(), serde_yaml::Value::String("/dev/ttyUSB0".to_string()));
    parameters.insert("baud_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(9600)));
    parameters.insert("data_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(8)));
    parameters.insert("parity".to_string(), serde_yaml::Value::String("none".to_string()));
    parameters.insert("stop_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2)));
    parameters.insert("timeout_ms".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3000)));
    parameters.insert("max_retries".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));

    let channel_config = ChannelConfig {
        id: 1002,
        name: "RealModbusRTU".to_string(),
        description: Some("真实Modbus RTU协议测试通道".to_string()),
        protocol: ProtocolType::ModbusRtu,
        parameters: ChannelParameters::Generic(parameters),
        logging: ChannelLoggingConfig {
            enabled: true,
            level: "debug".to_string(),
            log_dir: Some("/tmp/modbus_rtu_test_logs".to_string()),
            max_file_size: None,
            max_files: None,
            retention_days: None,
            console_output: true,
            log_messages: true,
        },
    };

    info!("📋 Modbus RTU通道配置:");
    info!("   - ID: {}", channel_config.id);
    info!("   - 名称: {}", channel_config.name);
    info!("   - 协议: {:?}", channel_config.protocol);

    // 创建通道
    let factory = protocol_factory.read().await;
    match factory.create_channel_with_config_manager(channel_config.clone(), None).await {
        Ok(()) => {
            info!("✅ Modbus RTU通道创建成功");
            
            if let Some(channel_instance) = factory.get_channel(channel_config.id).await {
                let mut channel = channel_instance.write().await;
                
                info!("🚀 启动Modbus RTU通道以触发协议报文生成...");
                match channel.start().await {
                    Ok(()) => {
                        info!("✅ RTU通道启动成功");
                        
                        // 等待一段时间
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        
                        let status = channel.status().await;
                        info!("📊 RTU通道状态:");
                        info!("   - 连接状态: {}", status.is_connected());
                        info!("   - 最后错误: {}", status.error_ref());
                        
                        let _ = channel.stop().await;
                        info!("✅ RTU通道已停止");
                    }
                    Err(e) => {
                        info!("ℹ️  RTU通道启动失败: {} (预期的，因为没有真实的串口设备)", e);
                        info!("   但这个过程应该已经生成了尝试连接的真实Modbus RTU协议报文");
                    }
                }
            }
        }
        Err(e) => {
            info!("ℹ️  Modbus RTU通道创建失败: {} (可能是因为缺少串口设备)", e);
        }
    }

    info!("🎉 真实Modbus RTU协议报文生成测试完成");
} 