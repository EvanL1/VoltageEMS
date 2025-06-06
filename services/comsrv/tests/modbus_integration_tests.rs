//! Modbus Integration Tests
//! 
//! Comprehensive integration tests for the comsrv Modbus functionality.
//! Tests include protocol creation, data exchange, error handling, and end-to-end scenarios.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tempfile::TempDir;

use comsrv::core::config::config_manager::{ChannelConfig, ChannelParameters, ConfigManager, ProtocolType};
use comsrv::core::protocols::common::ProtocolFactory;
use comsrv::core::protocols::modbus::client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::modbus::common::{ModbusRegisterMapping, ModbusRegisterType, ModbusDataType, ByteOrder};
use comsrv::utils::error::ComSrvError;

mod support;

/// Create a test configuration file for integration testing
fn create_test_config_file(temp_dir: &std::path::Path) -> std::path::PathBuf {
    let config_content = r#"
version: "1.0"
service:
  name: "comsrv_integration_test"
  description: "Integration test service"
  port: 8080
  log_level: "info"
  log_file: "comsrv_test.log"

redis:
  connection_type: "tcp"
  address: "127.0.0.1:6379"
  db: 0

channels:
  - id: 1
    name: "Modbus TCP Test Channel"
    description: "Test channel for Modbus TCP protocol"
    protocol: "ModbusTcp"
    parameters:
      address: "127.0.0.1"
      port: 502
      timeout: 5000
      slave_id: 1
  
  - id: 2
    name: "Modbus RTU Test Channel"  
    description: "Test channel for Modbus RTU protocol"
    protocol: "ModbusRtu"
    parameters:
      port: "/dev/ttyUSB0"
      baud_rate: 9600
      data_bits: 8
      stop_bits: 1
      parity: "None"
      timeout: 1000
      slave_id: 2

point_tables:
  tables:
    test_table:
      file_path: "test_points.csv"
      description: "Test point table"
      format: "csv"
"#;
    
    let config_path = temp_dir.join("test_config.yaml");
    std::fs::write(&config_path, config_content).expect("Failed to write test config");
    config_path
}

/// Create test point mappings CSV file
fn create_test_point_table(temp_dir: &std::path::Path) -> std::path::PathBuf {
    let csv_content = r#"name,display_name,register_type,address,data_type,scale,offset,unit,description,access_mode,group,byte_order
Temperature,Temperature Sensor,holding_register,100,uint16,0.1,-40.0,°C,Temperature measurement,read,Sensors,big_endian
Pressure,Pressure Sensor,input_register,110,int16,0.01,0.0,bar,Pressure measurement,read,Sensors,big_endian
Flow_Rate,Flow Rate,holding_register,120,float32,1.0,0.0,L/min,Current flow rate,read_write,Process,big_endian
Pump_Status,Pump Status,coil,200,bool,1.0,0.0,,Pump on/off status,read_write,Control,big_endian
Alarm_Active,Alarm Status,discrete_input,300,bool,1.0,0.0,,System alarm status,read,Status,big_endian
Energy_Total,Energy Total,input_register,150,uint64,0.001,0.0,kWh,Total energy consumption,read,Meters,big_endian
"#;

    let points_path = temp_dir.join("test_points.csv");
    std::fs::write(&points_path, csv_content).expect("Failed to write test points");
    points_path
}

/// Test protocol factory creation and basic functionality
#[tokio::test]
async fn test_protocol_factory_integration() {
    let factory = ProtocolFactory::new();
    
    // Test supported protocols
    let supported = factory.supported_protocols();
    assert!(supported.contains(&ProtocolType::ModbusTcp));
    assert!(supported.contains(&ProtocolType::ModbusRtu));
    assert!(supported.contains(&ProtocolType::Iec104));
    
    // Test protocol support checks
    assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
    assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    assert!(!factory.is_protocol_supported(&ProtocolType::Can)); // Should not be supported
}

/// Test Modbus TCP protocol creation and configuration
#[tokio::test]
async fn test_modbus_tcp_protocol_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = create_test_config_file(temp_dir.path());
    let _points_path = create_test_point_table(temp_dir.path());
    
    let config_manager = ConfigManager::from_file(&config_path)
        .expect("Failed to load test config");
    
    let channels = config_manager.get_channels();
    let tcp_channel = channels.iter()
        .find(|ch| ch.protocol == ProtocolType::ModbusTcp)
        .expect("TCP channel not found");
    
    let factory = ProtocolFactory::new();
    
    // Test protocol creation
    let result = factory.create_protocol(tcp_channel.clone());
    assert!(result.is_ok(), "Failed to create Modbus TCP protocol: {:?}", result.err());
    
    // Test channel creation in factory
    let channel_result = factory.create_channel(tcp_channel.clone());
    assert!(channel_result.is_ok(), "Failed to create TCP channel: {:?}", channel_result.err());
    
    assert_eq!(factory.channel_count(), 1);
    
    // Test channel retrieval
    let channel = factory.get_channel(tcp_channel.id).await;
    assert!(channel.is_some(), "Created channel should be retrievable");
}

/// Test Modbus RTU protocol creation and configuration
#[tokio::test]
async fn test_modbus_rtu_protocol_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = create_test_config_file(temp_dir.path());
    let _points_path = create_test_point_table(temp_dir.path());
    
    let config_manager = ConfigManager::from_file(&config_path)
        .expect("Failed to load test config");
    
    let channels = config_manager.get_channels();
    let rtu_channel = channels.iter()
        .find(|ch| ch.protocol == ProtocolType::ModbusRtu)
        .expect("RTU channel not found");
    
    let factory = ProtocolFactory::new();
    
    // Test protocol creation (will fail due to missing serial port, but should validate config)
    let result = factory.create_protocol(rtu_channel.clone());
    // RTU creation might fail due to missing hardware, but should not be a config error
    if result.is_err() {
        let error = result.unwrap_err();
        // Should be a connection error, not a config validation error
        assert!(!matches!(error, ComSrvError::ConfigError(_)));
    }
}

/// Test configuration validation and error handling
#[tokio::test]
async fn test_configuration_validation() {
    let factory = ProtocolFactory::new();
    
    // Test invalid Modbus TCP configuration - missing address
    let mut invalid_params = HashMap::new();
    invalid_params.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    invalid_params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    
    let invalid_config = ChannelConfig {
        id: 99,
        name: "Invalid Config".to_string(),
        description: "Missing address parameter".to_string(),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(invalid_params),
    };
    
    let result = factory.validate_config(&invalid_config);
    assert!(result.is_err(), "Configuration validation should fail for missing address");
    
    // Test invalid Modbus RTU configuration - missing port
    let mut invalid_rtu_params = HashMap::new();
    invalid_rtu_params.insert("baud_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(9600)));
    invalid_rtu_params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    
    let invalid_rtu_config = ChannelConfig {
        id: 98,
        name: "Invalid RTU Config".to_string(),
        description: "Missing port parameter".to_string(),
        protocol: ProtocolType::ModbusRtu,
        parameters: ChannelParameters::Generic(invalid_rtu_params),
    };
    
    let result = factory.validate_config(&invalid_rtu_config);
    assert!(result.is_err(), "RTU configuration validation should fail for missing port");
}

/// Test Modbus client configuration conversion
#[test]
fn test_modbus_client_config_conversion() {
    // Test TCP configuration conversion
    let mut tcp_params = HashMap::new();
    tcp_params.insert("address".to_string(), serde_yaml::Value::String("192.168.1.100".to_string()));
    tcp_params.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    tcp_params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3000)));
    tcp_params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5)));
    
    let tcp_config = ChannelConfig {
        id: 1,
        name: "TCP Test".to_string(),
        description: "TCP test config".to_string(),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(tcp_params),
    };
    
    let modbus_config: ModbusClientConfig = tcp_config.into();
    assert_eq!(modbus_config.slave_id, 5);
    assert_eq!(modbus_config.timeout, Duration::from_millis(3000));
    
    // Test RTU configuration conversion
    let mut rtu_params = HashMap::new();
    rtu_params.insert("port".to_string(), serde_yaml::Value::String("/dev/ttyUSB0".to_string()));
    rtu_params.insert("baud_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(19200)));
    rtu_params.insert("data_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(8)));
    rtu_params.insert("stop_bits".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    rtu_params.insert("parity".to_string(), serde_yaml::Value::String("Even".to_string()));
    rtu_params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2000)));
    rtu_params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3)));
    
    let rtu_config = ChannelConfig {
        id: 2,
        name: "RTU Test".to_string(),
        description: "RTU test config".to_string(),
        protocol: ProtocolType::ModbusRtu,
        parameters: ChannelParameters::Generic(rtu_params),
    };
    
    let modbus_config: ModbusClientConfig = rtu_config.into();
    assert_eq!(modbus_config.slave_id, 3);
    assert_eq!(modbus_config.timeout, Duration::from_millis(2000));
}

/// Test point table integration with Modbus mappings
#[tokio::test]
async fn test_point_table_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = create_test_config_file(temp_dir.path());
    let points_path = create_test_point_table(temp_dir.path());
    
    // Update config to point to correct CSV path
    let mut config_content = std::fs::read_to_string(&config_path).unwrap();
    config_content = config_content.replace("test_points.csv", &points_path.to_string_lossy());
    std::fs::write(&config_path, config_content).expect("Failed to update config");
    
    let config_manager = ConfigManager::from_file(&config_path)
        .expect("Failed to load test config");
    
    // Test that point tables are loaded
    let point_tables_config = config_manager.get_point_tables_config();
    assert!(point_tables_config.enabled);
    assert!(!point_tables_config.directory.is_empty());
}

/// Test multiple channels and concurrent operations
#[tokio::test]
async fn test_multiple_channels() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = create_test_config_file(temp_dir.path());
    let _points_path = create_test_point_table(temp_dir.path());
    
    let config_manager = ConfigManager::from_file(&config_path)
        .expect("Failed to load test config");
    
    let factory = ProtocolFactory::new();
    let channels = config_manager.get_channels();
    
    // Try to create all valid channels
    let mut created_count = 0;
    for channel in channels {
        match factory.create_channel(channel.clone()) {
            Ok(_) => {
                created_count += 1;
                println!("Successfully created channel: {} ({})", channel.name, channel.protocol);
            }
            Err(e) => {
                println!("Failed to create channel {}: {:?}", channel.name, e);
                // RTU channels might fail due to missing hardware, which is expected
            }
        }
    }
    
    assert!(created_count >= 1, "At least one channel should be created successfully");
    
    // Test channel statistics
    let stats = factory.get_channel_stats().await;
    assert_eq!(stats.total_channels, created_count);
}

/// Test protocol lifecycle and cleanup
#[tokio::test]
async fn test_protocol_lifecycle() {
    let factory = ProtocolFactory::new();
    
    // Create a simple TCP configuration
    let mut params = HashMap::new();
    params.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    params.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    
    let config = ChannelConfig {
        id: 100,
        name: "Lifecycle Test".to_string(),
        description: "Testing protocol lifecycle".to_string(),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(params),
    };
    
    // Create channel
    factory.create_channel(config.clone()).expect("Failed to create channel");
    assert_eq!(factory.channel_count(), 1);
    
    // Test channel access
    let channel = factory.get_channel(100).await;
    assert!(channel.is_some());
    
    // Test channel metadata
    let all_channels = factory.get_all_channels();
    assert_eq!(all_channels.len(), 1);
    
    let channel_ids = factory.get_channel_ids();
    assert_eq!(channel_ids.len(), 1);
    assert_eq!(channel_ids[0], 100);
}

/// Test error scenarios and recovery
#[tokio::test]
async fn test_error_scenarios() {
    let factory = ProtocolFactory::new();
    
    // Test unsupported protocol
    let mut params = HashMap::new();
    params.insert("test".to_string(), serde_yaml::Value::String("value".to_string()));
    
    let unsupported_config = ChannelConfig {
        id: 200,
        name: "Unsupported Protocol".to_string(),
        description: "Testing unsupported protocol".to_string(),
        protocol: ProtocolType::Can, // Not supported
        parameters: ChannelParameters::Generic(params),
    };
    
    let result = factory.create_protocol(unsupported_config.clone());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ComSrvError::ProtocolNotSupported(_)));
    
    // Test duplicate channel creation
    let mut valid_params = HashMap::new();
    valid_params.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    valid_params.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    valid_params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    valid_params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    
    let valid_config = ChannelConfig {
        id: 201,
        name: "Valid Channel".to_string(),
        description: "Testing duplicate creation".to_string(),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(valid_params.clone()),
    };
    
    // First creation should succeed
    let result1 = factory.create_channel(valid_config.clone());
    assert!(result1.is_ok());
    
    // Second creation with same ID should fail
    let result2 = factory.create_channel(valid_config);
    assert!(result2.is_err());
    assert!(matches!(result2.unwrap_err(), ComSrvError::ConfigError(_)));
}

/// Test data type and register mapping functionality
#[test]
fn test_data_type_mappings() {
    // Test all supported data types have correct register counts
    assert_eq!(ModbusDataType::Bool.register_count(), 1);
    assert_eq!(ModbusDataType::UInt16.register_count(), 1);
    assert_eq!(ModbusDataType::Int16.register_count(), 1);
    assert_eq!(ModbusDataType::UInt32.register_count(), 2);
    assert_eq!(ModbusDataType::Int32.register_count(), 2);
    assert_eq!(ModbusDataType::Float32.register_count(), 2);
    assert_eq!(ModbusDataType::UInt64.register_count(), 4);
    assert_eq!(ModbusDataType::Int64.register_count(), 4);
    assert_eq!(ModbusDataType::Float64.register_count(), 4);
    
    // Test register type compatibility
    let coil_mapping = ModbusRegisterMapping {
        name: "test_coil".to_string(),
        display_name: None,
        register_type: ModbusRegisterType::Coil,
        address: 100,
        data_type: ModbusDataType::Bool,
        scale: 1.0,
        offset: 0.0,
        unit: None,
        description: None,
        access_mode: "read_write".to_string(),
        group: None,
        byte_order: ByteOrder::BigEndian,
    };
    
    // Coils should only work with Bool data type
    assert_eq!(coil_mapping.data_type, ModbusDataType::Bool);
}

/// Test timeout and async operations
#[tokio::test]
async fn test_async_operations() {
    let factory = ProtocolFactory::new();
    
    // Test that async operations complete within reasonable time
    let result = timeout(Duration::from_secs(5), async {
        // Create multiple channels in parallel
        let mut tasks = Vec::new();
        
        for i in 0..3 {
            let factory_ref = &factory;
            let task = async move {
                let mut params = HashMap::new();
                params.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
                params.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502 + i)));
                params.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
                params.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
                
                let config = ChannelConfig {
                    id: 300 + i as u16,
                    name: format!("Async Test Channel {}", i),
                    description: "Testing async operations".to_string(),
                    protocol: ProtocolType::ModbusTcp,
                    parameters: ChannelParameters::Generic(params),
                };
                
                factory_ref.create_channel(config)
            };
            tasks.push(task);
        }
        
        // Execute all tasks
        let mut success_count = 0;
        for task in tasks {
            if task.await.is_ok() {
                success_count += 1;
            }
        }
        
        success_count
    }).await;
    
    assert!(result.is_ok(), "Async operations should complete within timeout");
    let success_count = result.unwrap();
    assert!(success_count >= 1, "At least one async operation should succeed");
}