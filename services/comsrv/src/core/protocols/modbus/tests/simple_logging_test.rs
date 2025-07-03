//! 简化的 Modbus 日志记录测试
//! 
//! 测试基本的传输层和 PDU 解析日志功能

use std::time::Duration;
use tokio;
use tracing::{info, debug};
use tracing_test::traced_test;

use crate::core::protocols::modbus::{
    tests::mock_transport::{MockTransport, MockTransportConfig},
    pdu::{ModbusPduProcessor, ModbusFunctionCode},
};
use crate::core::transport::traits::Transport;

#[tokio::test]
#[traced_test]
async fn test_mock_transport_logging() {
    debug!("Starting MockTransport logging functionality test");
    
    // 创建 MockTransport
    let config = MockTransportConfig {
        connect_success: true,
        latency_ms: 10,
        max_message_size: 260,
        fail_after_operations: 0,
        timeout: Duration::from_secs(5),
    };
    
    let mut mock_transport = MockTransport::new(config);
    
    // 准备响应数据
    let response_data = vec![0x01, 0x03, 0x02, 0x12, 0x34]; // 模拟 Modbus 响应
    mock_transport.queue_response(response_data).await;
    
    // Test connection logging
    debug!("Testing connection operation logging");
    match mock_transport.connect().await {
        Ok(_) => debug!("Connection successful"),
        Err(e) => debug!("Connection failed: {:?}", e),
    }
    
    // Test send logging
    debug!("Testing send operation logging");
    let test_data = vec![0x01, 0x03, 0x00, 0x01, 0x00, 0x01]; // 模拟 Modbus 请求
    match mock_transport.send(&test_data).await {
        Ok(len) => debug!("Send successful, length: {} bytes", len),
        Err(e) => debug!("Send failed: {:?}", e),
    }
    
    // Test receive logging
    debug!("Testing receive operation logging");
    let mut buffer = vec![0; 10];
    match mock_transport.receive(&mut buffer, Some(Duration::from_secs(1))).await {
        Ok(len) => {
            debug!("Receive successful, length: {} bytes", len);
            debug!("Received data: {:02X?}", &buffer[..len]);
        }
        Err(e) => debug!("Receive failed: {:?}", e),
    }
    
    debug!("MockTransport logging test completed");
}

#[tokio::test]
#[traced_test] 
async fn test_pdu_processor_logging() {
    debug!("Starting PDU processor logging functionality test");
    
    let processor = ModbusPduProcessor::new();
    
    // Test PDU construction logging
    debug!("Testing PDU construction logging");
    let read_request = processor.build_read_request(
        ModbusFunctionCode::Read03,
        40001,
        10
    );
    debug!("Built read request PDU: {:02X?}", read_request);
    
    // Test PDU parsing logging
    debug!("Testing PDU parsing logging");
    let response_pdu = vec![0x03, 0x04, 0x12, 0x34, 0x56, 0x78]; // 功能码03 + 4字节数据
    match processor.parse_pdu(&response_pdu) {
        Ok(result) => {
            debug!("PDU parsing successful");
            debug!("Parse result: {:?}", result);
        }
        Err(e) => {
            debug!("PDU parsing failed: {}", e);
        }
    }
    
    // Test exception PDU parsing logging
    debug!("Testing exception PDU parsing logging");
    let exception_pdu = vec![0x83, 0x02]; // 功能码03+0x80 + 异常码02
    match processor.parse_pdu(&exception_pdu) {
        Ok(result) => {
            debug!("Exception PDU parsing successful");
            debug!("Exception parse result: {:?}", result);
        }
        Err(e) => {
            debug!("Exception PDU parsing failed: {}", e);
        }
    }
    
    // Test read request parsing logging
    debug!("Testing read request parsing logging");
    let read_req_data = vec![0x00, 0x01, 0x00, 0x0A]; // 起始地址1, 数量10
    match processor.parse_read_request(&read_req_data) {
        Ok(request) => {
            debug!("Read request parsing successful");
            debug!("Read request: start_address={}, quantity={}", request.start_address, request.quantity);
        }
        Err(e) => {
            debug!("Read request parsing failed: {}", e);
        }
    }
    
    debug!("PDU processor logging test completed");
}

#[tokio::test]
#[traced_test]
async fn test_comprehensive_packet_logging() {
    debug!("Starting comprehensive packet exchange logging test");
    
    // 这个测试模拟完整的 Modbus 通信过程
    let config = MockTransportConfig::default();
    let mut mock_transport = MockTransport::new(config);
    let processor = ModbusPduProcessor::new();
    
    // Step 1: Connection
    debug!("Step 1: Establishing connection");
    mock_transport.connect().await.expect("Connection failed");
    
    // Step 2: Build request PDU
    debug!("Step 2: Building request PDU");
    let request_pdu = processor.build_read_request(
        ModbusFunctionCode::Read03,
        40001,
        5
    );
    debug!("Request PDU details: {:02X?}", request_pdu);
    
    // Step 3: Prepare mock response
    debug!("Step 3: Preparing mock response data");
    let response_data = vec![
        0x03,           // 功能码03
        0x0A,           // 字节计数10
        0x12, 0x34,     // 寄存器1: 0x1234
        0x56, 0x78,     // 寄存器2: 0x5678
        0x9A, 0xBC,     // 寄存器3: 0x9ABC
        0xDE, 0xF0,     // 寄存器4: 0xDEF0
        0x11, 0x22,     // 寄存器5: 0x1122
    ];
    mock_transport.queue_response(response_data.clone()).await;
    
    // Step 4: Send request
    debug!("Step 4: Sending request data");
    match mock_transport.send(&request_pdu).await {
        Ok(sent_len) => {
            debug!("Request sent successfully: {} bytes", sent_len);
        }
        Err(e) => {
            debug!("Request send failed: {:?}", e);
            return;
        }
    }
    
    // Step 5: Receive response
    debug!("Step 5: Receiving response data");
    let mut buffer = vec![0; 50];
    match mock_transport.receive(&mut buffer, Some(Duration::from_secs(1))).await {
        Ok(received_len) => {
            debug!("Response received successfully: {} bytes", received_len);
            let received_data = &buffer[..received_len];
            debug!("Received raw data: {:02X?}", received_data);
            
            // Step 6: Parse response PDU
            debug!("Step 6: Parsing response PDU");
            match processor.parse_pdu(received_data) {
                Ok(pdu_result) => {
                    debug!("PDU parsing successful");
                    debug!("Parsed PDU result: {:?}", pdu_result);
                }
                Err(e) => {
                    debug!("PDU parsing failed: {}", e);
                }
            }
        }
        Err(e) => {
            debug!("Response receive failed: {:?}", e);
        }
    }
    
    // Step 7: Check send history
    debug!("Step 7: Checking transmission history");
    let send_history = mock_transport.get_send_history().await;
    debug!("Send history count: {}", send_history.len());
    for (i, data) in send_history.iter().enumerate() {
        debug!("Send record {}: {:02X?}", i + 1, data);
    }
    
    debug!("Comprehensive packet exchange logging test completed");
}