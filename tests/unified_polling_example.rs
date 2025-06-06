//! Unified Polling Architecture Example
//!
//! This example demonstrates how the universal polling engine works with
//! protocol-specific implementations like ModbusClient, providing a unified
//! interface for data collection across different communication protocols.

use std::collections::HashMap;
use std::time::Duration;

use comsrv::core::protocols::modbus::client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::common::combase::{
    PointReader, PollingPoint, PollingConfig, PollingStats, PointData
};
use comsrv::utils::error::Result;
use serde_json::Value;

/// Example: Multi-protocol data collection setup
///
/// This example shows how different protocol clients (Modbus, IEC60870, CAN, etc.)
/// can be integrated into a unified polling system.
#[tokio::test]
async fn test_unified_polling_architecture_example() -> Result<()> {
    println!("=== Unified Polling Architecture Example ===\n");
    
    // 1. Create protocol-specific clients
    println!("1. Creating protocol clients...");
    
    // Modbus TCP client for industrial PLCs
    let modbus_tcp_config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Tcp,
        slave_id: 1,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_millis(500),
        point_mappings: Vec::new(),
        port: None,
        baud_rate: None,
        data_bits: None,
        stop_bits: None,
        parity: None,
        host: Some("192.168.1.100".to_string()),
        tcp_port: Some(502),
    };
    
    // Modbus RTU client for field devices
    let modbus_rtu_config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Rtu,
        slave_id: 2,
        timeout: Duration::from_secs(3),
        max_retries: 2,
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
    
    let modbus_tcp_client = ModbusClient::new(modbus_tcp_config, ModbusCommunicationMode::Tcp)?;
    let modbus_rtu_client = ModbusClient::new(modbus_rtu_config, ModbusCommunicationMode::Rtu)?;
    
    println!("  ✓ Modbus TCP client: {}", modbus_tcp_client.protocol_name());
    println!("  ✓ Modbus RTU client: {}", modbus_rtu_client.protocol_name());
    
    // 2. Define data points for different systems
    println!("\n2. Defining data points...");
    
    let plc_points = create_plc_data_points();
    let field_device_points = create_field_device_points();
    
    println!("  ✓ PLC data points: {} points", plc_points.len());
    println!("  ✓ Field device points: {} points", field_device_points.len());
    
    // 3. Configure universal polling settings
    println!("\n3. Configuring universal polling...");
    
    let high_frequency_config = PollingConfig {
        interval: Duration::from_millis(100),  // High-frequency for critical data
        timeout: Duration::from_secs(2),
        max_retries: 2,
        batch_size: 20,
        retry_delay: Duration::from_millis(50),
        enable_batch_optimization: true,
        quality_check_enabled: true,
        adaptive_polling: true,
    };
    
    let normal_frequency_config = PollingConfig {
        interval: Duration::from_secs(1),      // Normal frequency for non-critical data
        timeout: Duration::from_secs(5),
        max_retries: 3,
        batch_size: 10,
        retry_delay: Duration::from_millis(100),
        enable_batch_optimization: true,
        quality_check_enabled: true,
        adaptive_polling: false,
    };
    
    println!("  ✓ High-frequency config: {:?} interval", high_frequency_config.interval);
    println!("  ✓ Normal-frequency config: {:?} interval", normal_frequency_config.interval);
    
    // 4. Demonstrate unified interface
    println!("\n4. Demonstrating unified interface...");
    
    // Both clients implement the same PointReader trait
    demonstrate_unified_interface(&modbus_tcp_client, &plc_points[0..2]).await?;
    demonstrate_unified_interface(&modbus_rtu_client, &field_device_points[0..2]).await?;
    
    // 5. Show batch processing capabilities
    println!("\n5. Demonstrating batch processing...");
    
    demonstrate_batch_processing(&modbus_tcp_client, &plc_points).await?;
    
    // 6. Protocol-specific optimizations
    println!("\n6. Protocol-specific optimizations...");
    
    demonstrate_protocol_optimizations(&modbus_tcp_client, &modbus_rtu_client).await?;
    
    println!("\n=== Example completed successfully ===");
    
    Ok(())
}

/// Create data points for PLC systems (high-frequency, critical data)
fn create_plc_data_points() -> Vec<PollingPoint> {
    vec![
        PollingPoint {
            id: "reactor_temperature".to_string(),
            address: 40001,
            data_type: "float32".to_string(),
            scale: 0.1,
            offset: -273.15,  // Convert to Celsius
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
            id: "reactor_pressure".to_string(),
            address: 40003,
            data_type: "uint32".to_string(),
            scale: 0.01,      // Convert to bar
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
            id: "emergency_stop".to_string(),
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
        PollingPoint {
            id: "production_rate".to_string(),
            address: 40005,
            data_type: "uint16".to_string(),
            scale: 1.0,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(1.into()));
                params.insert("register_type".to_string(), Value::String("holding".to_string()));
                params
            },
        },
    ]
}

/// Create data points for field devices (normal frequency, monitoring data)
fn create_field_device_points() -> Vec<PollingPoint> {
    vec![
        PollingPoint {
            id: "ambient_temperature".to_string(),
            address: 30001,
            data_type: "int16".to_string(),
            scale: 0.1,
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(2.into()));
                params.insert("register_type".to_string(), Value::String("input".to_string()));
                params.insert("byte_order".to_string(), Value::String("big_endian".to_string()));
                params
            },
        },
        PollingPoint {
            id: "humidity_level".to_string(),
            address: 30002,
            data_type: "uint16".to_string(),
            scale: 0.01,      // Convert to percentage
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(2.into()));
                params.insert("register_type".to_string(), Value::String("input".to_string()));
                params
            },
        },
        PollingPoint {
            id: "power_consumption".to_string(),
            address: 30003,
            data_type: "uint32".to_string(),
            scale: 0.001,     // Convert to kW
            offset: 0.0,
            protocol_params: {
                let mut params = HashMap::new();
                params.insert("slave_id".to_string(), Value::Number(2.into()));
                params.insert("register_type".to_string(), Value::String("input".to_string()));
                params.insert("quantity".to_string(), Value::Number(2.into()));
                params
            },
        },
    ]
}

/// Demonstrate how different protocol clients can be used through the same interface
async fn demonstrate_unified_interface(
    client: &ModbusClient,
    points: &[PollingPoint],
) -> Result<()> {
    println!("  Protocol: {}", client.protocol_name());
    println!("  Connection status: {}", client.is_connected().await);
    
    for point in points {
        println!("    Point: {} ({}:{}, type: {})", 
                 point.id, 
                 point.protocol_params.get("slave_id").unwrap_or(&Value::Null),
                 point.address,
                 point.data_type);
    }
    
    Ok(())
}

/// Demonstrate batch processing with protocol-specific optimizations
async fn demonstrate_batch_processing(
    client: &ModbusClient,
    points: &[PollingPoint],
) -> Result<()> {
    println!("  Protocol: {}", client.protocol_name());
    println!("  Total points for batch processing: {}", points.len());
    
    // Group points by optimization criteria
    let mut holding_register_points = Vec::new();
    let mut input_register_points = Vec::new();
    let mut coil_points = Vec::new();
    
    for point in points {
        match point.protocol_params.get("register_type") {
            Some(Value::String(reg_type)) => {
                match reg_type.as_str() {
                    "holding" => holding_register_points.push(point),
                    "input" => input_register_points.push(point),
                    "coil" => coil_points.push(point),
                    _ => {},
                }
            }
            _ => {},
        }
    }
    
    println!("    Holding registers: {} points", holding_register_points.len());
    println!("    Input registers: {} points", input_register_points.len());
    println!("    Coils: {} points", coil_points.len());
    
    // In real implementation, these would be optimized into batch reads
    println!("    Batch optimization: Ready for consecutive register reads");
    
    Ok(())
}

/// Show protocol-specific optimizations and differences
async fn demonstrate_protocol_optimizations(
    tcp_client: &ModbusClient,
    rtu_client: &ModbusClient,
) -> Result<()> {
    println!("  TCP client optimizations:");
    println!("    - High-speed concurrent connections");
    println!("    - Large batch reads (up to 125 registers)");
    println!("    - Connection pooling support");
    println!("    - Network error recovery");
    
    println!("  RTU client optimizations:");
    println!("    - Serial timing management");
    println!("    - CRC error detection and retry");
    println!("    - Baud rate adaptation");
    println!("    - Inter-frame delay handling");
    
    println!("  Both clients provide:");
    println!("    - Unified PointReader interface");
    println!("    - Automatic data type conversion");
    println!("    - Quality assessment");
    println!("    - Configurable retry logic");
    
    Ok(())
}

/// Example of creating a polling configuration for different scenarios
#[tokio::test]
async fn test_polling_configuration_scenarios() -> Result<()> {
    println!("=== Polling Configuration Scenarios ===\n");
    
    // Critical system monitoring (high frequency, low tolerance for errors)
    let critical_config = PollingConfig {
        interval: Duration::from_millis(50),   // 20 Hz
        timeout: Duration::from_millis(500),
        max_retries: 1,                        // Fast fail for critical systems
        batch_size: 5,                         // Small batches for responsiveness
        retry_delay: Duration::from_millis(10),
        enable_batch_optimization: false,      // Disable for lowest latency
        quality_check_enabled: true,
        adaptive_polling: true,                // Adapt to system load
    };
    
    // Normal monitoring (balanced performance)
    let normal_config = PollingConfig {
        interval: Duration::from_millis(500),  // 2 Hz
        timeout: Duration::from_secs(2),
        max_retries: 3,
        batch_size: 20,
        retry_delay: Duration::from_millis(100),
        enable_batch_optimization: true,
        quality_check_enabled: true,
        adaptive_polling: false,
    };
    
    // Historical data collection (low frequency, high throughput)
    let historical_config = PollingConfig {
        interval: Duration::from_secs(60),     // 1 minute
        timeout: Duration::from_secs(10),
        max_retries: 5,                        // High tolerance for temporary failures
        batch_size: 100,                       // Large batches for efficiency
        retry_delay: Duration::from_millis(500),
        enable_batch_optimization: true,
        quality_check_enabled: false,          // Accept lower quality for historical data
        adaptive_polling: false,
    };
    
    println!("Critical system polling:");
    print_polling_config(&critical_config);
    
    println!("\nNormal monitoring polling:");
    print_polling_config(&normal_config);
    
    println!("\nHistorical data collection:");
    print_polling_config(&historical_config);
    
    Ok(())
}

fn print_polling_config(config: &PollingConfig) {
    println!("  Interval: {:?}", config.interval);
    println!("  Timeout: {:?}", config.timeout);
    println!("  Max retries: {}", config.max_retries);
    println!("  Batch size: {}", config.batch_size);
    println!("  Batch optimization: {}", config.enable_batch_optimization);
    println!("  Quality checks: {}", config.quality_check_enabled);
    println!("  Adaptive polling: {}", config.adaptive_polling);
} 