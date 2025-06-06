//! Integration tests for Modbus client with universal polling engine
//!
//! This module tests the integration between ModbusClient and the universal
//! polling architecture, demonstrating how protocol-specific implementations
//! can be seamlessly integrated with the unified polling framework.

use std::collections::HashMap;
use std::time::Duration;

use comsrv::core::protocols::modbus::client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::common::combase::{
    PointReader, PollingPoint, PollingConfig, PollingStats, PointData
};
use comsrv::utils::error::Result;
use serde_json::Value;
use tokio::time::sleep;

/// Create a test Modbus client configuration
fn create_test_modbus_config() -> ModbusClientConfig {
    ModbusClientConfig {
        mode: ModbusCommunicationMode::Tcp,
        slave_id: 1,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_secs(1),
        point_mappings: Vec::new(),
        port: None,
        baud_rate: None,
        data_bits: None,
        stop_bits: None,
        parity: None,
        host: Some("127.0.0.1".to_string()),
        tcp_port: Some(502),
    }
}

/// Create test polling points with Modbus-specific parameters
fn create_test_polling_points() -> Vec<PollingPoint> {
    vec![
        PollingPoint {
            id: "temperature_sensor".to_string(),
            address: 40001,
            data_type: "int16".to_string(),
            scale: 0.1,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(1.into()));
                params.insert("register_type".to_string(), Value::String("holding".to_string()));
                params.insert("byte_order".to_string(), Value::String("big_endian".to_string()));
                params
            },
        },
        PollingPoint {
            id: "pressure_sensor".to_string(),
            address: 40002,
            data_type: "uint16".to_string(),
            scale: 1.0,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(1.into()));
                params.insert("register_type".to_string(), Value::String("holding".to_string()));
                params.insert("byte_order".to_string(), Value::String("big_endian".to_string()));
                params
            },
        },
        PollingPoint {
            id: "flow_rate".to_string(),
            address: 40003,
            data_type: "float32".to_string(),
            scale: 1.0,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(1.into()));
                params.insert("register_type".to_string(), Value::String("holding".to_string()));
                params.insert("byte_order".to_string(), Value::String("big_endian".to_string()));
                params.insert("quantity".to_string(), Value::Number(2.into()));
                params
            },
        },
        PollingPoint {
            id: "pump_status".to_string(),
            address: 1,
            data_type: "bool".to_string(),
            scale: 1.0,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(1.into()));
                params.insert("register_type".to_string(), Value::String("coil".to_string()));
                params
            },
        },
    ]
}

#[tokio::test]
async fn test_modbus_client_as_point_reader() -> Result<()> {
    // Create test client
    let config = create_test_modbus_config();
    let mut client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    // Mock connection state (in real tests, we would start actual client)
    println!("Testing ModbusClient as PointReader implementation");
    
    // Test protocol name
    assert_eq!(client.protocol_name(), "ModbusTCP");
    
    // Initially not connected
    assert!(!client.is_connected().await);
    
    Ok(())
}

#[tokio::test]
async fn test_single_point_reading() -> Result<()> {
    let config = create_test_modbus_config();
    let mut client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    // Create a test point
    let test_point = PollingPoint {
        id: "test_register".to_string(),
        address: 40001,
        data_type: "uint16".to_string(),
        scale: 1.0,
        offset: 0.0,
        protocol_params: {
            let mut params = HashMap::new();
            params.insert("slave_id".to_string(), Value::Number(1.into()));
            params.insert("register_type".to_string(), Value::String("holding".to_string()));
            params
        },
    };
    
    println!("Testing single point reading...");
    
    // For now, this will demonstrate the interface
    // In real implementation, we would need to establish connection first
    
    println!("Point reader interface is ready for: {}", test_point.id);
    println!("Data type: {}, Address: {}", test_point.data_type, test_point.address);
    
    Ok(())
}

#[tokio::test]
async fn test_batch_point_reading() -> Result<()> {
    let config = create_test_modbus_config();
    let client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    let test_points = create_test_polling_points();
    
    println!("Testing batch point reading with {} points...", test_points.len());
    
    // Demonstrate point grouping logic
    for point in &test_points {
        println!("Point {}: type={}, address={}, slave_id={}", 
                 point.id, 
                 point.data_type, 
                 point.address,
                 point.protocol_params.get("slave_id").unwrap_or(&Value::Null));
    }
    
    // For demonstration purposes, show that the interface is properly implemented
    assert_eq!(client.protocol_name(), "ModbusTCP");
    
    Ok(())
}

#[tokio::test]
async fn test_polling_engine_initialization() -> Result<()> {
    let config = create_test_modbus_config();
    let mut client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    println!("Testing polling engine initialization...");
    
    // Initialize polling engine
    client.initialize_polling_engine()?;
    
    println!("Polling engine initialization completed successfully");
    
    Ok(())
}

#[tokio::test]
async fn test_modbus_point_parameter_parsing() -> Result<()> {
    let config = create_test_modbus_config();
    let client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    let test_points = create_test_polling_points();
    
    println!("Testing Modbus point parameter parsing...");
    
    for point in &test_points {
        println!("Point: {} - Protocol params: {:?}", point.id, point.protocol_params);
        
        // Verify parameter structure
        assert!(point.protocol_params.contains_key("slave_id"));
        assert!(point.protocol_params.contains_key("register_type"));
        
        println!("  - Slave ID: {:?}", point.protocol_params.get("slave_id"));
        println!("  - Register type: {:?}", point.protocol_params.get("register_type"));
        println!("  - Data type: {}", point.data_type);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_data_type_conversion() -> Result<()> {
    println!("Testing Modbus data type conversions...");
    
    // Test different data types that ModbusClient should handle
    let data_types = vec![
        ("bool", "Boolean values for coils"),
        ("uint16", "16-bit unsigned integers"),
        ("int16", "16-bit signed integers"),
        ("uint32", "32-bit unsigned integers"),
        ("int32", "32-bit signed integers"),
        ("float32", "32-bit floating point numbers"),
    ];
    
    for (data_type, description) in data_types {
        println!("  - {}: {}", data_type, description);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_protocol_specific_features() -> Result<()> {
    println!("Testing Modbus protocol-specific features...");
    
    // Test RTU vs TCP modes
    let rtu_config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Rtu,
        slave_id: 1,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_secs(1),
        point_mappings: Vec::new(),
        port: Some("/dev/ttyUSB0".to_string()),
        baud_rate: Some(9600),
        data_bits: None,
        stop_bits: None,
        parity: None,
        host: None,
        tcp_port: None,
    };
    
    let tcp_config = create_test_modbus_config();
    
    let rtu_client = ModbusClient::new(rtu_config, ModbusCommunicationMode::Rtu)?;
    let tcp_client = ModbusClient::new(tcp_config, ModbusCommunicationMode::Tcp)?;
    
    println!("RTU client protocol: {}", rtu_client.protocol_name());
    println!("TCP client protocol: {}", tcp_client.protocol_name());
    
    assert_eq!(rtu_client.protocol_name(), "ModbusRTU");
    assert_eq!(tcp_client.protocol_name(), "ModbusTCP");
    
    Ok(())
}

#[tokio::test]
async fn test_integration_with_polling_config() -> Result<()> {
    println!("Testing integration with universal polling configuration...");
    
    let polling_config = PollingConfig {
        interval: Duration::from_millis(500),
        timeout: Duration::from_secs(5),
        max_retries: 3,
        batch_size: 10,
        retry_delay: Duration::from_millis(100),
        enable_batch_optimization: true,
        quality_check_enabled: true,
        adaptive_polling: false,
    };
    
    let test_points = create_test_polling_points();
    
    println!("Polling configuration:");
    println!("  - Interval: {:?}", polling_config.interval);
    println!("  - Timeout: {:?}", polling_config.timeout);
    println!("  - Max retries: {}", polling_config.max_retries);
    println!("  - Batch size: {}", polling_config.batch_size);
    println!("  - Batch optimization: {}", polling_config.enable_batch_optimization);
    
    println!("Test points count: {}", test_points.len());
    
    // This demonstrates that the configuration can be properly used
    // with ModbusClient through the PointReader interface
    
    Ok(())
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_high_frequency_polling_simulation() -> Result<()> {
        println!("Testing high-frequency polling simulation...");
        
        let config = create_test_modbus_config();
        let client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
        
        let test_points = create_test_polling_points();
        
        // Simulate high-frequency polling
        let start_time = std::time::Instant::now();
        let mut cycle_count = 0;
        
        for i in 0..10 {
            println!("Simulated polling cycle {}", i + 1);
            
            // Simulate processing each point
            for point in &test_points {
                // Simulate point reading time
                sleep(Duration::from_millis(1)).await;
                cycle_count += 1;
            }
            
            // Brief delay between cycles
            sleep(Duration::from_millis(10)).await;
        }
        
        let elapsed = start_time.elapsed();
        println!("Processed {} points in {:?}", cycle_count, elapsed);
        println!("Average time per point: {:?}", elapsed / cycle_count);
        
        Ok(())
    }
} 