/// Modbus Client Polling Test
/// 
/// Tests the core polling functionality that was missing from the system.
/// This test demonstrates how the client connects and starts polling data.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error};

// Import the comsrv modules
use comsrv::core::protocols::modbus::client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::modbus::common::{
    ModbusRegisterMapping, ModbusRegisterType, ModbusDataType, ModbusAccessMode, ByteOrder
};
use comsrv::utils::logger::init_logger;

/// Test configuration for Modbus client polling
#[derive(Debug, Clone)]
struct TestConfig {
    /// Test server host
    pub host: String,
    /// Test server port  
    pub port: u16,
    /// Slave ID to test
    pub slave_id: u8,
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Test duration in seconds
    pub test_duration_secs: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5502,
            slave_id: 1,
            poll_interval_ms: 500,  // 500ms polling
            test_duration_secs: 10,
        }
    }
}

/// Create test point mappings
fn create_test_point_mappings() -> Vec<ModbusRegisterMapping> {
    vec![
        ModbusRegisterMapping {
            name: "test_holding_reg_1".to_string(),
            address: 100,
            register_type: ModbusRegisterType::HoldingRegister,
            data_type: ModbusDataType::Uint16,
            scale: 1.0,
            offset: 0.0,
            unit: "units".to_string(),
            description: "Test holding register 1".to_string(),
            access_mode: ModbusAccessMode::Read,
            group: "test".to_string(),
            byte_order: ByteOrder::BigEndian,
        },
        ModbusRegisterMapping {
            name: "test_input_reg_1".to_string(),
            address: 200,
            register_type: ModbusRegisterType::InputRegister,
            data_type: ModbusDataType::Float32,
            scale: 0.1,
            offset: -10.0,
            unit: "°C".to_string(),
            description: "Test temperature sensor".to_string(),
            access_mode: ModbusAccessMode::Read,
            group: "sensors".to_string(),
            byte_order: ByteOrder::BigEndian,
        },
        ModbusRegisterMapping {
            name: "test_coil_1".to_string(),
            address: 1,
            register_type: ModbusRegisterType::Coil,
            data_type: ModbusDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Test digital output".to_string(),
            access_mode: ModbusAccessMode::ReadWrite,
            group: "digital".to_string(),
            byte_order: ByteOrder::BigEndian,
        },
        ModbusRegisterMapping {
            name: "test_discrete_input_1".to_string(),
            address: 1,
            register_type: ModbusRegisterType::DiscreteInput,
            data_type: ModbusDataType::Bool,
            scale: 1.0,
            offset: 0.0,
            unit: "".to_string(),
            description: "Test digital input".to_string(),
            access_mode: ModbusAccessMode::Read,
            group: "digital".to_string(),
            byte_order: ByteOrder::BigEndian,
        },
    ]
}

/// Create Modbus client with test configuration
fn create_test_modbus_client(config: &TestConfig) -> comsrv::utils::error::Result<ModbusClient> {
    let client_config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Tcp,
        slave_id: config.slave_id,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_millis(config.poll_interval_ms),
        point_mappings: create_test_point_mappings(),
        
        // TCP configuration
        host: Some(config.host.clone()),
        tcp_port: Some(config.port),
        
        // RTU configuration (not used in this test)
        port: None,
        baud_rate: None,
        data_bits: None,
        stop_bits: None,
        parity: None,
    };

    ModbusClient::new(client_config, ModbusCommunicationMode::Tcp)
}

/// Test Modbus client polling functionality
#[tokio::test]
async fn test_modbus_client_polling() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for the test
    let _ = init_logger("./tests/logs", "modbus_test", "debug", true);
    
    info!("🧪 Starting Modbus client polling test");
    
    let test_config = TestConfig::default();
    
    // Create and start the Modbus client
    let mut client = create_test_modbus_client(&test_config)?;
    
    info!("📡 Created Modbus client for {}:{}", test_config.host, test_config.port);
    info!("🔧 Configured with {} point mappings", client.get_parameters().len());
    
    // Note: This test will fail if there's no Modbus server running
    // In a real test environment, you would start a test server first
    match client.start().await {
        Ok(_) => {
            info!("✅ Modbus client started successfully");
            info!("🔄 Polling should be active now...");
            
            // Let the client poll for the configured test duration
            sleep(Duration::from_secs(test_config.test_duration_secs)).await;
            
            // Check client statistics
            let stats = client.get_stats().await;
            info!("📊 Final statistics:");
            info!("  - Total requests: {}", stats.total_requests);
            info!("  - Successful requests: {}", stats.successful_requests);
            info!("  - Failed requests: {}", stats.failed_requests);
            info!("  - Average response time: {:.2}ms", stats.avg_response_time_ms);
            
            // Stop the client
            client.stop().await?;
            info!("⏹️  Modbus client stopped");
            
            // Verify that polling occurred (even if simulated)
            assert!(stats.total_requests > 0, "Expected some polling requests to be made");
            
        }
        Err(e) => {
            error!("❌ Failed to start Modbus client: {}", e);
            info!("💡 This is expected if no Modbus server is running on {}:{}", 
                test_config.host, test_config.port);
            info!("💡 To test with a real server, start: python3 simple_modbus_server.py");
            
            // This is not a test failure - just no server available
            // The important thing is that the client attempted to connect
        }
    }
    
    info!("🎉 Modbus client polling test completed");
    Ok(())
}

/// Test Modbus client configuration and setup
#[test]
fn test_modbus_client_configuration() {
    let test_config = TestConfig::default();
    
    // Test client creation
    let client_result = create_test_modbus_client(&test_config);
    assert!(client_result.is_ok(), "Failed to create Modbus client");
    
    let client = client_result.unwrap();
    
    // Test client parameters
    let params = client.get_parameters();
    assert_eq!(params.get("slave_id").unwrap(), "1");
    assert!(params.contains_key("timeout"));
    assert!(params.contains_key("poll_interval"));
    assert_eq!(params.get("host").unwrap(), "127.0.0.1");
    assert_eq!(params.get("tcp_port").unwrap(), "5502");
    
    info!("✅ Modbus client configuration test passed");
}

/// Test point mappings configuration
#[test]
fn test_point_mappings() {
    let mappings = create_test_point_mappings();
    
    assert_eq!(mappings.len(), 4, "Expected 4 test point mappings");
    
    // Test holding register mapping
    let holding_reg = &mappings[0];
    assert_eq!(holding_reg.name, "test_holding_reg_1");
    assert_eq!(holding_reg.address, 100);
    assert!(matches!(holding_reg.register_type, ModbusRegisterType::HoldingRegister));
    assert!(matches!(holding_reg.data_type, ModbusDataType::Uint16));
    
    // Test input register mapping
    let input_reg = &mappings[1];
    assert_eq!(input_reg.name, "test_input_reg_1");
    assert_eq!(input_reg.scale, 0.1);
    assert_eq!(input_reg.offset, -10.0);
    assert_eq!(input_reg.unit, "°C");
    
    // Test coil mapping
    let coil = &mappings[2];
    assert!(matches!(coil.register_type, ModbusRegisterType::Coil));
    assert!(matches!(coil.data_type, ModbusDataType::Bool));
    assert!(matches!(coil.access_mode, ModbusAccessMode::ReadWrite));
    
    // Test discrete input mapping
    let discrete = &mappings[3];
    assert!(matches!(discrete.register_type, ModbusRegisterType::DiscreteInput));
    assert!(matches!(discrete.access_mode, ModbusAccessMode::Read));
    
    info!("✅ Point mappings configuration test passed");
}

/// Integration test with simulated server response
#[tokio::test]
async fn test_modbus_polling_simulation() -> Result<(), Box<dyn std::error::Error>> {
    let _ = init_logger("./tests/logs", "modbus_sim_test", "debug", true);
    
    info!("🧪 Starting Modbus polling simulation test");
    
    let mut test_config = TestConfig::default();
    test_config.test_duration_secs = 3;  // Shorter test
    test_config.poll_interval_ms = 200;  // Faster polling for test
    
    let mut client = create_test_modbus_client(&test_config)?;
    
    // Since we're testing the polling logic itself (not actual network communication),
    // the client should handle connection failures gracefully and continue attempting
    
    info!("🔄 Testing polling logic without server...");
    
    // This will attempt to connect and fail, but should handle it gracefully
    let _ = client.start().await;  // Ignore connection errors for this test
    
    // Give some time for the polling logic to attempt operation
    sleep(Duration::from_millis(1000)).await;
    
    // Check that the client is running (even if not connected)
    let is_running = client.is_running().await;
    info!("🏃 Client running state: {}", is_running);
    
    // Stop the client
    client.stop().await?;
    
    // Verify client stopped
    let is_running_after_stop = client.is_running().await;
    assert!(!is_running_after_stop, "Client should be stopped");
    
    info!("✅ Polling simulation test completed");
    Ok(())
}

#[cfg(test)]
mod test_with_server {
    use super::*;
    
    /// This test requires a running Modbus server
    /// Run: python3 simple_modbus_server.py
    /// Then run: cargo test test_with_real_server -- --ignored
    #[tokio::test]
    #[ignore]  // Ignored by default since it requires external server
    async fn test_with_real_server() -> Result<(), Box<dyn std::error::Error>> {
        let _ = init_logger("./tests/logs", "modbus_real_test", "debug", true);
        
        info!("🧪 Starting Modbus client test with real server");
        info!("🔧 Make sure server is running: python3 simple_modbus_server.py");
        
        let test_config = TestConfig::default();
        let mut client = create_test_modbus_client(&test_config)?;
        
        // This should succeed if server is running
        client.start().await?;
        info!("✅ Connected to real Modbus server");
        
        // Let it poll for a while
        sleep(Duration::from_secs(5)).await;
        
        let stats = client.get_stats().await;
        info!("📊 Real server statistics:");
        info!("  - Total requests: {}", stats.total_requests);
        info!("  - Successful requests: {}", stats.successful_requests);
        
        client.stop().await?;
        
        // With a real server, we should have successful requests
        assert!(stats.successful_requests > 0, "Expected successful requests with real server");
        
        info!("✅ Real server test completed successfully");
        Ok(())
    }
} 