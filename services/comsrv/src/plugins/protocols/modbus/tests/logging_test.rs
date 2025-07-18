//! Modbus 日志记录测试
//! 
//! 测试 Modbus 协议在请求-响应过程中的详细日志记录功能

use std::collections::HashMap;
use std::time::Duration;
use tokio;
use tracing::{info, debug};
use tracing_test::traced_test;

use crate::plugins::protocols::modbus::{
    types::{ModbusChannelConfig, ProtocolMappingTable},
    common::ModbusConfig,
    tests::mock_transport::{MockTransport, MockTransportConfig},
    protocol_engine::{ModbusTelemetryMapping, ModbusSignalMapping},
};
use crate::core::framework::base::telemetry::TelemetryType;

/// 创建测试用的 Modbus 配置
fn create_test_modbus_config() -> ModbusConfig {
    ModbusConfig {
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
    }
}

/// 创建测试用的通道配置
fn create_test_channel_config() -> ModbusChannelConfig {
    ModbusChannelConfig {
        channel_id: 1,
        channel_name: "测试通道_日志".to_string(),
        connection: create_test_modbus_config(),
        request_timeout: Duration::from_millis(5000),
        max_retries: 3,
        retry_delay: Duration::from_millis(1000),
    }
}

/// 创建测试用的协议映射表
fn create_test_mappings() -> ProtocolMappingTable {
    let mut mappings = ProtocolMappingTable::default();
    
    // 添加遥测点映射
    mappings.telemetry_mappings.insert(1001, ModbusTelemetryMapping {
        point_id: 1001,
        slave_id: 1,
        address: 40001,
        data_type: "uint16".to_string(),
        scale: 1.0,
        offset: 0.0,
    });
    
    mappings.telemetry_mappings.insert(1002, ModbusTelemetryMapping {
        point_id: 1002,
        slave_id: 1,
        address: 40002,
        data_type: "float32".to_string(),
        scale: 0.1,
        offset: 0.0,
    });
    
    // 添加遥信点映射
    mappings.signal_mappings.insert(2001, ModbusSignalMapping {
        point_id: 2001,
        slave_id: 1,
        address: 10001,
        bit_location: Some(0),
    });
    
    mappings
}

#[tokio::test]
#[traced_test]
async fn test_modbus_logging_basic_operations() {
    info!("🧪 开始测试 Modbus 基本操作的日志记录");
    
    // 创建 Mock Transport 配置
    let mut mock_config = MockTransportConfig::default();
    mock_config.latency_ms = 10; // 模拟 10ms 延迟
    
    let mock_transport = MockTransport::new(mock_config);
    
    // 准备模拟响应数据
    // 遥测点读取响应 (功能码 0x03)
    let telemetry_response = vec![
        0x01, 0x03, 0x02, 0x12, 0x34  // 从站1, 功能码03, 2字节数据, 值=0x1234
    ];
    
    // 遥信点读取响应 (功能码 0x01)  
    let signal_response = vec![
        0x01, 0x01, 0x01, 0x01  // 从站1, 功能码01, 1字节数据, 值=0x01
    ];
    
    mock_transport.queue_responses(vec![
        telemetry_response,
        signal_response,
    ]).await;
    
    // 创建 Modbus 客户端
    let config = create_test_channel_config();
    let _transport = Box::new(mock_transport);
    let mut client = ModbusClient::new(config, _transport).await
        .expect("Failed to create Modbus client");
    
    info!("✅ Modbus 客户端创建成功");
    
    // 加载协议映射
    let mappings = create_test_mappings();
    client.load_protocol_mappings(mappings).await
        .expect("Failed to load protocol mappings");
    
    info!("✅ 协议映射加载成功");
    
    // 连接到设备
    client.connect().await
        .expect("Failed to connect to device");
    
    info!("✅ 设备连接成功，开始进行点位读取测试");
    
    // 测试遥测点读取 - 这应该产生详细的日志
    debug!("📊 测试遥测点读取 - Point ID: 1001");
    match client.read_point(1001, TelemetryType::Telemetry).await {
        Ok(point_data) => {
            info!("✅ 遥测点读取成功: {} = {}", point_data.name, point_data.value);
        }
        Err(e) => {
            info!("❌ 遥测点读取失败: {e}");
        }
    }
    
    // 测试遥信点读取 - 这应该产生详细的日志
    debug!("📡 测试遥信点读取 - Point ID: 2001");
    match client.read_point(2001, TelemetryType::Signaling).await {
        Ok(point_data) => {
            info!("✅ 遥信点读取成功: {} = {}", point_data.name, point_data.value);
        }
        Err(e) => {
            info!("❌ 遥信点读取失败: {e}");
        }
    }
    
    info!("🎯 Modbus 日志记录测试完成");
}

#[tokio::test]
#[traced_test]
async fn test_modbus_error_logging() {
    info!("🧪 开始测试 Modbus 错误情况的日志记录");
    
    let mock_config = MockTransportConfig::default();
    let mock_transport = MockTransport::new(mock_config);
    
    // 准备异常响应数据
    let exception_response = vec![
        0x01, 0x83, 0x02  // 从站1, 功能码03+0x80(异常), 异常码02(非法数据地址)
    ];
    
    mock_transport.queue_response(exception_response).await;
    
    let config = create_test_channel_config();
    let _transport = Box::new(mock_transport);
    let mut client = ModbusClient::new(config, _transport).await
        .expect("Failed to create Modbus client");
    
    let mappings = create_test_mappings();
    client.load_protocol_mappings(mappings).await
        .expect("Failed to load protocol mappings");
    
    client.connect().await
        .expect("Failed to connect to device");
    
    // 测试异常响应的日志记录
    debug!("🚨 测试异常响应的日志记录");
    match client.read_point(1001, TelemetryType::Telemetry).await {
        Ok(_) => {
            info!("⚠️ 意外成功 - 应该收到异常响应");
        }
        Err(e) => {
            info!("✅ 正确处理异常响应: {e}");
        }
    }
    
    info!("🎯 Modbus 错误日志记录测试完成");
}

#[tokio::test]
#[traced_test]
async fn test_modbus_batch_logging() {
    info!("🧪 开始测试 Modbus 批量操作的日志记录");
    
    let mock_config = MockTransportConfig::default();
    let mock_transport = MockTransport::new(mock_config);
    
    // 准备多个响应数据
    let responses = vec![
        vec![0x01, 0x03, 0x02, 0x12, 0x34],  // 点位 1001
        vec![0x01, 0x03, 0x04, 0x43, 0x70, 0x00, 0x00],  // 点位 1002 (float32)
        vec![0x01, 0x01, 0x01, 0x01],  // 点位 2001
    ];
    
    mock_transport.queue_responses(responses).await;
    
    let config = create_test_channel_config();
    let _transport = Box::new(mock_transport);
    let mut client = ModbusClient::new(config, _transport).await
        .expect("Failed to create Modbus client");
    
    let mappings = create_test_mappings();
    client.load_protocol_mappings(mappings).await
        .expect("Failed to load protocol mappings");
    
    client.connect().await
        .expect("Failed to connect to device");
    
    // 测试批量读取的日志记录
    debug!("📦 测试批量读取的日志记录");
    let point_ids = vec![1001, 1002, 2001];
    match client.read_points_batch(&point_ids).await {
        Ok(points) => {
            info!("✅ 批量读取成功，读取了 {} 个点位", points.len());
            for point in points {
                debug!("  📊 点位: {} = {}", point.name, point.value);
            }
        }
        Err(e) => {
            info!("❌ 批量读取失败: {e}");
        }
    }
    
    info!("🎯 Modbus 批量日志记录测试完成");
}

#[tokio::test]
#[traced_test] 
async fn test_transport_logging() {
    info!("🧪 开始测试传输层日志记录");
    
    let mock_config = MockTransportConfig::default();
    let mock_transport = MockTransport::new(mock_config);
    
    // 测试原始传输层日志
    let test_data = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x01, 0x00, 0x01];
    let response_data = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x12, 0x34];
    
    mock_transport.queue_response(response_data).await;
    
    info!("📤 模拟发送数据包");
    debug!("发送数据: {:02X?}", test_data);
    
    info!("📥 检查响应队列");
    let history = mock_transport.get_send_history().await;
    debug!("发送历史记录: {} 条", history.len());
    
    info!("🎯 传输层日志记录测试完成");
}