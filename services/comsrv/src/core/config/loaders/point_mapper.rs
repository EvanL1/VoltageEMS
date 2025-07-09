//! Point Mapping Utilities
//!
//! This module provides utilities for mapping between different point formats,
//! combining four telemetry data with protocol mappings, and handling point validation.

use super::csv_loader::{FourTelemetryRecord, ModbusMappingRecord};
use crate::utils::error::{ComSrvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Combined point data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedPoint {
    // Four telemetry data
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: Option<String>,
    pub data_type: String,
    pub scale: f64,
    pub offset: f64,
    pub reverse: Option<bool>,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub group: Option<String>,

    // Protocol mapping data
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_format: String,
    pub number_of_bytes: Option<u8>,
    pub bit_position: Option<u8>,
    pub byte_order: Option<String>,
    pub register_count: Option<u16>,

    // Computed fields
    pub telemetry_type: String, // YC, YX, YT, YK
}

/// Point mapper utility
#[derive(Debug)]
pub struct PointMapper;

impl PointMapper {
    /// Combine four telemetry data with Modbus mappings
    pub fn combine_modbus_points(
        telemetry_points: Vec<FourTelemetryRecord>,
        modbus_mappings: Vec<ModbusMappingRecord>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        // Create a HashMap for fast lookup of Modbus mappings
        let mut mapping_lookup: HashMap<u32, ModbusMappingRecord> = HashMap::new();
        for mapping in modbus_mappings {
            mapping_lookup.insert(mapping.point_id, mapping);
        }

        let mut combined_points = Vec::new();

        for telemetry in telemetry_points {
            if let Some(mapping) = mapping_lookup.remove(&telemetry.point_id) {
                let combined = CombinedPoint {
                    // Four telemetry data
                    point_id: telemetry.point_id,
                    signal_name: telemetry.signal_name.to_string(),
                    chinese_name: telemetry.chinese_name.map(|s| s.to_string()),
                    data_type: telemetry.data_type.to_string(),
                    scale: telemetry.scale.unwrap_or(1.0),
                    offset: telemetry.offset.unwrap_or(0.0),
                    reverse: telemetry.reverse,
                    unit: telemetry.unit.map(|s| s.to_string()),
                    description: telemetry.description.map(|s| s.to_string()),
                    group: telemetry.group.map(|s| s.to_string()),

                    // Protocol mapping data
                    slave_id: mapping.slave_id,
                    function_code: mapping.function_code,
                    register_address: mapping.register_address,
                    data_format: mapping.data_format.to_string(),
                    number_of_bytes: mapping.number_of_bytes,
                    bit_position: mapping.bit_position,
                    byte_order: mapping.byte_order.map(|s| s.to_string()),
                    register_count: mapping.register_count,

                    // Computed fields
                    telemetry_type: telemetry_type.to_string(),
                };

                combined_points.push(combined);
            } else {
                tracing::warn!(
                    "No protocol mapping found for point_id: {}",
                    telemetry.point_id
                );
            }
        }

        // Check for unmapped protocol mappings
        for (point_id, _) in mapping_lookup {
            tracing::warn!(
                "Protocol mapping for point_id {} has no corresponding telemetry point",
                point_id
            );
        }

        Ok(combined_points)
    }

    /// Validate combined points for consistency
    pub fn validate_combined_points(points: &[CombinedPoint]) -> Result<()> {
        for point in points {
            // Validate point ID
            if point.point_id == 0 {
                return Err(ComSrvError::ConfigError(
                    "Point ID cannot be zero".to_string(),
                ));
            }

            // Validate signal name
            if point.signal_name.is_empty() {
                return Err(ComSrvError::ConfigError(format!(
                    "Signal name cannot be empty for point_id: {}",
                    point.point_id
                )));
            }

            // Validate Modbus slave ID
            if point.slave_id == 0 || point.slave_id > 247 {
                return Err(ComSrvError::ConfigError(format!(
                    "Invalid Modbus slave ID: {} for point_id: {}. Must be 1-247",
                    point.slave_id, point.point_id
                )));
            }

            // Validate data type
            match point.data_type.as_str() {
                "bool" | "uint8" | "int8" | "uint16" | "int16" | "uint32" | "int32" | "float32"
                | "float64" => {}
                _ => {
                    return Err(ComSrvError::ConfigError(format!(
                        "Unsupported data type '{}' for point_id: {}",
                        point.data_type, point.point_id
                    )))
                }
            }

            // Validate function code
            match point.function_code {
                1 | 2 | 3 | 4 | 5 | 6 | 15 | 16 => {} // Valid Modbus function codes
                _ => {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid Modbus function code: {} for point_id: {}",
                        point.function_code, point.point_id
                    )))
                }
            }

            // Validate bit position if specified
            if let Some(bit_pos) = point.bit_position {
                if bit_pos == 0 || bit_pos > 16 {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid bit position: {} for point_id: {}. Must be 1-16",
                        bit_pos, point.point_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Group points by telemetry type
    pub fn group_points_by_type(points: Vec<CombinedPoint>) -> HashMap<String, Vec<CombinedPoint>> {
        let mut grouped = HashMap::new();

        for point in points {
            grouped
                .entry(point.telemetry_type.clone())
                .or_insert_with(Vec::new)
                .push(point);
        }

        grouped
    }

    /// Filter points by various criteria
    pub fn filter_points<F>(points: Vec<CombinedPoint>, predicate: F) -> Vec<CombinedPoint>
    where
        F: Fn(&CombinedPoint) -> bool,
    {
        points.into_iter().filter(predicate).collect()
    }

    /// Get statistics about points
    pub fn get_point_statistics(points: &[CombinedPoint]) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        // Count by telemetry type
        let grouped = Self::group_points_by_type(points.to_vec());
        for (telemetry_type, type_points) in grouped {
            stats.insert(format!("{}_count", telemetry_type), type_points.len());
        }

        // Count by data type
        let mut data_type_counts: HashMap<String, usize> = HashMap::new();
        for point in points {
            *data_type_counts.entry(point.data_type.clone()).or_insert(0) += 1;
        }

        for (data_type, count) in data_type_counts {
            stats.insert(format!("data_type_{}_count", data_type), count);
        }

        stats.insert("total_points".to_string(), points.len());

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_telemetry() -> FourTelemetryRecord {
        FourTelemetryRecord {
            point_id: 1,
            signal_name: "Test Signal".to_string(),
            chinese_name: Some("测试信号".to_string()),
            data_type: "float32".to_string(),
            scale: Some(1.0),
            offset: Some(0.0),
            reverse: None,
            unit: Some("V".to_string()),
            description: Some("Test description".to_string()),
            group: Some("Test Group".to_string()),
        }
    }

    fn create_test_mapping() -> ModbusMappingRecord {
        ModbusMappingRecord {
            point_id: 1,
            signal_name: "Test Signal".to_string(),
            slave_id: 1,
            function_code: 3,
            register_address: 100,
            data_format: "ABCD".to_string(),
            number_of_bytes: Some(4),
            bit_position: None,
            byte_order: Some("big_endian".to_string()),
            register_count: Some(2),
            description: Some("Test mapping".to_string()),
        }
    }

    #[test]
    fn test_combine_points() {
        let telemetry_points = vec![create_test_telemetry()];
        let protocol_mappings = vec![create_test_mapping()];

        let combined =
            PointMapper::combine_modbus_points(telemetry_points, protocol_mappings, "YC").unwrap();

        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].point_id, 1);
        assert_eq!(combined[0].signal_name, "Test Signal");
        assert_eq!(combined[0].slave_id, 1);
        assert_eq!(combined[0].telemetry_type, "YC");
    }

    #[test]
    fn test_validate_combined_points() {
        let mut combined = CombinedPoint {
            point_id: 1,
            signal_name: "Test Signal".to_string(),
            chinese_name: Some("测试信号".to_string()),
            data_type: "float32".to_string(),
            scale: 1.0,
            offset: 0.0,
            reverse: None,
            unit: Some("V".to_string()),
            description: Some("Test description".to_string()),
            group: Some("Test Group".to_string()),
            slave_id: 1,
            function_code: 3,
            register_address: 100,
            data_format: "ABCD".to_string(),
            number_of_bytes: Some(4),
            bit_position: None,
            byte_order: Some("big_endian".to_string()),
            register_count: Some(2),
            telemetry_type: "YC".to_string(),
        };

        // Valid point should pass
        assert!(PointMapper::validate_combined_points(&[combined.clone()]).is_ok());

        // Invalid slave ID should fail
        combined.slave_id = 0;
        assert!(PointMapper::validate_combined_points(&[combined.clone()]).is_err());

        // Invalid function code should fail
        combined.slave_id = 1;
        combined.function_code = 99;
        assert!(PointMapper::validate_combined_points(&[combined]).is_err());
    }

    #[test]
    fn test_group_points_by_type() {
        let mut point1 = CombinedPoint {
            point_id: 1,
            signal_name: "Signal 1".to_string(),
            chinese_name: None,
            data_type: "float32".to_string(),
            scale: 1.0,
            offset: 0.0,
            reverse: None,
            unit: None,
            description: None,
            group: None,
            slave_id: 1,
            function_code: 3,
            register_address: 100,
            data_format: "ABCD".to_string(),
            number_of_bytes: Some(4),
            bit_position: None,
            byte_order: None,
            register_count: None,
            telemetry_type: "YC".to_string(),
        };

        let mut point2 = point1.clone();
        point2.point_id = 2;
        point2.telemetry_type = "YX".to_string();

        let grouped = PointMapper::group_points_by_type(vec![point1, point2]);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["YC"].len(), 1);
        assert_eq!(grouped["YX"].len(), 1);
    }
}
