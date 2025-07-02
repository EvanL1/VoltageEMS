//! Standalone Modbus test runner
//! 
//! Run comprehensive tests for Modbus functionality

use comsrv::modbus_test_runner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
        
    println!("Modbus Communication Test Suite");
    println!("==============================");
    
    // Run all tests
    modbus_test_runner::run_all_tests().await?;
    
    Ok(())
}