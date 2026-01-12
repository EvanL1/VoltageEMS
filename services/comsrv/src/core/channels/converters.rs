//! Point configuration converters
//!
//! Convert comsrv RuntimeChannelConfig to IGW PointConfig/CanPoint.
//!
//! This module handles the "translation" between comsrv's configuration format
//! and the IGW protocol library's point configuration format.

use tracing::warn;

use crate::core::config::RuntimeChannelConfig;
use crate::protocols::core::point::{
    PointConfig, ProtocolAddress, TransformConfig, VirtualAddress,
};
use voltage_model::PointType;

#[cfg(feature = "modbus")]
use crate::protocols::core::point::{ByteOrder, DataFormat, ModbusAddress};

#[cfg(all(feature = "can", target_os = "linux"))]
use crate::protocols::adapters::can::{CanDataType, CanPoint};

// ============================================================================
// Virtual Channel Point Conversion
// ============================================================================

/// Convert RuntimeChannelConfig to IGW PointConfig list.
///
/// This function sets up TransformConfig for each point type:
/// - Telemetry: scale/offset transformation
/// - Signal: reverse boolean transformation
/// - Control: reverse boolean transformation
/// - Adjustment: scale/offset transformation
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id,
/// avoiding collisions when different types share the same original point_id.
pub fn convert_to_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Convert telemetry points with scale/offset transformation
    for pt in &runtime_config.telemetry_points {
        let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert signal points with reverse transformation
    for pt in &runtime_config.signal_points {
        let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert control points with reverse transformation
    for pt in &runtime_config.control_points {
        let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            }),
        );
    }

    // Convert adjustment points with scale/offset transformation
    for pt in &runtime_config.adjustment_points {
        let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
        configs.push(
            PointConfig::new(
                internal_id,
                ProtocolAddress::Virtual(VirtualAddress::new(pt.base.point_id.to_string())),
            )
            .with_name(&pt.base.signal_name)
            .with_transform(TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                ..Default::default()
            }),
        );
    }

    configs
}

// ============================================================================
// Modbus Point Conversion
// ============================================================================

/// Convert RuntimeChannelConfig to IGW PointConfig list for Modbus.
///
/// Extracts Modbus mapping information from each point's embedded protocol_mappings JSON field.
/// This replaces the old approach of using separate modbus_mappings collection.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
#[cfg(feature = "modbus")]
pub fn convert_to_modbus_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Helper to parse modbus config from protocol_mappings JSON
    // Returns: (slave_id, function_code, register, data_type, byte_order, bit_position)
    fn parse_modbus_mapping(
        json_str: &str,
        point_id: u32,
    ) -> Option<(u8, u8, u16, String, String, Option<u8>)> {
        let v: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Point {} has invalid protocol_mappings JSON: {}",
                    point_id, e
                );
                return None;
            },
        };

        if !v.is_object() {
            warn!(
                "Point {} has invalid protocol_mappings (expected JSON object): {}",
                point_id, v
            );
            return None;
        }

        fn parse_u64_field(v: &serde_json::Value, key: &str) -> Option<u64> {
            let raw = v.get(key)?;
            raw.as_u64()
                .or_else(|| raw.as_i64().and_then(|n| u64::try_from(n).ok()))
                .or_else(|| raw.as_str().and_then(|s| s.parse::<u64>().ok()))
        }

        let slave_id: u8 = match parse_u64_field(&v, "slave_id").and_then(|n| u8::try_from(n).ok())
        {
            Some(n) => n,
            None => {
                warn!(
                    "Point {} protocol_mappings missing/invalid 'slave_id': {}",
                    point_id, v
                );
                return None;
            },
        };

        let function_code: u8 =
            match parse_u64_field(&v, "function_code").and_then(|n| u8::try_from(n).ok()) {
                Some(n) => n,
                None => {
                    warn!(
                        "Point {} protocol_mappings missing/invalid 'function_code': {}",
                        point_id, v
                    );
                    return None;
                },
            };

        let register: u16 =
            match parse_u64_field(&v, "register_address").and_then(|n| u16::try_from(n).ok()) {
                Some(n) => n,
                None => {
                    warn!(
                        "Point {} protocol_mappings missing/invalid 'register_address': {}",
                        point_id, v
                    );
                    return None;
                },
            };

        Some((
            slave_id,
            function_code,
            register,
            v.get("data_type")
                .and_then(|x| x.as_str())
                .unwrap_or("uint16")
                .to_string(),
            v.get("byte_order")
                .and_then(|x| x.as_str())
                .unwrap_or("ABCD")
                .to_string(),
            // bit_position: None means not set, Some(0) means bit 0
            parse_u64_field(&v, "bit_position").and_then(|n| u8::try_from(n).ok()),
        ))
    }

    // Process telemetry points
    for point in &runtime_config.telemetry_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Telemetry.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    scale: point.scale,
                    offset: point.offset,
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process signal points
    for point in &runtime_config.signal_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Signal.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process control points
    for point in &runtime_config.control_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Control.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    reverse: point.reverse,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    // Process adjustment points
    for point in &runtime_config.adjustment_points {
        if let Some(ref mappings_json) = point.base.protocol_mappings {
            if let Some((
                slave_id,
                function_code,
                register,
                data_type_str,
                byte_order_str,
                bit_pos,
            )) = parse_modbus_mapping(mappings_json, point.base.point_id)
            {
                let internal_id = PointType::Adjustment.to_internal_id(point.base.point_id);
                let modbus_addr = ModbusAddress {
                    slave_id,
                    function_code,
                    register,
                    format: parse_data_format(&data_type_str),
                    byte_order: parse_byte_order(&byte_order_str),
                    bit_position: bit_pos,
                };
                let transform = TransformConfig {
                    scale: point.scale,
                    offset: point.offset,
                    ..Default::default()
                };
                let config = PointConfig::new(internal_id, ProtocolAddress::Modbus(modbus_addr))
                    .with_transform(transform);
                configs.push(config);
            }
        }
    }

    configs
}

/// Parse data format string to DataFormat enum.
#[cfg(feature = "modbus")]
pub fn parse_data_format(s: &str) -> DataFormat {
    match s.to_lowercase().as_str() {
        "bool" | "boolean" => DataFormat::Bool,
        "uint16" | "u16" => DataFormat::UInt16,
        "int16" | "i16" => DataFormat::Int16,
        "uint32" | "u32" => DataFormat::UInt32,
        "int32" | "i32" => DataFormat::Int32,
        "float32" | "f32" | "float" => DataFormat::Float32,
        "float64" | "f64" | "double" => DataFormat::Float64,
        "uint64" | "u64" => DataFormat::UInt64,
        "int64" | "i64" => DataFormat::Int64,
        _ => DataFormat::UInt16, // Default
    }
}

/// Parse byte order string to ByteOrder enum.
#[cfg(feature = "modbus")]
pub fn parse_byte_order(s: &str) -> ByteOrder {
    match s.to_uppercase().as_str() {
        "ABCD" | "BIG_ENDIAN" | "BE" => ByteOrder::Abcd,
        "DCBA" | "LITTLE_ENDIAN" | "LE" => ByteOrder::Dcba,
        "BADC" | "WORD_SWAP" => ByteOrder::Badc,
        "CDAB" | "BYTE_SWAP" => ByteOrder::Cdab,
        _ => ByteOrder::Abcd, // Default to big-endian
    }
}

// ============================================================================
// CAN Point Conversion
// ============================================================================

/// CAN protocol mapping from protocol_mappings JSON field
#[cfg(all(feature = "can", target_os = "linux"))]
#[derive(Debug, Clone, serde::Deserialize)]
struct CanProtocolMapping {
    can_id: u32,
    start_bit: u32,
    bit_length: u32,
    #[serde(default)]
    data_type: CanDataType,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    offset: f64,
}

#[cfg(all(feature = "can", target_os = "linux"))]
fn default_scale() -> f64 {
    1.0
}

/// Convert RuntimeChannelConfig to IGW CanPoint list for CAN protocol.
///
/// Parses CAN configuration from each point's protocol_mappings JSON field.
/// Scale and offset are applied during decoding in the protocol layer.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
#[cfg(all(feature = "can", target_os = "linux"))]
pub fn convert_to_can_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<CanPoint> {
    let mut configs = Vec::new();

    // Helper to parse protocol_mappings JSON and create CanPoint
    let parse_can_point =
        |internal_id: u32, protocol_mappings: &Option<String>| -> Option<CanPoint> {
            let json_str = protocol_mappings.as_ref()?;
            let mapping: CanProtocolMapping = serde_json::from_str(json_str)
                .map_err(|e| {
                    tracing::warn!(
                        internal_id,
                        error = %e,
                        "Failed to parse CAN protocol_mappings JSON"
                    );
                    e
                })
                .ok()?;

            Some(CanPoint {
                point_id: internal_id,
                can_id: mapping.can_id,
                byte_offset: (mapping.start_bit / 8) as u8,
                bit_position: (mapping.start_bit % 8) as u8,
                bit_length: mapping.bit_length as u8,
                data_type: mapping.data_type,
                scale: mapping.scale,
                offset: mapping.offset,
            })
        };

    // Collect from all point types with internal_id encoding
    for pt in &runtime_config.telemetry_points {
        let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.signal_points {
        let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.control_points {
        let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }
    for pt in &runtime_config.adjustment_points {
        let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
        if let Some(can_point) = parse_can_point(internal_id, &pt.base.protocol_mappings) {
            configs.push(can_point);
        }
    }

    configs
}

/// Convert runtime CAN mappings to IGW PointConfig format (for RedisDataStore).
///
/// This conversion is used to register points with the data store for proper
/// data transformation and storage.
/// Parses CAN configuration from each point's protocol_mappings JSON field.
///
/// **Important**: Uses `PointType::to_internal_id()` to encode type into point_id.
#[cfg(all(feature = "can", target_os = "linux"))]
pub fn convert_can_to_point_configs(runtime_config: &RuntimeChannelConfig) -> Vec<PointConfig> {
    let mut configs = Vec::new();

    // Helper to build protocol address from CAN mapping
    let build_protocol_addr = |protocol_mappings: &Option<String>| -> Option<ProtocolAddress> {
        let json_str = protocol_mappings.as_ref()?;
        let mapping: CanProtocolMapping = serde_json::from_str(json_str).ok()?;
        Some(ProtocolAddress::Generic(format!(
            "can_id:0x{:X},start_bit:{},len:{}",
            mapping.can_id, mapping.start_bit, mapping.bit_length
        )))
    };

    // Telemetry points
    for pt in &runtime_config.telemetry_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Telemetry.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Signal points
    for pt in &runtime_config.signal_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Signal.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Control points
    for pt in &runtime_config.control_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Control.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                reverse: pt.reverse,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    // Adjustment points
    for pt in &runtime_config.adjustment_points {
        if let Some(protocol_addr) = build_protocol_addr(&pt.base.protocol_mappings) {
            let internal_id = PointType::Adjustment.to_internal_id(pt.base.point_id);
            let transform = TransformConfig {
                scale: pt.scale,
                offset: pt.offset,
                ..Default::default()
            };
            let config = PointConfig::new(internal_id, protocol_addr).with_transform(transform);
            configs.push(config);
        }
    }

    configs
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::core::config::{
        AdjustmentPoint, ChannelConfig, ChannelCore, ControlPoint, Point, SignalPoint,
        TelemetryPoint,
    };
    use std::collections::HashMap;

    fn create_test_runtime_config() -> RuntimeChannelConfig {
        let base_config = ChannelConfig {
            core: ChannelCore {
                id: 1,
                name: "test_channel".to_string(),
                description: None,
                protocol: "virtual".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };
        let mut config = RuntimeChannelConfig::from_base(base_config);

        config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 10,
                signal_name: "temperature".to_string(),
                description: None,
                unit: Some("C".to_string()),
                protocol_mappings: None,
            },
            scale: 1.0,
            offset: 0.0,
            data_type: "float32".to_string(),
            reverse: false,
        });

        config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 20,
                signal_name: "status".to_string(),
                description: None,
                unit: None,
                protocol_mappings: None,
            },
            reverse: false,
        });

        config.control_points.push(ControlPoint {
            base: Point {
                point_id: 30,
                signal_name: "switch".to_string(),
                description: None,
                unit: None,
                protocol_mappings: None,
            },
            reverse: false,
            control_type: "latching".to_string(),
            on_value: 1,
            off_value: 0,
            pulse_duration_ms: None,
        });

        config.adjustment_points.push(AdjustmentPoint {
            base: Point {
                point_id: 40,
                signal_name: "setpoint".to_string(),
                description: None,
                unit: Some("C".to_string()),
                protocol_mappings: None,
            },
            min_value: None,
            max_value: None,
            step: 1.0,
            data_type: "float32".to_string(),
            scale: 1.0,
            offset: 0.0,
        });

        config
    }

    #[test]
    fn test_convert_to_point_configs() {
        use voltage_model::PointType;

        let runtime_config = create_test_runtime_config();
        let configs = convert_to_point_configs(&runtime_config);

        assert_eq!(configs.len(), 4);

        // Check telemetry point - now uses internal_id
        let telemetry_internal = PointType::Telemetry.to_internal_id(10);
        let telemetry = configs.iter().find(|c| c.id == telemetry_internal).unwrap();
        assert_eq!(telemetry.name, Some("temperature".to_string()));

        // Check signal point exists with internal_id
        let signal_internal = PointType::Signal.to_internal_id(20);
        assert!(configs.iter().any(|c| c.id == signal_internal));

        // Check control point exists with internal_id
        let control_internal = PointType::Control.to_internal_id(30);
        assert!(configs.iter().any(|c| c.id == control_internal));

        // Check adjustment point exists with internal_id
        let adjustment_internal = PointType::Adjustment.to_internal_id(40);
        assert!(configs.iter().any(|c| c.id == adjustment_internal));
    }

    #[test]
    #[cfg(feature = "modbus")]
    fn test_convert_to_modbus_point_configs() {
        // Create a runtime config with embedded protocol_mappings
        let base_config = ChannelConfig {
            core: ChannelCore {
                id: 1,
                name: "test_modbus".to_string(),
                description: None,
                protocol: "modbus_tcp".to_string(),
                enabled: true,
            },
            parameters: HashMap::new(),
            logging: Default::default(),
        };
        let mut runtime_config = RuntimeChannelConfig::from_base(base_config);

        // Add telemetry point with embedded Modbus mapping
        runtime_config.telemetry_points.push(TelemetryPoint {
            base: Point {
                point_id: 100,
                signal_name: "voltage".to_string(),
                description: None,
                unit: Some("V".to_string()),
                protocol_mappings: Some(r#"{"slave_id":1,"function_code":3,"register_address":0,"data_type":"float32","byte_order":"ABCD"}"#.to_string()),
            },
            scale: 1.0,
            offset: 0.0,
            data_type: "float32".to_string(),
            reverse: false,
        });

        // Add signal point with embedded Modbus mapping (with bit_position)
        runtime_config.signal_points.push(SignalPoint {
            base: Point {
                point_id: 101,
                signal_name: "status".to_string(),
                description: None,
                unit: None,
                protocol_mappings: Some(r#"{"slave_id":1,"function_code":1,"register_address":10,"data_type":"bool","byte_order":"ABCD","bit_position":5}"#.to_string()),
            },
            reverse: false,
        });

        use voltage_model::PointType;

        let configs = convert_to_modbus_point_configs(&runtime_config);

        assert_eq!(configs.len(), 2);

        // Check first point (telemetry, float32) - now uses internal_id encoding
        let telemetry_internal = PointType::Telemetry.to_internal_id(100);
        let pt1 = configs.iter().find(|c| c.id == telemetry_internal).unwrap();
        if let ProtocolAddress::Modbus(addr) = &pt1.address {
            assert_eq!(addr.slave_id, 1);
            assert_eq!(addr.function_code, 3);
            assert_eq!(addr.register, 0);
            assert_eq!(addr.format, DataFormat::Float32);
            assert_eq!(addr.byte_order, ByteOrder::Abcd);
        } else {
            panic!("Expected ModbusAddress");
        }

        // Check second point (signal, bool with bit_position) - now uses internal_id encoding
        let signal_internal = PointType::Signal.to_internal_id(101);
        let pt2 = configs.iter().find(|c| c.id == signal_internal).unwrap();
        if let ProtocolAddress::Modbus(addr) = &pt2.address {
            assert_eq!(addr.slave_id, 1);
            assert_eq!(addr.function_code, 1);
            assert_eq!(addr.register, 10);
            assert_eq!(addr.format, DataFormat::Bool);
            assert_eq!(addr.bit_position, Some(5));
        } else {
            panic!("Expected ModbusAddress");
        }
    }

    #[test]
    #[cfg(feature = "modbus")]
    fn test_parse_data_format() {
        assert_eq!(parse_data_format("bool"), DataFormat::Bool);
        assert_eq!(parse_data_format("FLOAT32"), DataFormat::Float32);
        assert_eq!(parse_data_format("uint16"), DataFormat::UInt16);
        assert_eq!(parse_data_format("Int32"), DataFormat::Int32);
    }

    #[test]
    #[cfg(feature = "modbus")]
    fn test_parse_byte_order() {
        assert_eq!(parse_byte_order("ABCD"), ByteOrder::Abcd);
        assert_eq!(parse_byte_order("big_endian"), ByteOrder::Abcd);
        assert_eq!(parse_byte_order("CDAB"), ByteOrder::Cdab);
        assert_eq!(parse_byte_order("DCBA"), ByteOrder::Dcba);
    }

    /// Test the specific internal_id encoding for all four point types.
    #[test]
    fn test_internal_id_encoding_for_all_point_types() {
        use voltage_model::PointType;

        let point_id = 1u32;

        // Telemetry: offset = 0
        let telemetry_internal = PointType::Telemetry.to_internal_id(point_id);
        assert_eq!(telemetry_internal, point_id); // No offset

        // Signal: offset = OFFSET (0x40000000)
        let signal_internal = PointType::Signal.to_internal_id(point_id);
        assert_eq!(signal_internal, PointType::OFFSET + point_id);

        // Control: offset = OFFSET * 2 (0x80000000)
        let control_internal = PointType::Control.to_internal_id(point_id);
        assert_eq!(control_internal, PointType::OFFSET * 2 + point_id);

        // Adjustment: offset = OFFSET * 3 (0xC0000000)
        let adjustment_internal = PointType::Adjustment.to_internal_id(point_id);
        assert_eq!(adjustment_internal, PointType::OFFSET * 3 + point_id);

        // Verify round-trip
        let (pt, id) = PointType::from_internal_id(control_internal);
        assert_eq!(pt, PointType::Control);
        assert_eq!(id, point_id);
    }
}
