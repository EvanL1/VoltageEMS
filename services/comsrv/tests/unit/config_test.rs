//! 配置系统单元测试

use comsrv::core::config::types::{
    ChannelConfig, ChannelLoggingConfig, CombinedPoint, ScalingInfo, TelemetryType,
};
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
        logging: ChannelLoggingConfig::default(),
        table_config: None,
        measurement_points: HashMap::new(),
        signal_points: HashMap::new(),
        control_points: HashMap::new(),
        adjustment_points: HashMap::new(),
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
            reverse: None,
        }),
    }
}

/// 创建测试信号点位（带reverse）
fn create_test_signal_point() -> CombinedPoint {
    let mut protocol_params = HashMap::new();
    protocol_params.insert("slave_id".to_string(), "1".to_string());
    protocol_params.insert("function_code".to_string(), "2".to_string());
    protocol_params.insert("register_address".to_string(), "10001".to_string());
    protocol_params.insert("bit_position".to_string(), "0".to_string());

    CombinedPoint {
        point_id: 1,
        signal_name: "Breaker_Status".to_string(),
        telemetry_type: "Signal".to_string(),
        data_type: "bool".to_string(),
        protocol_params,
        scaling: Some(ScalingInfo {
            scale: 1.0,
            offset: 0.0,
            unit: None,
            reverse: Some(true),
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
    assert!(scaling.reverse.is_none());
}

#[test]
fn test_signal_point_with_reverse() {
    let point = create_test_signal_point();

    assert_eq!(point.point_id, 1);
    assert_eq!(point.telemetry_type, "Signal");
    assert_eq!(point.data_type, "bool");

    let scaling = point.scaling.unwrap();
    assert_eq!(scaling.reverse, Some(true));
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
        ("Measurement", TelemetryType::Measurement),
        ("Signal", TelemetryType::Signal),
        ("Control", TelemetryType::Control),
        ("Adjustment", TelemetryType::Adjustment),
    ];

    for (type_str, expected_type) in telemetry_types {
        // 这里可以添加字符串到TelemetryType的转换测试
        // 目前只验证类型定义正确
        let _telemetry_type = match type_str {
            "Measurement" => TelemetryType::Measurement,
            "Signal" => TelemetryType::Signal,
            "Control" => TelemetryType::Control,
            "Adjustment" => TelemetryType::Adjustment,
            _ => TelemetryType::Measurement,
        };
    }
}

#[test]
fn test_channel_config_with_points() {
    let mut config = create_test_channel_config();
    let point = create_test_point();

    config
        .measurement_points
        .insert(point.point_id, point.clone());

    assert_eq!(config.measurement_points.len(), 1);
    assert_eq!(
        config.measurement_points.get(&10001).unwrap().point_id,
        10001
    );
    assert_eq!(
        config.measurement_points.get(&10001).unwrap().signal_name,
        "Temperature"
    );
}

#[test]
fn test_multiple_points() {
    let mut config = create_test_channel_config();

    // 添加多个不同的点位到不同的遥测类型
    for i in 1..=5 {
        let mut point = create_test_point();
        point.point_id = i;
        point.signal_name = format!("Point_{}", i);

        match i {
            1..=2 => config.measurement_points.insert(point.point_id, point),
            3..=4 => config.signal_points.insert(point.point_id, point),
            _ => config.control_points.insert(point.point_id, point),
        };
    }

    assert_eq!(config.measurement_points.len(), 2);
    assert_eq!(config.signal_points.len(), 2);
    assert_eq!(config.control_points.len(), 1);

    // 验证点位ID都是从1开始
    assert!(config.measurement_points.contains_key(&1));
    assert!(config.measurement_points.contains_key(&2));
    assert!(config.signal_points.contains_key(&3));
    assert!(config.signal_points.contains_key(&4));
    assert!(config.control_points.contains_key(&5));
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
    let data_types = vec![
        "UINT16", "INT16", "UINT32", "INT32", "FLOAT32", "DOUBLE", "bool",
    ];

    for data_type in data_types {
        let mut point = create_test_point();
        point.data_type = data_type.to_string();

        assert_eq!(point.data_type, data_type);
    }
}

#[test]
fn test_four_telemetry_separation() {
    let mut config = create_test_channel_config();

    // 创建不同类型的点位
    let mut measurement_point = create_test_point();
    measurement_point.point_id = 1;
    measurement_point.telemetry_type = "Measurement".to_string();

    let mut signal_point = create_test_signal_point();
    signal_point.point_id = 1;
    signal_point.telemetry_type = "Signal".to_string();

    let mut control_point = create_test_point();
    control_point.point_id = 1;
    control_point.telemetry_type = "Control".to_string();
    control_point.scaling = Some(ScalingInfo {
        scale: 1.0,
        offset: 0.0,
        unit: None,
        reverse: Some(true),
    });

    let mut adjustment_point = create_test_point();
    adjustment_point.point_id = 1;
    adjustment_point.telemetry_type = "Adjustment".to_string();

    // 插入到不同的HashMap
    config.measurement_points.insert(1, measurement_point);
    config.signal_points.insert(1, signal_point);
    config.control_points.insert(1, control_point);
    config.adjustment_points.insert(1, adjustment_point);

    // 验证四个HashMap相互独立，都可以有相同的点位ID
    assert_eq!(config.measurement_points.len(), 1);
    assert_eq!(config.signal_points.len(), 1);
    assert_eq!(config.control_points.len(), 1);
    assert_eq!(config.adjustment_points.len(), 1);

    // 验证每种类型都有点位ID=1
    assert!(config.measurement_points.contains_key(&1));
    assert!(config.signal_points.contains_key(&1));
    assert!(config.control_points.contains_key(&1));
    assert!(config.adjustment_points.contains_key(&1));
}
