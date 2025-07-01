//! Modbus客户端使用示例
//!
//! 这个示例展示了如何使用新的Modbus架构：
//! - 现代化配置系统
//! - 高性能客户端
//! - 协议引擎
//! - 增强传输桥接
//! - 基础监控

use std::time::Duration;
use tokio::time;
use tracing::{info, warn, error};

use comsrv::core::config::{ConfigManager, NewChannelConfig};
use comsrv::core::protocols::modbus::{
    ModbusClient, ModbusChannelConfig, ProtocolMappingTable,
    ConnectionState, ClientStatistics,
};
use comsrv::core::protocols::common::combase::{
    BasicMonitoring, HealthChecker, HealthLevel, ConnectionHealthChecker,
    PerformanceHealthChecker, PerformanceThresholds, RequestPriority,
    EnhancedTransportBridge, ConnectionPoolConfig, RetryConfig,
};
use comsrv::core::transport::mock::{MockTransport, MockTransportConfig};
use comsrv::core::protocols::modbus::protocol_engine::{
    ModbusTelemetryMapping, ModbusSignalMapping,
};
use comsrv::utils::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    info!("启动Modbus客户端示例");
    
    // 1. 创建示例配置
    let config = create_example_config().await?;
    info!("创建了示例配置");
    
    // 2. 创建客户端
    let client = create_client(&config).await?;
    info!("创建了Modbus客户端");
    
    // 3. 设置监控
    let monitoring = setup_monitoring(&client).await?;
    info!("设置了监控系统");
    
    // 4. 连接并运行示例
    run_example(&client, &monitoring).await?;
    
    info!("Modbus客户端示例完成");
    Ok(())
}

/// 创建示例配置
async fn create_example_config() -> Result<NewChannelConfig> {
    // 创建配置管理器并生成示例配置
    let config = ConfigManager::generate_example_config();
    
    // 获取第一个通道配置
    let channel_config = config.channels.into_iter()
        .next()
        .ok_or_else(|| comsrv::utils::ComSrvError::ConfigError("没有找到通道配置".to_string()))?;
    
    info!("通道配置: {} (ID: {})", channel_config.name, channel_config.id);
    info!("协议类型: {}", channel_config.protocol);
    info!("点位数量: {}", channel_config.points.len());
    
    Ok(channel_config)
}

/// 创建客户端
async fn create_client(channel_config: &NewChannelConfig) -> Result<ModbusClient> {
    // 创建模拟传输
    let mock_config = MockTransportConfig {
        response_data: vec![0x01, 0x03, 0x04, 0x00, 0x64, 0x00, 0xC8], // 示例响应数据
        delay_ms: 100,
        should_fail: false,
    };
    let transport = Box::new(MockTransport::new(mock_config)?);
    
    // 转换为ModbusChannelConfig
    let config_manager = ConfigManager::from_file("nonexistent.yaml").await
        .unwrap_or_else(|_| {
            // 如果文件不存在，创建一个临时的配置管理器
            let config = ConfigManager::generate_example_config();
            ConfigManager { 
                config, 
                config_path: "temp".to_string(),
                last_modified: None,
            }
        });
    
    let modbus_config = config_manager.to_modbus_channel_config(channel_config);
    
    // 创建客户端
    let client = ModbusClient::new(modbus_config, transport).await?;
    
    // 加载协议映射
    let mappings = create_protocol_mappings(channel_config);
    client.load_protocol_mappings(mappings).await?;
    
    Ok(client)
}

/// 创建协议映射
fn create_protocol_mappings(channel_config: &NewChannelConfig) -> ProtocolMappingTable {
    let mut mappings = ProtocolMappingTable::default();
    
    for point in &channel_config.points {
        match point.point_type {
            comsrv::core::config::config::PointType::Telemetry => {
                let mapping = ModbusTelemetryMapping {
                    point_id: point.id,
                    slave_id: point.protocol_mapping.slave_id.unwrap_or(1),
                    function_code: point.protocol_mapping.function_code,
                    register_address: point.protocol_mapping.address,
                    register_count: point.protocol_mapping.count,
                    data_format: point.protocol_mapping.data_type.clone(),
                    byte_order: point.protocol_mapping.byte_order.clone(),
                };
                mappings.telemetry_mappings.insert(point.id, mapping);
            }
            comsrv::core::config::config::PointType::Signaling => {
                let mapping = ModbusSignalMapping {
                    point_id: point.id,
                    slave_id: point.protocol_mapping.slave_id.unwrap_or(1),
                    function_code: point.protocol_mapping.function_code,
                    register_address: point.protocol_mapping.address,
                    bit_position: point.protocol_mapping.bit_position,
                };
                mappings.signal_mappings.insert(point.id, mapping);
            }
            _ => {
                // 其他类型暂时不处理
            }
        }
    }
    
    info!("创建了 {} 个遥测映射", mappings.telemetry_mappings.len());
    info!("创建了 {} 个遥信映射", mappings.signal_mappings.len());
    
    mappings
}

/// 设置监控系统
async fn setup_monitoring(client: &ModbusClient) -> Result<BasicMonitoring> {
    let monitoring = BasicMonitoring::new("modbus_client".to_string());
    
    // 添加连接健康检查器
    let connection_checker = ConnectionHealthChecker::new(
        "modbus_connection".to_string(),
        || true, // 模拟连接检查
    );
    monitoring.add_health_checker(Box::new(connection_checker)).await;
    
    // 添加性能健康检查器
    let thresholds = PerformanceThresholds {
        max_error_rate: 5.0,
        max_response_time_ms: 2000.0,
        min_success_rate: 95.0,
        max_memory_usage_mb: 512.0,
    };
    
    let performance_checker = PerformanceHealthChecker::new(
        "modbus_performance".to_string(),
        std::sync::Arc::new(monitoring.clone()),
        thresholds,
    );
    monitoring.add_health_checker(Box::new(performance_checker)).await;
    
    // 启动监控任务
    monitoring.start_monitoring_task().await;
    
    Ok(monitoring)
}

/// 运行示例
async fn run_example(client: &ModbusClient, monitoring: &BasicMonitoring) -> Result<()> {
    info!("开始运行示例...");
    
    // 连接到设备
    client.connect().await?;
    info!("已连接到Modbus设备");
    
    // 运行循环
    let mut iteration = 0;
    while iteration < 10 {
        iteration += 1;
        info!("=== 迭代 {} ===", iteration);
        
        // 读取点位数据
        let start_time = std::time::Instant::now();
        
        // 尝试读取遥测点位
        match client.read_point(1001, comsrv::core::protocols::common::combase::TelemetryType::Telemetry).await {
            Ok(point_data) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                info!("读取遥测点位成功: {} = {}", point_data.name, point_data.value);
                monitoring.record_request(true, response_time).await;
            }
            Err(e) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                warn!("读取遥测点位失败: {}", e);
                monitoring.record_request(false, response_time).await;
            }
        }
        
        // 尝试读取遥信点位
        let start_time = std::time::Instant::now();
        match client.read_point(2001, comsrv::core::protocols::common::combase::TelemetryType::Signaling).await {
            Ok(point_data) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                info!("读取遥信点位成功: {} = {}", point_data.name, point_data.value);
                monitoring.record_request(true, response_time).await;
            }
            Err(e) => {
                let response_time = start_time.elapsed().as_millis() as u64;
                warn!("读取遥信点位失败: {}", e);
                monitoring.record_request(false, response_time).await;
            }
        }
        
        // 每5次迭代显示统计信息
        if iteration % 5 == 0 {
            display_statistics(client, monitoring).await;
        }
        
        // 等待1秒
        time::sleep(Duration::from_secs(1)).await;
    }
    
    // 最终统计
    display_final_statistics(client, monitoring).await;
    
    // 断开连接
    client.disconnect().await?;
    info!("已断开Modbus设备连接");
    
    Ok(())
}

/// 显示统计信息
async fn display_statistics(client: &ModbusClient, monitoring: &BasicMonitoring) {
    info!("--- 当前统计信息 ---");
    
    // 客户端统计
    let client_stats = client.get_statistics().await;
    info!("客户端统计:");
    info!("  总请求数: {}", client_stats.total_requests);
    info!("  成功请求数: {}", client_stats.successful_requests);
    info!("  失败请求数: {}", client_stats.failed_requests);
    info!("  平均响应时间: {:.1}ms", client_stats.average_response_time_ms);
    
    // 监控统计
    let monitoring_metrics = monitoring.get_performance_metrics().await;
    info!("监控指标:");
    info!("  请求速率: {:.2} req/s", monitoring_metrics.request_rate);
    info!("  成功率: {:.1}%", monitoring_metrics.success_rate);
    info!("  错误率: {:.1}%", monitoring_metrics.error_rate);
    info!("  运行时间: {}s", monitoring_metrics.uptime_seconds);
    
    // 健康检查
    let health_results = monitoring.health_check().await;
    info!("健康检查:");
    for result in health_results {
        info!("  {}: {} - {}", result.component, result.level, result.message);
    }
}

/// 显示最终统计信息
async fn display_final_statistics(client: &ModbusClient, monitoring: &BasicMonitoring) {
    info!("=== 最终统计信息 ===");
    
    // 连接状态
    let connection_state = client.get_connection_state().await;
    info!("连接状态:");
    info!("  已连接: {}", connection_state.connected);
    info!("  重试次数: {}", connection_state.retry_count);
    if let Some(last_connect) = connection_state.last_connect_time {
        info!("  最后连接时间: {}", last_connect.format("%Y-%m-%d %H:%M:%S"));
    }
    
    // 映射统计
    let mapping_counts = client.get_mapping_counts().await;
    info!("映射统计:");
    for (mapping_type, count) in mapping_counts {
        info!("  {}: {}", mapping_type, count);
    }
    
    // 系统状态
    let system_status = monitoring.get_system_status().await;
    info!("系统状态:");
    for (key, value) in system_status {
        info!("  {}: {}", key, value);
    }
    
    // 健康检查
    if let Ok(health) = client.health_check().await {
        info!("客户端健康检查:");
        for (key, value) in health {
            info!("  {}: {}", key, value);
        }
    }
}

