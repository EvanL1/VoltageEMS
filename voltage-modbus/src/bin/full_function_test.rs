/// Voltage Modbus Full Function Test Client
/// 
/// Author: Evan Liu <evan.liu@voltageenergy.com>
/// Tests all Modbus function codes including read and write operations

use std::time::Duration;
use voltage_modbus::{ModbusTcpClient, ModbusResult};
use voltage_modbus::client::ModbusClient;

#[tokio::main]
async fn main() -> ModbusResult<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("🧪 Voltage Modbus Full Function Test");
    println!("====================================");
    println!("Testing all standard Modbus function codes");
    println!();

    let address = "127.0.0.1:5020";
    let timeout = Duration::from_millis(3000);
    
    println!("📡 Connecting to server {}...", address);
    let mut client = ModbusTcpClient::from_address(address, timeout).await?;
    println!("✅ Connection successful!");
    println!();

    // Test 1: Read Coils (0x01)
    println!("🧪 Test 1: Read 01 (Function Code 0x01)");
    test_read_01(&mut client).await?;
    
    // Test 2: Read Discrete Inputs (0x02)  
    println!("\n🧪 Test 2: Read 02 (Function Code 0x02)");
    test_read_02(&mut client).await?;
    
    // Test 3: Read Holding Registers (0x03)
    println!("\n🧪 Test 3: Read 03 (Function Code 0x03)");
    test_read_03(&mut client).await?;
    
    // Test 4: Read Input Registers (0x04)
    println!("\n🧪 Test 4: Read 04 (Function Code 0x04)");
    test_read_04(&mut client).await?;
    
    // Test write operations
    println!("🔧 Testing write operations...");
    
    // Test write single register (0x06)
    print!("  Testing write single register (0x06)... ");
    match client.write_06(1, 100, 0x1234).await {
        Ok(_) => println!("✅ Success"),
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    // Test write single coil (0x05)
    print!("  Testing write single coil (0x05)... ");
    match client.write_05(1, 50, true).await {
        Ok(_) => println!("✅ Success"),
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    // Test write multiple registers (0x10)
    print!("  Testing write multiple registers (0x10)... ");
    match client.write_10(1, 200, &[0x1111, 0x2222, 0x3333]).await {
        Ok(_) => println!("✅ Success"),
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    // Test write multiple coils (0x0F)
    print!("  Testing write multiple coils (0x0F)... ");
    match client.write_0f(1, 100, &[true, false, true, false]).await {
        Ok(_) => println!("✅ Success"),
        Err(e) => println!("❌ Failed: {}", e),
    }

    // Get final statistics
    let stats = client.get_stats();
    println!("\n📊 Test Statistics Summary:");
    println!("  Total requests: {}", stats.requests_sent);
    println!("  Total responses: {}", stats.responses_received);
    println!("  Success rate: {:.1}%", (stats.responses_received as f64 / stats.requests_sent as f64) * 100.0);
    println!("  Total errors: {}", stats.errors);
    println!("  Total timeouts: {}", stats.timeouts);
    println!("  Bytes sent: {} bytes", stats.bytes_sent);
    println!("  Bytes received: {} bytes", stats.bytes_received);

    client.close().await?;
    
    println!("\n🎉 All function tests completed!");
    Ok(())
}

async fn test_read_01(client: &mut ModbusTcpClient) -> ModbusResult<()> {
    match client.read_01(1, 0, 10).await {
        Ok(values) => {
            println!("  ✅ Read 01 successful");
            print!("  📊 Coil states (address 0-9): ");
            for (i, value) in values.iter().enumerate() {
                if i < 10 {
                    print!("{} ", if *value { "1" } else { "0" });
                }
            }
            println!();
            println!("  📦 Values count: {}", values.len());
        }
        Err(e) => {
            println!("  ❌ Read 01 failed: {}", e);
        }
    }
    Ok(())
}

async fn test_read_02(client: &mut ModbusTcpClient) -> ModbusResult<()> {
    match client.read_02(1, 0, 8).await {
        Ok(values) => {
            println!("  ✅ Read 02 successful");
            print!("  📊 Input states (address 0-7): ");
            for (i, value) in values.iter().enumerate() {
                if i < 8 {
                    print!("{} ", if *value { "1" } else { "0" });
                }
            }
            println!();
            println!("  📦 Values count: {}", values.len());
        }
        Err(e) => {
            println!("  ❌ Read 02 failed: {}", e);
        }
    }
    Ok(())
}

async fn test_read_03(client: &mut ModbusTcpClient) -> ModbusResult<()> {
    match client.read_03(1, 0, 5).await {
        Ok(values) => {
            println!("  ✅ Read 03 successful");
            print!("  📊 Register values (address 0-4): ");
            for (i, value) in values.iter().enumerate() {
                if i < 5 {
                    print!("0x{:04x} ", value);
                }
            }
            println!();
            println!("  📦 Values count: {}", values.len());
        }
        Err(e) => {
            println!("  ❌ Read 03 failed: {}", e);
        }
    }
    Ok(())
}

async fn test_read_04(client: &mut ModbusTcpClient) -> ModbusResult<()> {
    match client.read_04(1, 0, 3).await {
        Ok(values) => {
            println!("  ✅ Read 04 successful");
            print!("  📊 Register values (address 0-2): ");
            for (i, value) in values.iter().enumerate() {
                if i < 3 {
                    print!("0x{:04x} ", value);
                }
            }
            println!();
            println!("  📦 Values count: {}", values.len());
        }
        Err(e) => {
            println!("  ❌ Read 04 failed: {}", e);
        }
    }
    Ok(())
} 