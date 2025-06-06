//! Protocol Factory Usage Example
//! 
//! This example demonstrates how to use the integrated protocol factory
//! to create and manage communication channels with different protocols.

use comsrv::core::protocol_factory::{ProtocolFactory, create_default_factory, ProtocolClientFactory};
use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
use comsrv::utils::Result;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Example 1: Create a default factory with built-in protocols
    println!("=== Example 1: Default Factory ===");
    let factory = create_default_factory();
    
    // List supported protocols
    let protocols = factory.supported_protocols();
    println!("Supported protocols: {:?}", protocols);
    
    // Example 2: Create Modbus TCP channel
    println!("\n=== Example 2: Modbus TCP Channel ===");
    let modbus_config = create_modbus_tcp_config(1, "192.168.1.100", 502);
    
    // Validate configuration before creating channel
    match factory.validate_config(&modbus_config) {
        Ok(_) => println!("Modbus TCP configuration is valid"),
        Err(e) => println!("Configuration validation failed: {}", e),
    }
    
    // Create the channel
    factory.create_channel(modbus_config.clone())?;
    println!("Created Modbus TCP channel with ID: {}", modbus_config.id);
    
    // Example 3: Create IEC 104 channel
    println!("\n=== Example 3: IEC 104 Channel ===");
    let iec104_config = create_iec104_config(2, "192.168.1.200", 2404);
    
    factory.validate_config(&iec104_config)?;
    factory.create_channel(iec104_config.clone())?;
    println!("Created IEC 104 channel with ID: {}", iec104_config.id);
    
    // Example 4: Get channel statistics
    println!("\n=== Example 4: Channel Statistics ===");
    let stats = factory.get_channel_stats();
    println!("Total channels: {}", stats.total_channels);
    println!("Protocol distribution: {:?}", stats.protocol_counts);
    
    // Example 5: Access individual channels
    println!("\n=== Example 5: Channel Access ===");
    if let Some(channel) = factory.get_channel(1).await {
        println!("Retrieved channel 1 successfully");
        // You can now use the channel for communication
        // let mut ch = channel.write().await;
        // ch.start().await?;
    }
    
    // Example 6: Get default configurations and schemas
    println!("\n=== Example 6: Configuration Templates ===");
    if let Some(default_config) = factory.get_default_config(&ProtocolType::ModbusTcp) {
        println!("Default Modbus TCP config: {}", default_config.name);
    }
    
    if let Some(schema) = factory.get_config_schema(&ProtocolType::ModbusTcp) {
        println!("Modbus TCP schema has {} properties", 
                schema["properties"].as_object().map(|o| o.len()).unwrap_or(0));
    }
    
    // Example 7: Batch operations
    println!("\n=== Example 7: Batch Operations ===");
    let configs = vec![
        create_modbus_tcp_config(10, "192.168.1.10", 502),
        create_modbus_tcp_config(11, "192.168.1.11", 502),
        create_iec104_config(12, "192.168.1.12", 2404),
    ];
    
    // Create multiple protocols in parallel
    let results = factory.create_protocols_parallel(configs).await;
    println!("Created {} protocols in parallel", results.len());
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(_) => println!("  Protocol {}: OK", i),
            Err(e) => println!("  Protocol {}: Error - {}", i, e),
        }
    }
    
    // Example 8: Start all channels
    println!("\n=== Example 8: Channel Lifecycle ===");
    match factory.start_all_channels().await {
        Ok(_) => println!("All channels started successfully"),
        Err(e) => println!("Failed to start some channels: {}", e),
    }
    
    // Simulate some work
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Stop all channels
    match factory.stop_all_channels().await {
        Ok(_) => println!("All channels stopped successfully"),
        Err(e) => println!("Failed to stop some channels: {}", e),
    }
    
    // Example 9: Cleanup
    println!("\n=== Example 9: Cleanup ===");
    let idle_time = tokio::time::Duration::from_secs(5);
    factory.cleanup_channels(idle_time).await;
    println!("Cleaned up idle channels (idle time > 5s)");
    
    println!("\nFinal channel count: {}", factory.channel_count());
    
    Ok(())
}

/// Create a Modbus TCP channel configuration
fn create_modbus_tcp_config(id: u16, address: &str, port: u16) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("address".to_string(), serde_yaml::Value::String(address.to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(port)));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    
    ChannelConfig {
        id,
        name: format!("Modbus TCP Channel {}", id),
        description: format!("Modbus TCP communication to {}:{}", address, port),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(parameters),
    }
}

/// Create an IEC 104 channel configuration
fn create_iec104_config(id: u16, address: &str, port: u16) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("address".to_string(), serde_yaml::Value::String(address.to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(port)));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    
    ChannelConfig {
        id,
        name: format!("IEC 104 Channel {}", id),
        description: format!("IEC 60870-5-104 communication to {}:{}", address, port),
        protocol: ProtocolType::Iec104,
        parameters: ChannelParameters::Generic(parameters),
    }
}

// Example of custom protocol factory implementation
#[allow(dead_code)]
struct CustomProtocolFactory;

#[allow(dead_code)]
impl ProtocolClientFactory for CustomProtocolFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Virtual  // Using Virtual as example
    }
    
    fn create_client(&self, config: ChannelConfig) -> Result<Box<dyn comsrv::core::protocols::common::ComBase>> {
        // Implementation would go here
        Err(comsrv::utils::ComSrvError::ProtocolNotSupported(
            "Custom protocol not implemented".to_string()
        ))
    }
    
    fn validate_config(&self, _config: &ChannelConfig) -> Result<()> {
        // Custom validation logic
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        ChannelConfig {
            id: 0,
            name: "Custom Protocol Channel".to_string(),
            description: "Custom protocol implementation".to_string(),
            protocol: ProtocolType::Virtual,
            parameters: ChannelParameters::Generic(HashMap::new()),
        }
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "custom_param": {
                    "type": "string",
                    "description": "Custom parameter for the protocol"
                }
            }
        })
    }
}

#[allow(dead_code)]
fn example_custom_factory_registration() -> ProtocolFactory {
    let factory = ProtocolFactory::new();
    
    // Register custom protocol factory
    factory.register_protocol_factory(Arc::new(CustomProtocolFactory));
    
    println!("Registered custom protocol factory");
    factory
} 