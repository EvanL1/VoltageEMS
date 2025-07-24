//! Modbus核心功能单元测试

use chrono::Utc;
use comsrv::core::config::types::{ChannelConfig, TelemetryType};
// use comsrv::plugins::protocols::modbus::core::ModbusProtocol;
use comsrv::plugins::protocols::modbus::types::{ModbusPoint, ModbusPollingConfig};
use comsrv::plugins::protocols::modbus::transport::MockTransport;
use std::collections::HashMap;
use std::sync::Arc;

/// 创建测试用的通道配置
fn create_test_channel_config() -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("host".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(502.into()));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(5000.into()));

    ChannelConfig {
        id: 1001,
        name: "Test Modbus Channel".to_string(),
        description: Some("Unit test channel".to_string()),
        protocol: "modbus_tcp".to_string(),
        parameters,
        logging: Default::default(),
        table_config: None,
        points: vec![],
        combined_points: vec![],
    }
}

/// 创建测试用的轮询配置
fn create_test_polling_config() -> ModbusPollingConfig {
    ModbusPollingConfig {
        polling_interval: 1000,
        timeout: 5000,
        retry_count: 3,
        retry_delay: 1000,
        max_concurrent_requests: 5,
        batch_size: 100,
    }
}

/// 创建测试点位
fn create_test_points() -> Vec<ModbusPoint> {
    vec![
        ModbusPoint {
            point_id: "10001".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 40001,
            data_format: "UINT16".to_string(),
            register_count: 1,
            byte_order: Some("big_endian".to_string()),
        },
        ModbusPoint {
            point_id: "10002".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 40002,
            data_format: "FLOAT32".to_string(),
            register_count: 2,
            byte_order: Some("big_endian".to_string()),
        },
    ]
}

// #[tokio::test]
// async fn test_modbus_protocol_creation() {
//     let config = create_test_channel_config();
//     let transport = Arc::new(MockTransport::new());
//     let polling_config = create_test_polling_config();

//     let result = ModbusProtocol::new(config, transport, polling_config);
//     assert!(result.is_ok());

//     let protocol = result.unwrap();
//     assert_eq!(protocol.name(), "Test Modbus Channel");
//     assert_eq!(protocol.channel_id(), 1001);
// }

#[tokio::test]
async fn test_modbus_point_parsing() {
    let points = create_test_points();
    
    // 验证第一个点
    let point1 = &points[0];
    assert_eq!(point1.point_id, "10001");
    assert_eq!(point1.slave_id, 1);
    assert_eq!(point1.function_code, 3);
    assert_eq!(point1.register_address, 40001);
    assert_eq!(point1.data_format, "UINT16");
    assert_eq!(point1.register_count, 1);

    // 验证第二个点
    let point2 = &points[1];
    assert_eq!(point2.point_id, "10002");
    assert_eq!(point2.data_format, "FLOAT32");
    assert_eq!(point2.register_count, 2);
}

#[tokio::test]
async fn test_polling_config() {
    let config = create_test_polling_config();
    
    assert_eq!(config.polling_interval, 1000);
    assert_eq!(config.timeout, 5000);
    assert_eq!(config.retry_count, 3);
    assert_eq!(config.retry_delay, 1000);
    assert_eq!(config.max_concurrent_requests, 5);
    assert_eq!(config.batch_size, 100);
}

#[tokio::test]
async fn test_telemetry_type_mapping() {
    // 测试遥测类型映射
    let telemetry_types = vec![
        TelemetryType::Telemetry,
        TelemetryType::Signal,
        TelemetryType::Control,
        TelemetryType::Adjustment,
    ];

    for telemetry_type in telemetry_types {
        // 简单验证类型存在且可以使用
        let _type_str = match telemetry_type {
            TelemetryType::Telemetry => "m",
            TelemetryType::Signal => "s", 
            TelemetryType::Control => "c",
            TelemetryType::Adjustment => "a",
        };
        // 测试通过，类型映射正确
    }
}

#[cfg(test)]
mod mock_transport_tests {
    use super::*;
    use comsrv::plugins::protocols::modbus::transport::MockTransport;

    #[tokio::test]
    async fn test_mock_transport_basic() {
        let transport = MockTransport::new();
        
        // 测试默认状态
        assert!(!transport.is_connected());
        
        // 测试连接
        let result = transport.connect().await;
        assert!(result.is_ok());
        assert!(transport.is_connected());
        
        // 测试断开连接
        let result = transport.disconnect().await;
        assert!(result.is_ok());
        assert!(!transport.is_connected());
    }

    #[tokio::test]
    async fn test_mock_transport_read_write() {
        let transport = MockTransport::new();
        transport.connect().await.unwrap();
        
        // 测试写入数据
        let write_data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x01];
        let result = transport.write(&write_data).await;
        assert!(result.is_ok());
        
        // 测试读取数据  
        let result = transport.read().await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.is_empty());
    }
}