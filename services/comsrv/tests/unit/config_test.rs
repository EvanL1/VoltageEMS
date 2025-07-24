//! 配置系统单元测试

use comsrv::core::config::types::{ChannelConfig, CombinedPoint, ScalingInfo, TelemetryType};
use std::collections::HashMap;

/// 创建测试配置
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
        name: "Test Channel".to_string(),
        description: Some("Test channel for unit testing".to_string()),
        protocol: "modbus_tcp".to_string(),
        parameters,
        logging: Default::default(),
        table_config: None,
        points: vec![],
        combined_points: vec![],
    }
}

/// 创建测试点位
fn create_test_point() -> CombinedPoint {
    let mut protocol_params = HashMap::new();
    protocol_params.insert("slave_id".to_string(), "1".to_string());
    protocol_params.insert("function_code".to_string(), "3".to_string());
    protocol_params.insert("register_address".to_string(), "40001".to_string());

    CombinedPoint {
        point_id: 10001,
        signal_name: "Temperature".to_string(),
        telemetry_type: "Measurement".to_string(),
        data_type: "FLOAT32".to_string(),
        protocol_params,
        scaling: Some(ScalingInfo {
            scale: 0.1,
            offset: 0.0,
            unit: Some("°C".to_string()),
        }),
    }
}

#[test]
fn test_channel_config_creation() {
    let config = create_test_channel_config();

    assert_eq!(config.id, 1001);
    assert_eq!(config.name, "Test Channel");
    assert_eq!(config.protocol, "modbus_tcp");
    assert!(config.description.is_some());
    assert_eq!(config.description.unwrap(), "Test channel for unit testing");
}

#[test]
fn test_channel_config_parameters() {
    let config = create_test_channel_config();

    // 验证参数存在
    assert!(config.parameters.contains_key("host"));
    assert!(config.parameters.contains_key("port"));
    assert!(config.parameters.contains_key("timeout"));

    // 验证参数值
    if let Some(serde_yaml::Value::String(host)) = config.parameters.get("host") {
        assert_eq!(host, "127.0.0.1");
    } else {
        panic!("Host parameter not found or wrong type");
    }

    if let Some(serde_yaml::Value::Number(port)) = config.parameters.get("port") {
        assert_eq!(port.as_u64(), Some(502));
    } else {
        panic!("Port parameter not found or wrong type");
    }
}

#[test]
fn test_combined_point_creation() {
    let point = create_test_point();

    assert_eq!(point.point_id, 10001);
    assert_eq!(point.signal_name, "Temperature");
    assert_eq!(point.telemetry_type, "Measurement");
    assert_eq!(point.data_type, "FLOAT32");
}

#[test]
fn test_scaling_info() {
    let point = create_test_point();

    assert!(point.scaling.is_some());
    let scaling = point.scaling.unwrap();

    assert_eq!(scaling.scale, 0.1);
    assert_eq!(scaling.offset, 0.0);
    assert!(scaling.unit.is_some());
    assert_eq!(scaling.unit.unwrap(), "°C");
}

#[test]
fn test_protocol_parameters() {
    let point = create_test_point();

    assert!(point.protocol_params.contains_key("slave_id"));
    assert!(point.protocol_params.contains_key("function_code"));
    assert!(point.protocol_params.contains_key("register_address"));

    assert_eq!(
        point.protocol_params.get("slave_id"),
        Some(&"1".to_string())
    );
    assert_eq!(
        point.protocol_params.get("function_code"),
        Some(&"3".to_string())
    );
    assert_eq!(
        point.protocol_params.get("register_address"),
        Some(&"40001".to_string())
    );
}

#[test]
fn test_telemetry_type_parsing() {
    let telemetry_types = vec![
        ("Measurement", TelemetryType::Telemetry),
        ("Signal", TelemetryType::Signal),
        ("Control", TelemetryType::Control),
        ("Adjustment", TelemetryType::Adjustment),
    ];

    for (type_str, expected_type) in telemetry_types {
        // 这里可以添加字符串到TelemetryType的转换测试
        // 目前只验证类型定义正确
        let _telemetry_type = match type_str {
            "Measurement" => TelemetryType::Telemetry,
            "Signal" => TelemetryType::Signal,
            "Control" => TelemetryType::Control,
            "Adjustment" => TelemetryType::Adjustment,
            _ => TelemetryType::Telemetry,
        };
    }
}

#[test]
fn test_channel_config_with_points() {
    let mut config = create_test_channel_config();
    let point = create_test_point();

    config.combined_points.push(point);

    assert_eq!(config.combined_points.len(), 1);
    assert_eq!(config.combined_points[0].point_id, 10001);
    assert_eq!(config.combined_points[0].signal_name, "Temperature");
}

#[test]
fn test_multiple_points() {
    let mut config = create_test_channel_config();

    // 添加多个不同的点位
    for i in 1..=5 {
        let mut point = create_test_point();
        point.point_id = 10000 + i;
        point.signal_name = format!("Point_{}", i);
        config.combined_points.push(point);
    }

    assert_eq!(config.combined_points.len(), 5);

    for (i, point) in config.combined_points.iter().enumerate() {
        assert_eq!(point.point_id, 10001 + i as u32);
        assert_eq!(point.signal_name, format!("Point_{}", i + 1));
    }
}

#[test]
fn test_config_validation() {
    let config = create_test_channel_config();

    // 基本验证
    assert!(config.id > 0);
    assert!(!config.name.is_empty());
    assert!(!config.protocol.is_empty());
    assert!(!config.parameters.is_empty());
}

#[test]
fn test_data_types() {
    let data_types = vec!["UINT16", "INT16", "UINT32", "INT32", "FLOAT32", "DOUBLE"];

    for data_type in data_types {
        let mut point = create_test_point();
        point.data_type = data_type.to_string();

        assert_eq!(point.data_type, data_type);
    }
}
