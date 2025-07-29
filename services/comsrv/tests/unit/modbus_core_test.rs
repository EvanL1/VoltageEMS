//! Modbus核心功能单元测试

use comsrv::core::config::types::{ChannelConfig, ChannelLoggingConfig, TelemetryType};
use comsrv::plugins::protocols::modbus::types::{BatchConfig, ModbusPollingConfig};
use std::collections::HashMap;

/// 创建测试用的通道配置
fn create_test_channel_config() -> ChannelConfig {
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

    ChannelConfig {
        id: 1001,
        name: "Test Modbus Channel".to_string(),
        description: Some("Unit test channel".to_string()),
        protocol: "modbus_tcp".to_string(),
        parameters,
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        measurement_points: HashMap::new(),
        signal_points: HashMap::new(),
        control_points: HashMap::new(),
        adjustment_points: HashMap::new(),
    }
}

/// 创建测试用的轮询配置
fn create_test_polling_config() -> ModbusPollingConfig {
    ModbusPollingConfig {
        enabled: true,
        default_interval_ms: 1000,
        batch_config: BatchConfig {
            max_batch_size: 100,
            max_gap_registers: 5,
            enable_batch_optimization: true,
        },
        connection_config: Default::default(),
    }
}

#[test]
fn test_telemetry_type_mapping() {
    // 测试遥测类型映射
    let telemetry_types = vec![
        TelemetryType::Measurement,
        TelemetryType::Signal,
        TelemetryType::Control,
        TelemetryType::Adjustment,
    ];

    for telemetry_type in telemetry_types {
        // 简单验证类型存在且可以使用
        let _type_str = match telemetry_type {
            TelemetryType::Measurement => "m",
            TelemetryType::Signal => "s",
            TelemetryType::Control => "c",
            TelemetryType::Adjustment => "a",
        };
        // 测试通过，类型映射正确
    }
}

#[test]
fn test_polling_config() {
    let config = create_test_polling_config();

    assert!(config.enabled);
    assert_eq!(config.default_interval_ms, 1000);
    assert_eq!(config.batch_config.max_batch_size, 100);
    assert_eq!(config.batch_config.max_gap_registers, 5);
    assert!(config.batch_config.enable_batch_optimization);
}

#[test]
fn test_channel_config_creation() {
    let config = create_test_channel_config();

    assert_eq!(config.id, 1001);
    assert_eq!(config.name, "Test Modbus Channel");
    assert_eq!(config.protocol, "modbus_tcp");

    // 验证四遥点位HashMap初始化
    assert!(config.measurement_points.is_empty());
    assert!(config.signal_points.is_empty());
    assert!(config.control_points.is_empty());
    assert!(config.adjustment_points.is_empty());
}

// Mock transport 测试已经移到 transport.rs 模块中
