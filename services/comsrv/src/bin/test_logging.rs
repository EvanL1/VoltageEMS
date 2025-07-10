//! Test logging functionality
//!
//! This test demonstrates the INFO vs DEBUG logging levels

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with INFO level first
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("=== Logging Test ===");

    #[cfg(feature = "modbus")]
    {
        use comsrv::core::transport::traits::Transport;
        use comsrv::plugins::protocols::modbus::{
            pdu::ModbusPduProcessor,
            tests::mock_transport::{MockTransport, MockTransportConfig},
        };
        use std::time::Duration;

        println!("设置RUST_LOG=info查看INFO级别日志");
        println!("设置RUST_LOG=debug查看DEBUG级别日志");
        println!();

        // Test MockTransport logging
        println!("Testing MockTransport logging...");

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

        // Connect
        println!("1. Connecting...");
        transport.connect().await?;

        // Send request
        println!("2. Sending request...");
        let request = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01];
        let sent_len = transport.send(&request).await?;
        println!("   Sent {} bytes", sent_len);

        // Receive response
        println!("3. Receiving response...");
        let mut buffer = vec![0; 10];
        let received_len = transport
            .receive(&mut buffer, Some(Duration::from_secs(1)))
            .await?;
        println!("   Received {} bytes", received_len);

        println!();
        println!("Testing PDU processing...");

        let processor = ModbusPduProcessor::new();

        // Test PDU parsing
        let response_pdu = vec![0x03, 0x02, 0x12, 0x34];
        match processor.parse_pdu(&response_pdu) {
            Ok(_) => println!("PDU parsing successful"),
            Err(e) => println!("PDU parsing failed: {e}"),
        }

        println!();
        println!("=== 日志测试完成 ===");
        println!("INFO级别应显示: Send/Recv数据包");
        println!("DEBUG级别应显示: 详细的解析过程");
    }

    #[cfg(not(feature = "modbus"))]
    {
        println!("Modbus feature not enabled. Run with --features modbus to test Modbus logging.");
    }

    Ok(())
}
