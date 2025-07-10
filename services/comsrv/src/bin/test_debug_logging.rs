//! Test DEBUG level logging specifically
//!
//! This test shows the difference between INFO and DEBUG logging levels

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with DEBUG level
    tracing_subscriber::fmt().with_env_filter("debug").init();

    println!("=== Testing DEBUG Level Logging ===");

    #[cfg(feature = "modbus")]
    {
        use comsrv::core::transport::traits::Transport;
        use comsrv::plugins::protocols::modbus::{
            pdu::ModbusPduProcessor,
            tests::mock_transport::{MockTransport, MockTransportConfig},
        };
        use std::time::Duration;

        println!("You should see both INFO and DEBUG level logs below:");
        println!();

        // Create mock transport with configuration
        let config = MockTransportConfig {
            connect_success: true,
            latency_ms: 0,
            max_message_size: 260,
            fail_after_operations: 0,
            timeout: Duration::from_secs(5),
        };

        let mut transport = MockTransport::new(config);

        // Queue multiple responses for testing
        let responses = vec![
            vec![0x01, 0x03, 0x02, 0x12, 0x34], // Read holding registers response
            vec![0x01, 0x06, 0x00, 0x01, 0x00, 0x02], // Write single register response
        ];
        transport.queue_responses(responses).await;

        // Connect - should show DEBUG logs
        println!("=== Connect Test ===");
        transport.connect().await?;
        println!();

        // Send/Receive - should show both INFO (hex data) and DEBUG (parsing details)
        println!("=== Send/Receive Test ===");
        let request1 = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01];
        transport.send(&request1).await?;

        let mut buffer = vec![0; 10];
        let len = transport
            .receive(&mut buffer, Some(Duration::from_secs(1)))
            .await?;
        println!("Received {} bytes", len);
        println!();

        // Test PDU processing with DEBUG logging
        println!("=== PDU Processing Test ===");
        let processor = ModbusPduProcessor::new();

        // Build a read request
        let read_pdu = processor.build_read_request(
            comsrv::plugins::protocols::modbus::common::ModbusFunctionCode::Read03,
            0x0001,
            0x0002,
        );
        println!("Built PDU: {:02X?}", read_pdu);

        // Parse response
        let response_pdu = vec![0x03, 0x04, 0x00, 0x01, 0x00, 0x02];
        match processor.parse_pdu(&response_pdu) {
            Ok(result) => println!("PDU parsed successfully: {:?}", result),
            Err(e) => println!("PDU parsing error: {}", e),
        }
        println!();

        println!("=== Test Complete ===");
        println!("DEBUG level logs show:");
        println!("- Detailed connection process");
        println!("- Packet parsing details");
        println!("- Internal state changes");
    }

    #[cfg(not(feature = "modbus"))]
    {
        println!("Modbus feature not enabled. Run with --features modbus to test debug logging.");
    }

    Ok(())
}
