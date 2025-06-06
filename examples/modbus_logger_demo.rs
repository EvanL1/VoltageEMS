use std::time::Duration;
use comsrv::core::protocols::modbus::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::modbus::common::{ModbusRegisterMapping, ModbusDataType, ModbusRegisterType, ByteOrder};
use comsrv::utils::logger::{ChannelLogger, LogLevel};
use comsrv::utils::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    println!("=== Modbus Client with ChannelLogger Demo ===");
    
    // Create channel logger
    let log_dir = std::path::Path::new("./logs");
    std::fs::create_dir_all(log_dir).expect("Failed to create log directory");
    
    let channel_logger = ChannelLogger::new(log_dir, "modbus_tcp_demo", LogLevel::Debug)?;
    
    // Configure Modbus client for TCP mode
    let config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Tcp,
        slave_id: 1,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_secs(2),
        point_mappings: vec![
            ModbusRegisterMapping {
                name: "temperature".to_string(),
                display_name: Some("Room Temperature".to_string()),
                description: Some("Temperature sensor reading".to_string()),
                address: 40001,
                register_type: ModbusRegisterType::HoldingRegister,
                data_type: ModbusDataType::Float32,
                scale: 0.1,
                offset: 0.0,
                unit: Some("°C".to_string()),
                byte_order: ByteOrder::BigEndian,
                access_mode: "read".to_string(),
                group: Some("sensors".to_string()),
            },
            ModbusRegisterMapping {
                name: "pressure".to_string(),
                display_name: Some("System Pressure".to_string()),
                description: Some("Pressure sensor reading".to_string()),
                address: 40002,
                register_type: ModbusRegisterType::HoldingRegister,
                data_type: ModbusDataType::UInt16,
                scale: 1.0,
                offset: 0.0,
                unit: Some("Pa".to_string()),
                byte_order: ByteOrder::BigEndian,
                access_mode: "read".to_string(),
                group: Some("sensors".to_string()),
            }
        ],
        host: Some("127.0.0.1".to_string()),
        tcp_port: Some(502),
        ..Default::default()
    };
    
    // Create Modbus client
    let mut client = ModbusClient::new(config, ModbusCommunicationMode::Tcp)?;
    
    // Set the channel logger
    client.set_channel_logger(channel_logger);
    
    println!("Created Modbus TCP client with logger");
    println!("Logger will record all communication frames to: ./logs/modbus_tcp_demo.log");
    
    // Note: In a real environment, you would have a Modbus server running
    // For this demo, we'll just show how the client would work
    
    println!("\n=== Attempting to connect to Modbus server ===");
    match client.start().await {
        Ok(_) => {
            println!("✓ Connected successfully!");
            println!("Client is now running and polling configured points...");
            
            // Let it run for a few seconds to demonstrate polling
            println!("Running for 10 seconds to demonstrate communication logging...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            // Try some manual operations
            println!("\n=== Testing manual read operations ===");
            
            // Try to read a single register
            match client.read_holding_register(40001).await {
                Ok(value) => println!("✓ Read register 40001: {}", value),
                Err(e) => println!("✗ Failed to read register 40001: {}", e),
            }
            
            // Try to read multiple registers
            match client.read_holding_registers(40001, 2).await {
                Ok(values) => println!("✓ Read registers 40001-40002: {:?}", values),
                Err(e) => println!("✗ Failed to read registers 40001-40002: {}", e),
            }
            
            // Try to write a register
            match client.write_single_register(40010, 1234).await {
                Ok(_) => println!("✓ Wrote value 1234 to register 40010"),
                Err(e) => println!("✗ Failed to write register 40010: {}", e),
            }
            
        }
        Err(e) => {
            println!("✗ Failed to connect: {}", e);
            println!("This is expected if no Modbus server is running on localhost:502");
        }
    }
    
    // Stop the client
    println!("\n=== Stopping client ===");
    if let Err(e) = client.stop().await {
        println!("Warning: Error stopping client: {}", e);
    } else {
        println!("✓ Client stopped successfully");
    }
    
    println!("\n=== Demo Complete ===");
    println!("Check the log file at: ./logs/modbus_tcp_demo.log");
    println!("The log should contain detailed records of all Modbus communication attempts,");
    println!("including connection events, read/write operations, and their responses/errors.");
    
    // Show what the log might contain
    println!("\nExample log entries you might see:");
    println!("== [12:34:56.789] Connected to Modbus device (mode: Tcp, slave: 1)");
    println!(">> [12:34:56.790] ReadHolding slave=1 addr=40001 qty=1");
    println!("<< [12:34:56.791] ReadHolding ERR: Connection refused (1ms)");
    println!(">> [12:34:56.792] WriteSingle slave=1 addr=40010 value=1234");
    println!("<< [12:34:56.793] WriteSingle ERR: Connection refused (1ms)");
    println!("== [12:34:56.794] Disconnected from Modbus device");
    
    Ok(())
} 