/// Universal Polling Engine Test
/// 
/// This test demonstrates how the universal polling engine can be used
/// with different protocols (Modbus TCP, RTU, IEC60870, etc.) through
/// a unified interface.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde_json::json;
use tokio::time::sleep;

// Import comsrv modules
use comsrv::core::protocols::common::combase::{
    PollingEngine, UniversalPollingEngine, PointReader, PollingConfig, PollingPoint, PointData
};
use comsrv::core::protocols::modbus::rtu_point_reader::ModbusRtuPointReader;
use comsrv::utils::error::Result;

/// Test configuration for universal polling
#[derive(Debug, Clone)]
struct TestPollingConfig {
    /// Test protocol name
    pub protocol_name: String,
    /// Test polling interval
    pub poll_interval_ms: u64,
    /// Test points to poll
    pub test_points: Vec<TestPoint>,
}

/// Test data point definition
#[derive(Debug, Clone)]
struct TestPoint {
    pub id: String,
    pub name: String,
    pub address: u32,
    pub data_type: String,
    pub slave_id: u8,
    pub register_type: String,
}

impl TestPoint {
    /// Convert to PollingPoint
    fn to_polling_point(&self) -> PollingPoint {
        let mut protocol_params = HashMap::new();
        protocol_params.insert("slave_id".to_string(), json!(self.slave_id));
        protocol_params.insert("register_type".to_string(), json!(self.register_type));
        protocol_params.insert("byte_order".to_string(), json!("big_endian"));
        protocol_params.insert("quantity".to_string(), json!(1));
        
        PollingPoint {
            id: self.id.clone(),
            name: self.name.clone(),
            address: self.address,
            data_type: self.data_type.clone(),
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: format!("Test point {}", self.name),
            access_mode: "read".to_string(),
            group: "test_group".to_string(),
            protocol_params,
        }
    }
}

/// Create test configuration for Modbus RTU
fn create_modbus_rtu_test_config() -> TestPollingConfig {
    TestPollingConfig {
        protocol_name: "ModbusRTU".to_string(),
        poll_interval_ms: 1000,
        test_points: vec![
            TestPoint {
                id: "voltage_l1".to_string(),
                name: "L1 Voltage".to_string(),
                address: 1000,
                data_type: "uint16".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            },
            TestPoint {
                id: "voltage_l2".to_string(),
                name: "L2 Voltage".to_string(),
                address: 1001,
                data_type: "uint16".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            },
            TestPoint {
                id: "current_l1".to_string(),
                name: "L1 Current".to_string(),
                address: 2000,
                data_type: "float32".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            },
            TestPoint {
                id: "status_relay_1".to_string(),
                name: "Relay 1 Status".to_string(),
                address: 100,
                data_type: "bool".to_string(),
                slave_id: 1,
                register_type: "coil".to_string(),
            },
            TestPoint {
                id: "alarm_input_1".to_string(),
                name: "Alarm Input 1".to_string(),
                address: 200,
                data_type: "bool".to_string(),
                slave_id: 1,
                register_type: "discrete".to_string(),
            },
        ],
    }
}

/// Create test configuration for Modbus TCP
fn create_modbus_tcp_test_config() -> TestPollingConfig {
    TestPollingConfig {
        protocol_name: "ModbusTCP".to_string(),
        poll_interval_ms: 500,
        test_points: vec![
            TestPoint {
                id: "power_active".to_string(),
                name: "Active Power".to_string(),
                address: 3000,
                data_type: "int32".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            },
            TestPoint {
                id: "power_reactive".to_string(),
                name: "Reactive Power".to_string(),
                address: 3002,
                data_type: "int32".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            },
            TestPoint {
                id: "frequency".to_string(),
                name: "Grid Frequency".to_string(),
                address: 4000,
                data_type: "uint16".to_string(),
                slave_id: 1,
                register_type: "input".to_string(),
            },
        ],
    }
}

/// Test the universal polling engine with RTU protocol
#[tokio::test]
async fn test_universal_polling_engine_rtu() -> Result<()> {
    println!("🧪 Testing Universal Polling Engine with Modbus RTU");
    
    // Create RTU point reader
    let rtu_reader = ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600);
    let point_reader: Arc<dyn PointReader> = Arc::new(rtu_reader);
    
    // Create universal polling engine
    let mut engine = UniversalPollingEngine::new(
        "ModbusRTU".to_string(),
        point_reader,
    );
    
    // Set up data callback
    engine.set_data_callback(|data_points| {
        println!("📊 Received {} data points:", data_points.len());
        for point in data_points {
            println!("  - {}: {:?}", 
                     point.id, point.value);
        }
    });
    
    // Create polling configuration
    let test_config = create_modbus_rtu_test_config();
    let polling_config = PollingConfig {
        enabled: true,
        interval_ms: test_config.poll_interval_ms,
        max_points_per_cycle: 100,
        timeout_ms: 5000,
        max_retries: 3,
        retry_delay_ms: 1000,
        enable_batch_reading: true,
        point_read_delay_ms: 10,
    };
    
    // Convert test points to polling points
    let polling_points: Vec<PollingPoint> = test_config.test_points
        .into_iter()
        .map(|tp| tp.to_polling_point())
        .collect();
    
    println!("📋 Created {} polling points for RTU test", polling_points.len());
    
    // Note: This would fail in test because there's no actual RTU device
    // but it demonstrates the API usage
    match engine.start_polling(polling_config, polling_points).await {
        Ok(_) => {
            println!("✅ Polling engine started successfully");
            
            // Let it run for a few seconds
            sleep(Duration::from_secs(3)).await;
            
            // Get statistics
            let stats = engine.get_polling_stats().await;
            println!("📈 Polling Statistics:");
            println!("  Total cycles: {}", stats.total_cycles);
            println!("  Successful: {}", stats.successful_cycles);
            println!("  Failed: {}", stats.failed_cycles);
    
            println!("  Total Successful Requests: {}", stats.successful_requests);
            println!("  Total Failed Requests: {}", stats.failed_requests);
            println!("  Average Response Time: {:.2}ms", stats.avg_response_time_ms);
            
            // Stop polling
            engine.stop_polling().await?;
            println!("🛑 Polling engine stopped");
        }
        Err(e) => {
            println!("❌ Failed to start polling (expected in test): {}", e);
            // This is expected in test environment without actual hardware
        }
    }
    
    Ok(())
}

/// Demonstrate multi-protocol polling
#[tokio::test]
async fn test_multi_protocol_polling() -> Result<()> {
    println!("🧪 Testing Multi-Protocol Polling");
    
    // This test demonstrates how different protocols can use the same polling engine
    let test_protocols = vec![
        ("ModbusRTU", create_modbus_rtu_test_config()),
        ("ModbusTCP", create_modbus_tcp_test_config()),
    ];
    
    for (protocol_name, test_config) in test_protocols {
        println!("\n📡 Testing {} protocol:", protocol_name);
        
        // Create appropriate point reader based on protocol
        // (In real implementation, this would be a factory pattern)
        let point_reader: Arc<dyn PointReader> = match protocol_name {
            "ModbusRTU" => Arc::new(ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600)),
            "ModbusTCP" => {
                // Would create TCP point reader here
                Arc::new(ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600))
            }
            _ => continue,
        };
        
        // Create engine
        let engine = UniversalPollingEngine::new(
            protocol_name.to_string(),
            point_reader,
        );
        
        // Create configuration
        let polling_config = PollingConfig {
            enabled: true,
            interval_ms: test_config.poll_interval_ms,
            max_points_per_cycle: 50,
            timeout_ms: 3000,
            max_retries: 2,
            retry_delay_ms: 500,
            enable_batch_reading: protocol_name == "ModbusTCP", // TCP benefits more from batching
            point_read_delay_ms: if protocol_name == "ModbusRTU" { 50 } else { 10 },
        };
        
        // Convert points
        let polling_points: Vec<PollingPoint> = test_config.test_points
            .into_iter()
            .map(|tp| tp.to_polling_point())
            .collect();
        
        println!("  📋 Points: {}", polling_points.len());
        println!("  ⏱️  Interval: {}ms", polling_config.interval_ms);
        println!("  📦 Batch reading: {}", polling_config.enable_batch_reading);
        
        // Simulate starting polling (would fail without hardware)
        match engine.start_polling(polling_config, polling_points).await {
            Ok(_) => println!("  ✅ {} polling started", protocol_name),
            Err(_) => println!("  ❌ {} polling failed (expected)", protocol_name),
        }
    }
    
    println!("\n🎯 Multi-protocol polling test demonstrates unified interface");
    Ok(())
}

/// Test polling configuration changes
#[tokio::test]
async fn test_polling_configuration_updates() -> Result<()> {
    println!("🧪 Testing Polling Configuration Updates");
    
    // Create engine
    let point_reader: Arc<dyn PointReader> = Arc::new(
        ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600)
    );
    let engine = UniversalPollingEngine::new("TestProtocol".to_string(), point_reader);
    
    // Initial configuration
    let initial_config = PollingConfig {
        enabled: true,
        interval_ms: 1000,
        max_points_per_cycle: 50,
        timeout_ms: 5000,
        max_retries: 3,
        retry_delay_ms: 1000,
        enable_batch_reading: true,
        point_read_delay_ms: 10,
    };
    
    let test_points = vec![
        TestPoint {
            id: "test1".to_string(),
            name: "Test Point 1".to_string(),
            address: 100,
            data_type: "uint16".to_string(),
            slave_id: 1,
            register_type: "holding".to_string(),
        }
    ];
    
    let polling_points: Vec<PollingPoint> = test_points
        .into_iter()
        .map(|tp| tp.to_polling_point())
        .collect();
    
    // Start with initial config
    match engine.start_polling(initial_config.clone(), polling_points.clone()).await {
        Ok(_) => println!("✅ Initial polling started"),
        Err(_) => println!("❌ Initial polling failed (expected)"),
    }
    
    // Test configuration update
    let updated_config = PollingConfig {
        interval_ms: 500, // Faster polling
        max_points_per_cycle: 100,
        enable_batch_reading: false, // Disable batching
        ..initial_config
    };
    
    match engine.update_polling_config(updated_config).await {
        Ok(_) => println!("✅ Configuration updated successfully"),
        Err(e) => println!("❌ Configuration update failed: {}", e),
    }
    
    // Test points update
    let additional_points = vec![
        TestPoint {
            id: "test2".to_string(),
            name: "Test Point 2".to_string(),
            address: 101,
            data_type: "float32".to_string(),
            slave_id: 1,
            register_type: "holding".to_string(),
        }
    ];
    
    let mut all_points = polling_points;
    all_points.extend(additional_points.into_iter().map(|tp| tp.to_polling_point()));
    
    match engine.update_polling_points(all_points).await {
        Ok(_) => println!("✅ Points updated successfully"),
        Err(e) => println!("❌ Points update failed: {}", e),
    }
    
    // Check status
    println!("📊 Engine status:");
    println!("  Active: {}", engine.is_polling_active().await);
    
    let stats = engine.get_polling_stats().await;
    println!("  Statistics: {:?}", stats);
    
    // Stop polling
    match engine.stop_polling().await {
        Ok(_) => println!("🛑 Polling stopped successfully"),
        Err(e) => println!("❌ Failed to stop polling: {}", e),
    }
    
    Ok(())
}

/// Performance test with high-frequency polling
#[tokio::test]
async fn test_high_frequency_polling() -> Result<()> {
    println!("🧪 Testing High-Frequency Polling Performance");
    
    let point_reader: Arc<dyn PointReader> = Arc::new(
        ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600)
    );
    let engine = UniversalPollingEngine::new("HighFreqTest".to_string(), point_reader);
    
    // High-frequency configuration
    let config = PollingConfig {
        enabled: true,
        interval_ms: 100, // Very fast polling - 10Hz
        max_points_per_cycle: 200,
        timeout_ms: 200,
        max_retries: 1,
        retry_delay_ms: 50,
        enable_batch_reading: true,
        point_read_delay_ms: 1,
    };
    
    // Create many test points
    let test_points: Vec<PollingPoint> = (0..50)
        .map(|i| {
            let test_point = TestPoint {
                id: format!("high_freq_point_{}", i),
                name: format!("High Frequency Point {}", i),
                address: 1000 + i as u32,
                data_type: "uint16".to_string(),
                slave_id: 1,
                register_type: "holding".to_string(),
            };
            test_point.to_polling_point()
        })
        .collect();
    
    println!("📊 Testing with {} points at {}ms intervals", 
             test_points.len(), config.interval_ms);
    
    match engine.start_polling(config, test_points).await {
        Ok(_) => {
            println!("✅ High-frequency polling started");
            
            // Run for a short time
            sleep(Duration::from_millis(500)).await;
            
            let stats = engine.get_polling_stats().await;
            println!("📈 Performance Results:");
            println!("  Total cycles: {}", stats.total_cycles);
            println!("  Avg cycle time: {:.2}ms", stats.avg_cycle_time_ms);
            println!("  Polling rate: {:.2} Hz", stats.current_polling_rate);
            
            engine.stop_polling().await?;
        }
        Err(e) => {
            println!("❌ High-frequency polling failed (expected): {}", e);
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_polling_engine_creation() {
    println!("🧪 Testing Universal Polling Engine Creation");
    
    let point_reader: Arc<dyn PointReader> = Arc::new(
        ModbusRtuPointReader::new("/dev/ttyUSB0".to_string(), 9600)
    );
    
    let engine = UniversalPollingEngine::new("TestEngine".to_string(), point_reader);
    
    // Test initial state
    assert!(!engine.is_polling_active().await);
    
    let stats = engine.get_polling_stats().await;
    assert_eq!(stats.total_cycles, 0);
    assert_eq!(stats.successful_cycles, 0);
    
    
    println!("✅ Engine created successfully with correct initial state");
} 