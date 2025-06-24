/// Example: Using ProtocolLogger trait for unified logging across protocols
/// 
/// This example demonstrates how to use the new ProtocolLogger trait to achieve
/// the same logging functionality as ChannelLogger but with better integration
/// into the ComBase architecture and using standard env_logger.

use std::time::Instant;
use comsrv::core::protocols::common::combase::ProtocolLogger;
use comsrv::utils::error::Result;

/// Example Modbus client that implements ProtocolLogger
pub struct ExampleModbusClient {
    channel_id: String,
    slave_id: u8,
}

impl ExampleModbusClient {
    pub fn new(channel_id: String, slave_id: u8) -> Self {
        Self { channel_id, slave_id }
    }
    
    /// Simulate connection with unified logging
    pub async fn connect(&self) -> Result<()> {
        self.log_connection("connecting", Some("192.168.1.100:502")).await;
        
        // Simulate connection delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        self.log_connection("connected", None).await;
        Ok(())
    }
    
    /// Simulate reading holding register with unified logging
    pub async fn read_holding_register(&self, address: u16) -> Result<u16> {
        let start_time = Instant::now();
        let details = format!("slave={} addr={} qty=1", self.slave_id, address);
        
        self.log_request("ReadHolding", &details).await;
        
        // Simulate modbus read operation
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        let value = 12345; // Simulated value
        let result: Result<u16> = Ok(value);
        
        self.log_operation_result("ReadHolding", "<<", &details, &result, start_time).await;
        
        result
    }
    
    /// Simulate error case with unified logging
    pub async fn read_with_error(&self, address: u16) -> Result<u16> {
        let start_time = Instant::now();
        let details = format!("slave={} addr={} qty=1", self.slave_id, address);
        
        self.log_request("ReadHolding", &details).await;
        
        // Simulate timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        let result: Result<u16> = Err(comsrv::utils::error::ComSrvError::TimeoutError("Connection timeout".to_string()));
        
        self.log_operation_result("ReadHolding", "<<", &details, &result, start_time).await;
        
        result
    }
    
    /// Simulate batch data sync with unified logging
    pub async fn sync_to_redis(&self, point_count: usize) -> Result<()> {
        let result: Result<()> = if point_count > 0 {
            // Simulate successful sync
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            Ok(())
        } else {
            Err(comsrv::utils::error::ComSrvError::ConfigurationError("No points to sync".to_string()))
        };
        
        self.log_data_sync_result("redis_sync", point_count, &result).await;
        
        result
    }
    
    /// Simulate reconnection with unified logging
    pub async fn reconnect(&self) -> Result<()> {
        self.log_connection("reconnecting", Some("Connection lost")).await;
        
        // Simulate reconnection attempt
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        self.log_connection("reconnected", None).await;
        
        Ok(())
    }
}

impl ProtocolLogger for ExampleModbusClient {
    fn channel_id(&self) -> String {
        self.channel_id.clone()
    }
    
    fn protocol_type(&self) -> &str {
        "modbus"
    }
}

/// Example IEC60870 client that implements ProtocolLogger
pub struct ExampleIecClient {
    channel_id: String,
    station_address: u16,
}

impl ExampleIecClient {
    pub fn new(channel_id: String, station_address: u16) -> Self {
        Self { channel_id, station_address }
    }
    
    pub async fn connect(&self) -> Result<()> {
        self.log_connection("connecting", Some("IEC60870-5-104")).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        self.log_connection("connected", None).await;
        Ok(())
    }
    
    pub async fn read_measurement(&self, ioa: u32) -> Result<f64> {
        let start_time = Instant::now();
        let details = format!("station={} ioa={}", self.station_address, ioa);
        
        self.log_request("ReadMeasurement", &details).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        
        let value = 123.45; // Simulated measurement
        let result: Result<f64> = Ok(value);
        
        self.log_operation_result("ReadMeasurement", "<<", &details, &result, start_time).await;
        
        result
    }
}

impl ProtocolLogger for ExampleIecClient {
    fn channel_id(&self) -> String {
        self.channel_id.clone()
    }
    
    fn protocol_type(&self) -> &str {
        "iec60870"
    }
}

/// Initialize env_logger for protocol-aware logging
pub fn init_protocol_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .format_target(true)  // Show the target (protocol::channel) in logs
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Unified Protocol Logging Demo ===\n");
    
    // Initialize logger
    init_protocol_logger();
    
    // Create different protocol clients
    let modbus_client = ExampleModbusClient::new("modbus_001".to_string(), 1);
    let iec_client = ExampleIecClient::new("iec_001".to_string(), 100);
    
    println!("1. Testing Modbus connection and operations...");
    modbus_client.connect().await?;
    let _ = modbus_client.read_holding_register(100).await?;
    let _ = modbus_client.read_with_error(999).await.ok();
    modbus_client.sync_to_redis(25).await?;
    modbus_client.reconnect().await?;
    
    println!("\n2. Testing IEC60870 connection and operations...");
    iec_client.connect().await?;
    let _ = iec_client.read_measurement(1001).await?;
    
    println!("\n3. Testing failed sync...");
    modbus_client.sync_to_redis(0).await.ok();
    
    println!("\n=== Demo completed! ===");
    println!("All logs above use the unified ProtocolLogger trait.");
    println!("\nTo filter logs by protocol and channel:");
    println!("RUST_LOG=\"modbus::channel::modbus_001=debug\" ./your_program");
    println!("RUST_LOG=\"iec60870::channel::iec_001=info\" ./your_program");
    println!("RUST_LOG=\"modbus=debug,iec60870=info\" ./your_program");
    
    Ok(())
} 