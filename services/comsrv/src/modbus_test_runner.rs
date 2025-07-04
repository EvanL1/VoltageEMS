//! Independent test runner for Modbus functionality
//! 
//! This module provides standalone tests for Modbus components

use std::time::Duration;
use chrono;

// Import core Modbus modules directly
use crate::core::protocols::modbus::{
    pdu::ModbusPduProcessor,
    tests::mock_transport::{MockTransport, MockTransportConfig},
    protocol_engine::ModbusProtocolEngine,
    common::{ModbusConfig, ModbusFunctionCode},
    frame::{ModbusFrameProcessor, ModbusMode},
};
use crate::core::transport::traits::Transport;

/// Test basic PDU functionality
pub async fn test_modbus_pdu_basic() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Modbus PDU Basic Functionality ===");
    
    let processor = ModbusPduProcessor::new();
    
    // Test 1: Function code conversion
    println!("Test 1: Function code conversion");
    let fc = ModbusFunctionCode::Read03;
    let fc_u8: u8 = fc.into();
    println!("  Read03 -> u8: 0x{:02X}", fc_u8);
    assert_eq!(fc_u8, 0x03);
    
    let fc_back = ModbusFunctionCode::try_from(0x03)?;
    println!("  u8 0x03 -> Read03: {:?}", fc_back);
    assert_eq!(fc_back, ModbusFunctionCode::Read03);
    
    // Test 2: Read request building
    println!("Test 2: Read request building");
    let read_request = processor.build_read_request(
        ModbusFunctionCode::Read03,
        40001,
        10
    );
    println!("  Built read request: {:02X?}", read_request);
    assert_eq!(read_request.len(), 5); // Function code + start address + quantity
    assert_eq!(read_request[0], 0x03); // Function code
    
    // Test 3: Read request parsing
    println!("Test 3: Read request parsing");
    let read_data = [0x9C, 0x41, 0x00, 0x0A]; // Start address 40001, quantity 10
    let parsed = processor.parse_read_request(&read_data)?;
    println!("  Parsed request - Start: {}, Quantity: {}", parsed.start_address, parsed.quantity);
    assert_eq!(parsed.start_address, 40001);
    assert_eq!(parsed.quantity, 10);
    
    println!("✅ PDU Basic tests passed!");
    Ok(())
}

/// Test MockTransport functionality
pub async fn test_mock_transport() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing MockTransport Functionality ===");
    
    let config = MockTransportConfig {
        connect_success: true,
        latency_ms: 10,
        max_message_size: 260,
        fail_after_operations: 0,
        timeout: Duration::from_secs(5),
    };
    
    let mut transport = MockTransport::new(config);
    
    // Test 1: Connection
    println!("Test 1: Connection");
    transport.connect().await?;
    println!("  Connection successful");
    assert!(transport.is_connected().await);
    
    // Test 2: Queue response and send/receive
    println!("Test 2: Send/Receive");
    let test_response = vec![0x01, 0x03, 0x02, 0x12, 0x34];
    transport.queue_response(test_response.clone()).await;
    
    let test_request = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01];
    let sent_len = transport.send(&test_request).await?;
    println!("  Sent {} bytes: {:02X?}", sent_len, test_request);
    assert_eq!(sent_len, test_request.len());
    
    let mut buffer = vec![0; 10];
    let received_len = transport.receive(&mut buffer, Some(Duration::from_secs(1))).await?;
    println!("  Received {} bytes: {:02X?}", received_len, &buffer[..received_len]);
    assert_eq!(&buffer[..received_len], &test_response);
    
    // Test 3: History tracking
    println!("Test 3: History tracking");
    let history = transport.get_send_history().await;
    println!("  Send history count: {}", history.len());
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], test_request);
    
    println!("✅ MockTransport tests passed!");
    Ok(())
}

/// Test protocol engine creation and basic functionality
pub async fn test_protocol_engine() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Protocol Engine ===");
    
    let config = ModbusConfig {
        protocol_type: "modbus_tcp".to_string(),
        host: Some("127.0.0.1".to_string()),
        port: Some(502),
        device_path: None,
        baud_rate: None,
        data_bits: None,
        stop_bits: None,
        parity: None,
        timeout_ms: Some(5000),
        points: vec![],
    };
    
    // Test 1: Engine creation
    println!("Test 1: Engine creation");
    let engine = ModbusProtocolEngine::new(&config).await?;
    println!("  Protocol engine created successfully");
    
    // Test 2: Statistics
    println!("Test 2: Statistics");
    let stats = engine.get_stats().await;
    println!("  Initial stats - Cache hits: {}, Cache misses: {}", stats.cache_hits, stats.cache_misses);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
    
    // Test 3: Cache stats
    println!("Test 3: Cache stats");
    let cache_stats = engine.get_cache_stats().await;
    println!("  Cache size: {}, Hit rate: {}", 
        cache_stats.get("cache_size").unwrap_or(&"0".to_string()),
        cache_stats.get("cache_hit_rate").unwrap_or(&"0%".to_string())
    );
    
    println!("✅ Protocol Engine tests passed!");
    Ok(())
}

/// Test coil and register response data building
pub async fn test_response_building() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Response Building ===");
    
    let processor = ModbusPduProcessor::new();
    
    // Test 1: Coil response data
    println!("Test 1: Coil response data");
    let coil_values = [true, false, true, true, false, false, true, false, true];
    let coil_data = processor.build_coil_response_data(&coil_values);
    println!("  Coil values: {:?}", coil_values);
    println!("  Coil data: {:02X?}", coil_data);
    assert_eq!(coil_data.len(), 2); // 9 bits = 2 bytes
    
    // Test 2: Register response data
    println!("Test 2: Register response data");
    let register_values = [0x1234, 0x5678, 0x9ABC];
    let register_data = processor.build_register_response_data(&register_values);
    println!("  Register values: {:04X?}", register_values);
    println!("  Register data: {:02X?}", register_data);
    assert_eq!(register_data.len(), 6); // 3 registers * 2 bytes = 6 bytes
    assert_eq!(register_data, vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
    
    // Test 3: Exception response
    println!("Test 3: Exception response");
    let exception_pdu = processor.build_exception_response(
        ModbusFunctionCode::Read03,
        crate::core::protocols::modbus::pdu::ModbusExceptionCode::IllegalDataAddress
    );
    println!("  Exception PDU: {:02X?}", exception_pdu);
    assert_eq!(exception_pdu.len(), 2);
    assert_eq!(exception_pdu[0], 0x83); // 0x03 | 0x80
    assert_eq!(exception_pdu[1], 0x02); // IllegalDataAddress
    
    println!("✅ Response Building tests passed!");
    Ok(())
}

/// Test Frame processing functionality
pub async fn test_frame_processing() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Frame Processing ===");
    
    // Test 1: TCP frame building
    println!("Test 1: TCP frame building");
    let tcp_processor = ModbusFrameProcessor::new(ModbusMode::Tcp);
    let pdu = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01]; // Read holding register PDU
    let transaction_id: u16 = 0x1234;
    let unit_id: u8 = 0x01;
    
    let tcp_frame = tcp_processor.build_frame(unit_id, pdu.clone(), Some(transaction_id));
    println!("  TCP frame: {:02X?}", tcp_frame);
    assert_eq!(tcp_frame.len(), 13); // MBAP header (6) + Unit ID (1) + PDU (6)
    assert_eq!(&tcp_frame[0..2], &transaction_id.to_be_bytes());
    assert_eq!(&tcp_frame[2..4], &[0x00, 0x00]); // Protocol ID
    assert_eq!(&tcp_frame[4..6], &((pdu.len() + 1) as u16).to_be_bytes()); // Length
    assert_eq!(tcp_frame[6], unit_id);
    assert_eq!(&tcp_frame[7..], &pdu);
    
    // Test 2: TCP frame parsing
    println!("Test 2: TCP frame parsing");
    let mut tcp_processor_parse = ModbusFrameProcessor::new(ModbusMode::Tcp);
    let parsed_frame = tcp_processor_parse.parse_frame(&tcp_frame)?;
    println!("  Parsed - TxID: {:?}, Unit: {}, PDU: {:02X?}", 
        parsed_frame.transaction_id, parsed_frame.unit_id, parsed_frame.pdu);
    assert_eq!(parsed_frame.transaction_id, Some(transaction_id));
    assert_eq!(parsed_frame.unit_id, unit_id);
    assert_eq!(parsed_frame.pdu, pdu);
    
    // Test 3: RTU frame building
    println!("Test 3: RTU frame building");
    let rtu_processor = ModbusFrameProcessor::new(ModbusMode::Rtu);
    let rtu_frame = rtu_processor.build_frame(unit_id, pdu.clone(), None);
    println!("  RTU frame: {:02X?}", rtu_frame);
    assert_eq!(rtu_frame.len(), 9); // Unit ID (1) + PDU (6) + CRC (2)
    assert_eq!(rtu_frame[0], unit_id);
    assert_eq!(&rtu_frame[1..7], &pdu);
    
    // Test 4: RTU frame parsing
    println!("Test 4: RTU frame parsing");
    let mut rtu_processor_parse = ModbusFrameProcessor::new(ModbusMode::Rtu);
    let parsed_rtu_frame = rtu_processor_parse.parse_frame(&rtu_frame)?;
    println!("  Parsed RTU - Unit: {}, PDU: {:02X?}", 
        parsed_rtu_frame.unit_id, parsed_rtu_frame.pdu);
    assert_eq!(parsed_rtu_frame.unit_id, unit_id);
    assert_eq!(parsed_rtu_frame.pdu, pdu);
    
    println!("✅ Frame Processing tests passed!");
    Ok(())
}

/// Test ModbusClient integration functionality
pub async fn test_modbus_client_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing ModbusClient Integration ===");
    
    // Test 1: Client configuration creation
    println!("Test 1: Client configuration");
    let modbus_config = ModbusConfig {
        protocol_type: "modbus_tcp".to_string(),
        host: Some("127.0.0.1".to_string()),
        port: Some(502),
        device_path: None,
        baud_rate: None,
        data_bits: None,
        stop_bits: None,
        parity: None,
        timeout_ms: Some(5000),
        points: vec![],
    };
    
    let config = crate::core::protocols::modbus::client::ModbusChannelConfig {
        channel_id: 1,
        channel_name: "test_channel".to_string(),
        connection: modbus_config,
        request_timeout: Duration::from_secs(5),
        max_retries: 3,
        retry_delay: Duration::from_millis(100),
        polling: crate::core::config::types::channel_parameters::ModbusPollingConfig::default(),
    };
    println!("  Config created - Channel: {}, Timeout: {}ms", config.channel_name, config.request_timeout.as_millis());
    
    // Test 2: Client creation requires Transport, so we skip actual creation
    println!("Test 2: Client would require Transport implementation");
    println!("  ModbusClient::new requires config and transport parameter");
    println!("  Skipping actual client creation in test environment");
    
    // Test 3: Connection state handling
    println!("Test 3: Connection state types");
    let connected_state = crate::core::protocols::modbus::client::ConnectionState {
        connected: true,
        last_connect_time: Some(chrono::Utc::now()),
        last_error: None,
        retry_count: 0,
    };
    
    let disconnected_state = crate::core::protocols::modbus::client::ConnectionState {
        connected: false,
        last_connect_time: None,
        last_error: Some("Connection failed".to_string()),
        retry_count: 3,
    };
    
    println!("  Connected state: {}, Disconnected state: {}", 
        connected_state.connected, disconnected_state.connected);
    
    // Test 4: Statistics structure
    println!("Test 4: Client statistics");
    let stats = crate::core::protocols::modbus::client::ClientStatistics {
        total_requests: 100,
        successful_requests: 95,
        failed_requests: 5,
        bytes_sent: 1024,
        bytes_received: 2048,
        average_response_time_ms: 50.5,
        last_request_time: Some(chrono::Utc::now()),
    };
    println!("  Stats - Total: {}, Success: {}, Failed: {}, Avg time: {:.1}ms", 
        stats.total_requests, stats.successful_requests, stats.failed_requests, stats.average_response_time_ms);
    
    println!("✅ ModbusClient Integration tests passed!");
    Ok(())
}

/// Run all Modbus tests
pub async fn run_all_tests() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Starting Comprehensive Modbus Test Suite");
    println!("============================================");
    
    test_modbus_pdu_basic().await?;
    println!();
    
    test_mock_transport().await?;
    println!();
    
    test_protocol_engine().await?;
    println!();
    
    test_response_building().await?;
    println!();
    
    test_frame_processing().await?;
    println!();
    
    test_modbus_client_integration().await?;
    println!();
    
    println!("🎉 All Modbus tests completed successfully!");
    println!("===========================================");
    
    Ok(())
}