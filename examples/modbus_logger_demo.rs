use std::time::Duration;
use voltage_modbus::{
    client::{ModbusTcpClient, ModbusClient},
    logging::{CallbackLogger, LogLevel},
    ModbusResult,
};

#[tokio::main]
async fn main() -> ModbusResult<()> {
    println!("🚀 Modbus Logger Demo with Callback System");
    println!("===========================================\n");

    // Create a custom logger with detailed packet information
    let packet_logger = CallbackLogger::new(
        Some(Box::new(|level, message| {
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
            match level {
                LogLevel::Error => eprintln!("[{}] 🔴 ERROR: {}", timestamp, message),
                LogLevel::Warn => eprintln!("[{}] 🟡 WARN: {}", timestamp, message),
                LogLevel::Info => {
                    if message.contains("Request") {
                        println!("[{}] 📤 {}", timestamp, message);
                    } else if message.contains("Response") {
                        println!("[{}] 📥 {}", timestamp, message);
                    } else {
                        println!("[{}] 📋 {}", timestamp, message);
                    }
                }
                LogLevel::Debug => println!("[{}] 🔍 DEBUG: {}", timestamp, message),
            }
        })),
        LogLevel::Info,
    );

    // Create client with callback logging
    let mut client = ModbusTcpClient::with_logging(
        "127.0.0.1:502", 
        Duration::from_secs(5), 
        Some(packet_logger)
    ).await?;

    println!("🔗 Connected to Modbus server at 127.0.0.1:502\n");

    // Test read operations with detailed logging
    println!("📖 Testing read operations...\n");

    // Read holding registers (function code 0x03)
    match client.read_03(1, 40001, 2).await {
        Ok(values) => {
            println!("✅ Successfully read registers: {:?}\n", values);
        }
        Err(e) => {
            println!("❌ Failed to read registers: {}\n", e);
        }
    }

    // Write single register (function code 0x06)
    println!("✏️ Testing write operations...\n");
    
    match client.write_06(1, 40010, 1234).await {
        Ok(_) => {
            println!("✅ Successfully wrote register\n");
        }
        Err(e) => {
            println!("❌ Failed to write register: {}\n", e);
        }
    }

    // Read coils (function code 0x01)
    match client.read_01(1, 1, 8).await {
        Ok(coils) => {
            println!("✅ Successfully read coils: {:?}\n", coils);
        }
        Err(e) => {
            println!("❌ Failed to read coils: {}\n", e);
        }
    }

    // Write single coil (function code 0x05)
    match client.write_05(1, 1, true).await {
        Ok(_) => {
            println!("✅ Successfully wrote coil\n");
        }
        Err(e) => {
            println!("❌ Failed to write coil: {}\n", e);
        }
    }

    // Write multiple registers (function code 0x10)
    match client.write_10(1, 40020, &[100, 200, 300]).await {
        Ok(_) => {
            println!("✅ Successfully wrote multiple registers\n");
        }
        Err(e) => {
            println!("❌ Failed to write multiple registers: {}\n", e);
        }
    }

    println!("🎉 Demo completed! Check the detailed packet logs above.");

    Ok(())
} 