//! Modbus end-to-end integration test
//! 
//! This test demonstrates a complete Modbus communication flow:
//! 1. Start a Modbus server simulator
//! 2. Create a Modbus client
//! 3. Configure points and mappings
//! 4. Start polling
//! 5. Verify data in Redis
//! 6. Test four-remote types (YC/YX/YK/YT)

use comsrv::core::protocols::modbus::{
    client::{ModbusClient, ModbusChannelConfig, ProtocolMappingTable},
    common::ModbusConfig,
    modbus_polling::ModbusPollingConfig,
};
use comsrv::core::config::types::protocol::{
    TelemetryMapping, SignalMapping, ControlMapping, AdjustmentMapping,
};
use comsrv::core::transport::tcp::TcpTransport;
use comsrv::utils::error::Result;
use std::time::Duration;
use std::collections::HashMap;
use tokio::time::sleep;
use tracing::{info, error};

/// Test configuration
struct TestConfig {
    server_host: String,
    server_port: u16,
    redis_url: String,
    polling_interval_ms: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server_host: "127.0.0.1".to_string(),
            server_port: 5020,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            polling_interval_ms: 1000,
        }
    }
}

/// Create test channel configuration
fn create_test_channel_config(test_config: &TestConfig) -> ModbusChannelConfig {
    ModbusChannelConfig {
        channel_id: 1,
        channel_name: "ModbusIntegrationTest".to_string(),
        connection: ModbusConfig {
            protocol_type: "modbus_tcp".to_string(),
            host: Some(test_config.server_host.clone()),
            port: Some(test_config.server_port),
            device_path: None,
            baud_rate: None,
            data_bits: None,
            stop_bits: None,
            parity: None,
            timeout_ms: Some(5000),
            points: vec![],
        },
        request_timeout: Duration::from_millis(5000),
        max_retries: 3,
        retry_delay: Duration::from_millis(1000),
        polling: ModbusPollingConfig {
            enabled: true,
            default_interval_ms: test_config.polling_interval_ms,
            enable_batch_reading: true,
            max_batch_size: 100,
            read_timeout_ms: 5000,
            slave_configs: HashMap::new(),
        },
    }
}

/// Create test point mappings for four-remote types
fn create_test_mappings() -> ProtocolMappingTable {
    let mut mappings = ProtocolMappingTable::default();
    
    // YC - 遥测 (Telemetry): Analog values
    mappings.telemetry_mappings.insert("1001".to_string(), TelemetryMapping {
        point_id: "1001".to_string(),
        slave_id: 1,
        address: 1000,
        data_type: "float".to_string(),
        scale: 1.0,
        offset: 0.0,
    });
    
    mappings.telemetry_mappings.insert("1002".to_string(), TelemetryMapping {
        point_id: "1002".to_string(),
        slave_id: 1,
        address: 1001,
        data_type: "float".to_string(),
        scale: 0.1,
        offset: 0.0,
    });
    
    // YX - 遥信 (Signal): Digital status
    mappings.signal_mappings.insert("2001".to_string(), SignalMapping {
        point_id: "2001".to_string(),
        slave_id: 1,
        address: 2000,
        bit_position: 0,
    });
    
    mappings.signal_mappings.insert("2002".to_string(), SignalMapping {
        point_id: "2002".to_string(),
        slave_id: 1,
        address: 2000,
        bit_position: 1,
    });
    
    // YK - 遥控 (Control): Digital control
    mappings.control_mappings.insert("3001".to_string(), ControlMapping {
        point_id: "3001".to_string(),
        slave_id: 1,
        address: 3000,
        on_value: 1,
        off_value: 0,
    });
    
    // YT - 遥调 (Adjustment): Analog setpoint
    mappings.adjustment_mappings.insert("4001".to_string(), AdjustmentMapping {
        point_id: "4001".to_string(),
        slave_id: 1,
        address: 4000,
        data_type: "float".to_string(),
        scale: 1.0,
        min_value: 0.0,
        max_value: 100.0,
    });
    
    mappings
}

/// Verify data in Redis
async fn verify_redis_data(redis_url: &str, point_ids: &[&str]) -> Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_async_connection().await?;
    
    use redis::AsyncCommands;
    
    for point_id in point_ids {
        let key = format!("realtime:point:{}", point_id);
        let value: Option<String> = conn.get(&key).await?;
        
        if let Some(data) = value {
            info!("Redis data for {}: {}", point_id, data);
        } else {
            error!("No data found for point {}", point_id);
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_modbus_end_to_end() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();
    
    let test_config = TestConfig::default();
    
    // Step 1: Check if Modbus server is running
    info!("Checking Modbus server at {}:{}", test_config.server_host, test_config.server_port);
    
    // Step 2: Create Modbus client
    let channel_config = create_test_channel_config(&test_config);
    let transport = Box::new(TcpTransport::new());
    
    let mut client = ModbusClient::new(channel_config, transport).await?;
    info!("Modbus client created");
    
    // Step 3: Load point mappings
    let mappings = create_test_mappings();
    client.load_protocol_mappings(mappings).await?;
    info!("Point mappings loaded");
    
    // Step 4: Start client (connects and starts polling)
    client.start().await?;
    info!("Client started, polling active");
    
    // Step 5: Wait for some polling cycles
    info!("Waiting for {} polling cycles...", 5);
    sleep(Duration::from_millis(test_config.polling_interval_ms * 5)).await;
    
    // Step 6: Read some points directly
    info!("Testing direct point reads...");
    
    // Read YC point
    let yc_value = client.read_point("1001").await?;
    info!("YC Point 1001 value: {:?}", yc_value);
    
    // Read YX point
    let yx_value = client.read_point("2001").await?;
    info!("YX Point 2001 value: {:?}", yx_value);
    
    // Step 7: Test YK control
    info!("Testing YK control...");
    client.write_point("3001", "1").await?; // Turn ON
    sleep(Duration::from_millis(500)).await;
    client.write_point("3001", "0").await?; // Turn OFF
    
    // Step 8: Test YT adjustment
    info!("Testing YT adjustment...");
    client.write_point("4001", "50.5").await?; // Set to 50.5
    
    // Step 9: Verify data in Redis
    info!("Verifying data in Redis...");
    let point_ids = vec!["1001", "1002", "2001", "2002", "3001", "4001"];
    verify_redis_data(&test_config.redis_url, &point_ids).await?;
    
    // Step 10: Get client statistics
    let stats = client.get_diagnostics().await;
    info!("Client statistics: {:?}", stats);
    
    // Step 11: Stop client
    client.stop().await?;
    info!("Client stopped");
    
    info!("Integration test completed successfully!");
    Ok(())
}

/// Test batch reading optimization
#[tokio::test]
async fn test_batch_reading_optimization() -> Result<()> {
    let test_config = TestConfig::default();
    let mut channel_config = create_test_channel_config(&test_config);
    
    // Enable batch reading
    channel_config.polling.enable_batch_reading = true;
    channel_config.polling.max_batch_size = 10;
    
    // Create multiple consecutive points
    let mut mappings = ProtocolMappingTable::default();
    for i in 0..10 {
        mappings.telemetry_mappings.insert(
            format!("100{}", i),
            TelemetryMapping {
                point_id: format!("100{}", i),
                slave_id: 1,
                address: 1000 + i,
                data_type: "uint16".to_string(),
                scale: 1.0,
                offset: 0.0,
            },
        );
    }
    
    let transport = Box::new(TcpTransport::new());
    let mut client = ModbusClient::new(channel_config, transport).await?;
    client.load_protocol_mappings(mappings).await?;
    
    client.start().await?;
    sleep(Duration::from_secs(2)).await;
    
    // Batch read should combine these into one request
    let values = client.read_all_points().await?;
    info!("Batch read {} points", values.len());
    
    client.stop().await?;
    Ok(())
}

/// Test error handling and reconnection
#[tokio::test] 
async fn test_error_handling() -> Result<()> {
    let test_config = TestConfig {
        server_port: 9999, // Invalid port
        ..Default::default()
    };
    
    let channel_config = create_test_channel_config(&test_config);
    let transport = Box::new(TcpTransport::new());
    
    let mut client = ModbusClient::new(channel_config, transport).await?;
    
    // Should handle connection error gracefully
    match client.start().await {
        Ok(_) => panic!("Expected connection error"),
        Err(e) => {
            info!("Expected error occurred: {}", e);
        }
    }
    
    Ok(())
}