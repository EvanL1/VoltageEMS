//! Usage Examples for Utils Module
//!
//! This module provides examples demonstrating how to use the various utility
//! functions in the utils module.

#[cfg(test)]
mod examples {
    use crate::utils::{
        error::{ComSrvError, ErrorExt, Result},
        hex::{bytes_to_hex, format_hex_spaced, hex_to_bytes},
        serialization::{from_json_string, to_json_string},
        time::{current_timestamp, elapsed_ms, sleep_ms},
    };
    use serde::{Deserialize, Serialize};
    use std::time::Instant;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct ExampleData {
        name: String,
        value: i32,
        enabled: bool,
    }

    /// Example: Using error handling utilities
    #[tokio::test]
    async fn example_error_handling() {
        fn might_fail() -> std::result::Result<String, std::io::Error> {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
        }

        // Using ErrorExt trait for better error messages
        let result: Result<String> = might_fail().config_error("Failed to load configuration");

        assert!(result.is_err());
        if let Err(ComSrvError::ConfigError(msg)) = result {
            assert!(msg.contains("Failed to load configuration"));
            assert!(msg.contains("File not found"));
        }
    }

    /// Example: Using time utilities
    #[tokio::test]
    async fn example_time_utilities() {
        // Get current timestamp
        let timestamp = current_timestamp();
        println!("Current timestamp: {}", timestamp);
        assert!(!timestamp.is_empty());

        // Measure elapsed time
        let start = Instant::now();
        sleep_ms(10).await;
        let elapsed = elapsed_ms(start);
        println!("Operation took: {}", elapsed);
        assert!(elapsed.contains("ms"));
    }

    /// Example: Using serialization utilities
    #[test]
    fn example_serialization() {
        let data = ExampleData {
            name: "test".to_string(),
            value: 42,
            enabled: true,
        };

        // Serialize to JSON
        let json = to_json_string(&data).unwrap();
        println!("JSON: {}", json);

        // Deserialize from JSON
        let parsed: ExampleData = from_json_string(&json).unwrap();
        assert_eq!(parsed, data);

        println!("Serialization example completed successfully!");
    }

    /// Example: Using hex utilities
    #[test]
    fn example_hex_utilities() {
        let data = vec![0x01, 0x02, 0x03, 0xFF, 0xAB, 0xCD];

        // Convert to hex string
        let hex = bytes_to_hex(&data);
        println!("Hex string: {}", hex);
        assert_eq!(hex, "010203ffabcd");

        // Format with spaces for readability
        let pretty_hex = format_hex_spaced(&data);
        println!("Pretty hex: {}", pretty_hex);
        assert_eq!(pretty_hex, "01 02 03 ff ab cd");

        // Convert back to bytes
        let recovered = hex_to_bytes(&hex).unwrap();
        assert_eq!(recovered, data);

        println!("Hex utilities example completed successfully!");
    }

    /// Example: Combined usage in a realistic scenario
    #[tokio::test]
    async fn example_protocol_message_handling() {
        // Simulate receiving a protocol message
        let message_data = vec![0x01, 0x03, 0x00, 0x00, 0x00, 0x02];
        let timestamp = current_timestamp();

        // Create a log entry
        let log_entry = LogEntry {
            timestamp,
            direction: "RX".to_string(),
            hex_data: format_hex_spaced(&message_data),
            description: "Read holding registers request".to_string(),
        };

        // Serialize log entry
        let json = to_json_string(&log_entry).unwrap();
        println!("Log entry JSON: {}", json);

        // Parse it back
        let parsed: LogEntry = from_json_string(&json).unwrap();
        assert_eq!(parsed.direction, "RX");
        assert!(parsed.hex_data.contains("01 03"));

        println!("Protocol message handling example completed!");
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct LogEntry {
        timestamp: String,
        direction: String,
        hex_data: String,
        description: String,
    }
}

/// # 数据来源属性使用示例
/// 
/// 本文件展示了如何在VoltageEMS中使用新的数据来源属性功能。
/// 数据来源属性允许您为每个点位指定数据的来源类型：协议、计算或手动。

use crate::core::config::config_manager::{
    AnalogPointConfig, DigitalPointConfig, DataSourceType, DataSource, SourceTables, SourceResolution,
    ModbusTcpSource
};
use serde_json::json;

/// Example: Create analog point with Modbus TCP data source
pub fn create_analog_point_with_modbus_source() -> AnalogPointConfig {
    AnalogPointConfig {
        id: 1,
        name: "PLC_MAIN_U".to_string(),
        chinese_name: "PLC主站电压".to_string(),
        data_source: DataSourceType::Protocol {
            config_id: "tcp_001".to_string(),
        },
        scale: 1.0,
        offset: 0.0,
        unit: Some("kV".to_string()),
        description: Some("PLC主站母线电压".to_string()),
        group: Some("电力系统".to_string()),
    }
}

/// Example: Create analog point with calculation data source
pub fn create_analog_point_with_calculation_source() -> AnalogPointConfig {
    AnalogPointConfig {
        id: 2,
        name: "PLC_CALC_POWER".to_string(),
        chinese_name: "PLC计算功率".to_string(),
        data_source: DataSourceType::Calculation {
            calculation_id: "calc_001".to_string(),
        },
        scale: 1.0,
        offset: 0.0,
        unit: Some("MW".to_string()),
        description: Some("电压电流计算功率".to_string()),
        group: Some("计算数据".to_string()),
    }
}

/// Example: Create digital point with manual data source
pub fn create_digital_point_with_manual_source() -> DigitalPointConfig {
    DigitalPointConfig {
        id: 101,
        name: "PLC_MANUAL_SWITCH".to_string(),
        chinese_name: "PLC手动开关".to_string(),
        data_source: DataSourceType::Manual {
            editable: true,
            default_value: Some(json!(false)),
        },
        description: Some("手动控制开关".to_string()),
        group: Some("手动控制".to_string()),
    }
}

/// Example: Load and use source tables from Redis
pub fn demonstrate_redis_source_table_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Create Redis-based source tables
    let source_tables = SourceTables::new("redis://127.0.0.1:6379", Some("comsrv:source_tables"))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Load CSV files into Redis
    source_tables.load_from_csv_to_redis(
        Some("modbus_tcp_source_table.csv"),
        Some("calculation_source_table.csv"),
        Some("manual_source_table.csv"),
    ).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Create a point configuration
    let point = create_analog_point_with_modbus_source();
    
    // For demonstration, create a DataSource from the point's DataSourceType
    let data_source = match &point.data_source {
        DataSourceType::Protocol { config_id } => DataSource {
            source_table: "modbus_tcp".to_string(),
            source_data: config_id.parse::<u32>().unwrap_or(1),
        },
        DataSourceType::Calculation { calculation_id } => DataSource {
            source_table: "calculation".to_string(),
            source_data: calculation_id.parse::<u32>().unwrap_or(1),
        },
        DataSourceType::Manual { .. } => DataSource {
            source_table: "manual".to_string(),
            source_data: 1,
        },
    };
    
    // Validate the data source (checks Redis)
    if let Err(e) = source_tables.validate_data_source(&data_source) {
        println!("Data source validation failed: {}", e);
        return Ok(());
    }
    
    // Resolve the data source from Redis
    match source_tables.resolve_source(&data_source) {
        Ok(Some(resolution)) => {
            match resolution {
                SourceResolution::ModbusTcp(modbus_src) => {
                    println!("Resolved Modbus TCP source from Redis:");
                    println!("  Source ID: {}", modbus_src.source_id);
                    println!("  Protocol: {}", modbus_src.protocol_type);
                    println!("  Slave ID: {}", modbus_src.slave_id);
                    println!("  Function Code: {}", modbus_src.function_code);
                    println!("  Register Address: {}", modbus_src.register_address);
                    println!("  Data Type: {}", modbus_src.data_type);
                    println!("  Description: {:?}", modbus_src.description);
                }
                SourceResolution::Calculation(calc_src) => {
                    println!("Resolved Calculation source from Redis:");
                    println!("  Source ID: {}", calc_src.source_id);
                    println!("  Type: {}", calc_src.calculation_type);
                    println!("  Expression: {}", calc_src.expression);
                    println!("  Source Points: {}", calc_src.source_points);
                    println!("  Update Interval: {}ms", calc_src.update_interval_ms);
                }
                SourceResolution::Manual(manual_src) => {
                    println!("Resolved Manual source from Redis:");
                    println!("  Source ID: {}", manual_src.source_id);
                    println!("  Type: {}", manual_src.manual_type);
                    println!("  Editable: {}", manual_src.editable);
                    println!("  Default Value: {}", manual_src.default_value);
                    println!("  Value Type: {}", manual_src.value_type);
                }
            }
        }
        Ok(None) => {
            println!("Data source not found in Redis: {:?}", data_source);
        }
        Err(e) => {
            println!("Failed to resolve data source from Redis: {}", e);
        }
    }

    Ok(())
}

/// Example: Demonstrate Redis CRUD operations for source tables
pub fn demonstrate_redis_crud_operations() -> Result<(), Box<dyn std::error::Error>> {
    let source_tables = SourceTables::new("redis://127.0.0.1:6379", Some("comsrv:source_tables"))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Create a new Modbus TCP source
    let new_modbus_source = ModbusTcpSource {
        source_id: 100,
        protocol_type: "modbus_tcp".to_string(),
        slave_id: 2,
        function_code: 4,
        register_address: 50001,
        data_type: "float32".to_string(),
        byte_order: "big_endian".to_string(),
        bit_index: None,
        scaling_factor: Some(0.01),
        description: Some("新增温度传感器".to_string()),
    };

    // Add to Redis
    source_tables.set_modbus_tcp_source(&new_modbus_source)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("✅ Added new Modbus TCP source to Redis: ID {}", new_modbus_source.source_id);

    // Read from Redis
    match source_tables.get_modbus_tcp_source(100) {
        Ok(Some(source)) => {
            println!("✅ Retrieved source from Redis: {:?}", source);
        }
        Ok(None) => {
            println!("❌ Source not found in Redis");
        }
        Err(e) => {
            println!("❌ Failed to retrieve from Redis: {}", e);
        }
    }

    // List all Modbus TCP source IDs
    match source_tables.list_source_ids("modbus_tcp") {
        Ok(ids) => {
            println!("✅ All Modbus TCP source IDs in Redis: {:?}", ids);
        }
        Err(e) => {
            println!("❌ Failed to list source IDs: {}", e);
        }
    }

    // Delete the source
    match source_tables.delete_source("modbus_tcp", 100) {
        Ok(true) => {
            println!("✅ Deleted source ID 100 from Redis");
        }
        Ok(false) => {
            println!("⚠️ Source ID 100 was not found for deletion");
        }
        Err(e) => {
            println!("❌ Failed to delete source: {}", e);
        }
    }

    Ok(())
}

/// Example: Demonstrate Redis validation
pub fn demonstrate_redis_validation() -> Result<(), Box<dyn std::error::Error>> {
    let source_tables = SourceTables::new("redis://127.0.0.1:6379", Some("comsrv:source_tables"))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Load test data
    source_tables.load_from_csv_to_redis(
        Some("modbus_tcp_source_table.csv"),
        Some("calculation_source_table.csv"),
        Some("manual_source_table.csv"),
    ).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Valid data source (exists in Redis)
    let valid_source = DataSource {
        source_table: "modbus_tcp".to_string(),
        source_data: 1,
    };

    // Invalid data source (doesn't exist in Redis)
    let invalid_source = DataSource {
        source_table: "modbus_tcp".to_string(),
        source_data: 999,
    };

    // Test valid source
    match source_tables.validate_data_source(&valid_source) {
        Ok(()) => println!("✅ Valid source data {} exists in Redis", valid_source.source_data),
        Err(e) => println!("✗ Validation error: {}", e),
    }

    // Test invalid source
    match source_tables.validate_data_source(&invalid_source) {
        Ok(()) => println!("✅ Valid source data: {}", invalid_source.source_data),
        Err(e) => println!("✗ Expected validation error: {}", e),
    }

    Ok(())
}

/// Example: Initialize Redis with sample data
pub fn initialize_redis_with_sample_data() -> Result<(), Box<dyn std::error::Error>> {
    let source_tables = SourceTables::new("redis://127.0.0.1:6379", Some("comsrv:source_tables"))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Clear existing data
    source_tables.clear_all_sources()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("🧹 Cleared all existing source data from Redis");

    // Load fresh data from CSV files
    source_tables.load_from_csv_to_redis(
        Some("modbus_tcp_source_table.csv"),
        Some("calculation_source_table.csv"),
        Some("manual_source_table.csv"),
    ).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("🚀 Successfully loaded source tables into Redis:");
    
    // Show loaded data counts
    if let Ok(modbus_ids) = source_tables.list_source_ids("modbus_tcp") {
        println!("  - Modbus TCP sources: {} entries", modbus_ids.len());
    }
    if let Ok(calc_ids) = source_tables.list_source_ids("calculation") {
        println!("  - Calculation sources: {} entries", calc_ids.len());
    }
    if let Ok(manual_ids) = source_tables.list_source_ids("manual") {
        println!("  - Manual sources: {} entries", manual_ids.len());
    }

    Ok(())
}

/// Create a mixed point configuration for testing
pub fn create_mixed_point_configuration() -> Vec<(String, DataSourceType)> {
    vec![
        ("PLC_MAIN_U".to_string(), DataSourceType::Protocol { config_id: "1".to_string() }),
        ("PLC_MAIN_I".to_string(), DataSourceType::Protocol { config_id: "2".to_string() }),
        ("PLC_CALC_POWER".to_string(), DataSourceType::Calculation { calculation_id: "1".to_string() }),
        ("PLC_CALC_ENERGY".to_string(), DataSourceType::Calculation { calculation_id: "2".to_string() }),
        ("PLC_MANUAL_SET".to_string(), DataSourceType::Manual { editable: true, default_value: Some(serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())) }),
        ("PLC_MANUAL_STATUS".to_string(), DataSourceType::Manual { editable: true, default_value: Some(serde_json::Value::Number(serde_json::Number::from_f64(1.0).unwrap())) }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_source_creation() {
        let source = DataSource {
            source_table: "modbus_tcp".to_string(),
            source_data: 1,
        };
        
        assert_eq!(source.source_table, "modbus_tcp");
        assert_eq!(source.source_data, 1);
    }

    #[test]
    fn test_analog_point_with_source() {
        let point = create_analog_point_with_modbus_source();
        
        assert_eq!(point.id, 1);
        assert_eq!(point.name, "PLC_MAIN_U");
        match &point.data_source {
            DataSourceType::Protocol { config_id } => {
                assert_eq!(config_id, "tcp_001");
            }
            _ => panic!("Expected Protocol data source"),
        }
    }

    #[test]
    fn test_digital_point_with_source() {
        let point = create_digital_point_with_manual_source();
        
        assert_eq!(point.id, 101);
        assert_eq!(point.name, "PLC_MANUAL_SWITCH");
        match &point.data_source {
            DataSourceType::Manual { editable, .. } => {
                assert_eq!(editable, &true);
            }
            _ => panic!("Expected Manual data source"),
        }
    }

    #[test]
    fn test_mixed_configuration() {
        let config = create_mixed_point_configuration();
        
        assert_eq!(config.len(), 6);
        
        // Check first Modbus TCP point
        let (point_id, source) = &config[0];
        assert_eq!(point_id, "PLC_MAIN_U");
        match source {
            DataSourceType::Protocol { config_id } => {
                assert_eq!(config_id, "1");
            }
            _ => panic!("Expected Protocol data source"),
        }
        
        // Check first calculation point
        let (point_id, source) = &config[2];
        assert_eq!(point_id, "PLC_CALC_POWER");
        match source {
            DataSourceType::Calculation { calculation_id } => {
                assert_eq!(calculation_id, "1");
            }
            _ => panic!("Expected Calculation data source"),
        }
        
        // Check first manual point
        let (point_id, source) = &config[4];
        assert_eq!(point_id, "PLC_MANUAL_SET");
        match source {
            DataSourceType::Manual { editable, .. } => {
                assert_eq!(editable, &true);
            }
            _ => panic!("Expected Manual data source"),
        }
    }
} 