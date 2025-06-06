/// Channel Logger Demo
/// 
/// This example demonstrates the complete Channel Logger functionality,
/// including creating channel loggers, logging at different levels,
/// and using the ChannelLoggerManager for managing multiple channels.

use comsrv::utils::logger::{
    init_logger, init_channel_logger, ChannelLogger, ChannelLoggerManager, LogLevel
};
use comsrv::utils::error::Result;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Channel Logger Demo ===\n");

    // Create a temporary directory for this demo
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();
    
    println!("1. Setting up main service logger...");
    // Initialize main service logger
    init_logger(&log_dir, "demo_service", "debug", true)?;
    
    println!("2. Creating individual channel loggers...");
    
    // Create channel loggers for different channels
    let modbus_logger = init_channel_logger(&log_dir, "demo_service", "modbus_01", "info")?;
    let iec104_logger = init_channel_logger(&log_dir, "demo_service", "iec104_01", "debug")?;
    let mqtt_logger = init_channel_logger(&log_dir, "demo_service", "mqtt_pub", "warn")?;
    
    println!("3. Testing different log levels...");
    
    // Test Modbus channel (INFO level)
    println!("\n--- Modbus Channel (INFO level) ---");
    modbus_logger.error("Connection failed to device 192.168.1.100");
    modbus_logger.warn("Device response timeout, retrying...");
    modbus_logger.info("Successfully read 10 registers from device");
    modbus_logger.debug("This debug message should NOT appear in INFO level");
    modbus_logger.trace("This trace message should NOT appear in INFO level");
    
    // Test IEC104 channel (DEBUG level)
    println!("\n--- IEC104 Channel (DEBUG level) ---");
    iec104_logger.error("APDU parsing error");
    iec104_logger.warn("Connection unstable, reconnecting...");
    iec104_logger.info("IEC104 connection established");
    iec104_logger.debug("Received ASDU type 36");
    iec104_logger.trace("This trace message should NOT appear in DEBUG level");
    
    // Test MQTT channel (WARN level)
    println!("\n--- MQTT Channel (WARN level) ---");
    mqtt_logger.error("Failed to publish message to broker");
    mqtt_logger.warn("MQTT broker connection lost");
    mqtt_logger.info("This info message should NOT appear in WARN level");
    mqtt_logger.debug("This debug message should NOT appear in WARN level");
    
    println!("4. Testing ChannelLoggerManager...");
    
    // Create a channel logger manager
    let manager = ChannelLoggerManager::new(&log_dir);
    
    // Get loggers through manager
    let managed_logger1 = manager.get_logger("channel_managed_1", LogLevel::Debug)?;
    let managed_logger2 = manager.get_logger("channel_managed_2", LogLevel::Error)?;
    
    // Test managed loggers
    managed_logger1.info("Message from managed channel 1");
    managed_logger2.error("Error from managed channel 2");
    managed_logger2.info("This info should NOT appear in ERROR level");
    
    // List active loggers
    let active_loggers = manager.list_loggers()?;
    println!("\nActive managed loggers: {:?}", active_loggers);
    
    // Remove a logger
    manager.remove_logger("channel_managed_1")?;
    let remaining_loggers = manager.list_loggers()?;
    println!("Remaining loggers after removal: {:?}", remaining_loggers);
    
    println!("5. Testing logger property access...");
    
    // Test logger properties
    println!("Modbus logger channel ID: {}", modbus_logger.channel_id());
    println!("Modbus logger level: {:?}", modbus_logger.level());
    
    // Test level modification
    let mut test_logger = ChannelLogger::new(&log_dir, "test_channel", LogLevel::Info)?;
    println!("Initial level: {:?}", test_logger.level());
    test_logger.set_level(LogLevel::Debug);
    println!("Updated level: {:?}", test_logger.level());
    
    println!("6. Testing log file structure...");
    
    // Show the created directory structure
    println!("\nCreated log directory structure:");
    show_directory_structure(&log_dir, 0);
    
    println!("\n=== Demo completed successfully! ===");
    println!("Check the log files in: {:?}", log_dir);
    println!("Note: Log files are created in the temporary directory and will be cleaned up automatically.");
    
    Ok(())
}

/// Helper function to display directory structure
fn show_directory_structure(dir: &std::path::Path, depth: usize) {
    let indent = "  ".repeat(depth);
    
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();
            
            if path.is_dir() {
                println!("{}📁 {}/", indent, name);
                show_directory_structure(&path, depth + 1);
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                println!("{}📄 {} ({} bytes)", indent, name, size);
            }
        }
    }
} 