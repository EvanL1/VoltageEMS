//! Test DEBUG level logging specifically
//! 
//! This test shows the difference between INFO and DEBUG logging levels

use comsrv::core::protocols::modbus::{
    tests::mock_transport::{MockTransport, MockTransportConfig},
    pdu::ModbusPduProcessor,
};
use comsrv::core::transport::traits::Transport;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with DEBUG level
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();
        
    println!("=== Testing DEBUG Level Logging ===");
    println!("You should see both INFO and DEBUG level logs below:");
    println!();
    
    // Test MockTransport with actual data processing
    let config = MockTransportConfig {
        connect_success: true,
        latency_ms: 0,
        max_message_size: 260,
        fail_after_operations: 0,
        timeout: Duration::from_secs(5),
    };
    
    let mut transport = MockTransport::new(config);
    
    // Queue some response data
    let response_data = vec![0x01, 0x03, 0x02, 0x12, 0x34];
    transport.queue_response(response_data).await;
    
    // Connect and send data (this will trigger INFO logs)
    transport.connect().await?;
    let request = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01];
    transport.send(&request).await?;
    
    let mut buffer = vec![0; 10];
    transport.receive(&mut buffer, Some(Duration::from_secs(1))).await?;
    
    println!();
    println!("Now testing PDU parsing (this should trigger DEBUG logs):");
    
    // Test PDU processing (this should trigger DEBUG logs)
    let processor = ModbusPduProcessor::new();
    
    // Parse a simple PDU
    let pdu_data = vec![0x03, 0x02, 0x12, 0x34]; // Function code + response data
    match processor.parse_pdu(&pdu_data) {
        Ok(result) => println!("PDU parsing succeeded: {:?}", result),
        Err(e) => println!("PDU parsing failed: {}", e),
    }
    
    // Parse a read request
    let read_request_data = vec![0x00, 0x01, 0x00, 0x0A]; // Start address 1, quantity 10
    match processor.parse_read_request(&read_request_data) {
        Ok(request) => println!("Read request parsed: start={}, quantity={}", request.start_address, request.quantity),
        Err(e) => println!("Read request parsing failed: {}", e),
    }
    
    println!();
    println!("=== Logging Test Complete ===");
    println!("Expected behavior:");
    println!("- INFO logs: Only raw packet data (Send/Recv)");
    println!("- DEBUG logs: Detailed parsing process with function codes, addresses, etc.");
    
    Ok(())
}