//! Modbus Protocol Packet Testing
//! Real Modbus TCP packet testing with hex logging

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use log::{info, debug, warn, error};
use comsrv::utils::logger::{ChannelLogger, ChannelLoggerManager, LogLevel};
use comsrv::core::protocols;

#[derive(Debug, Clone)]
pub struct ModbusProtocolTestConfig {
    pub server_count: usize,
    pub base_port: u16,
    pub client_count: usize,
    pub test_duration_secs: u64,
    pub monitor_interval_ms: u64,
    pub enable_packet_logging: bool,
    pub register_count: u16,
    pub coil_count: u16,
}

impl Default for ModbusProtocolTestConfig {
    fn default() -> Self {
        Self {
            server_count: 2,
            base_port: 5502,
            client_count: 8,
            test_duration_secs: 30,
            monitor_interval_ms: 3000,
            enable_packet_logging: true,
            register_count: 100,
            coil_count: 64,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ModbusPacketStats {
    pub start_time: Option<Instant>,
    pub total_requests: u64,
    pub total_responses: u64,
    pub read_holding_registers: u64,
    pub read_input_registers: u64,
    pub read_coils: u64,
    pub write_single_register: u64,
    pub error_responses: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_response_time: f64,
}

impl ModbusPacketStats {
    pub fn error_rate(&self) -> f64 {
        if self.total_requests > 0 {
            self.error_responses as f64 / self.total_requests as f64
        } else {
            0.0
        }
    }
    
    pub fn avg_response_time(&self) -> f64 {
        if self.total_responses > 0 {
            self.total_response_time / self.total_responses as f64
        } else {
            0.0
        }
    }
    
    pub fn packet_rate(&self) -> f64 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                (self.total_requests + self.total_responses) as f64 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

pub struct ModbusProtocolTestManager {
    config: ModbusProtocolTestConfig,
    packet_stats: Arc<RwLock<ModbusPacketStats>>,
    logger_manager: ChannelLoggerManager,
    log_dir: String,
}

fn log_modbus_packet_debug(
    message: &[u8],
    direction: &str,
    logger: &ChannelLogger,
) {
    logger.log_packet(direction, message);
}

impl ModbusProtocolTestManager {
    pub fn new(config: ModbusProtocolTestConfig) -> Self {
        let log_dir = "tests/logs".to_string();
        let logger_manager = ChannelLoggerManager::new(&log_dir);
        
        // Initialize protocol parsers
        protocols::init_protocol_parsers();
        
        Self {
            config,
            packet_stats: Arc::new(RwLock::new(ModbusPacketStats::default())),
            logger_manager,
            log_dir,
        }
    }

    pub async fn run_complete_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Starting Modbus Protocol Packet Test");
        println!("Configuration: {} servers, {} clients, ports {}-{}", 
                 self.config.server_count, 
                 self.config.client_count,
                 self.config.base_port,
                 self.config.base_port + self.config.server_count as u16 - 1);
        
        {
            let mut stats = self.packet_stats.write().await;
            stats.start_time = Some(Instant::now());
        }
        
        self.init_channel_loggers().await?;
        self.start_modbus_servers().await?;
        self.start_packet_monitoring().await?;
        self.start_client_testing().await?;
        self.generate_packet_analysis_report().await;
        
        println!("✅ Test completed, test logs saved to tests/logs/channels/ directory");
        Ok(())
    }

    async fn init_channel_loggers(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📝 Initializing independent loggers for channels...");
        
        for i in 0..self.config.server_count {
            let channel_id = format!("modbus_server_{}", i + 1);
            let logger = self.logger_manager.get_logger(&channel_id, LogLevel::Debug)?;
            logger.info(&format!("Modbus server channel {} logger initialized", i + 1));
            println!("  📋 Server channel {} logs: tests/logs/channels/{}/", i + 1, channel_id);
        }
        
        for i in 0..self.config.client_count {
            let channel_id = format!("modbus_client_{}", i + 1);
            let logger = self.logger_manager.get_logger(&channel_id, LogLevel::Debug)?;
            logger.info(&format!("Modbus client channel {} logger initialized", i + 1));
            println!("  📋 Client channel {} logs: tests/logs/channels/{}/", i + 1, channel_id);
        }
        
        println!("✅ All channel loggers initialized");
        Ok(())
    }

    async fn start_modbus_servers(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔧 Starting {} Modbus TCP servers...", self.config.server_count);
        
        for server_id in 0..self.config.server_count {
            let port = self.config.base_port + server_id as u16;
            let channel_id = format!("modbus_server_{}", server_id + 1);
            let mut logger = self.logger_manager.get_logger(&channel_id, LogLevel::Debug)?;
            
            // Set protocol type for packet parsing
            logger.set_protocol("Modbus");
            
            let log_dir = self.log_dir.clone();
            let register_count = self.config.register_count;
            let coil_count = self.config.coil_count;
            
            tokio::spawn(async move {
                if let Err(e) = Self::run_modbus_server(
                    port, channel_id, logger, log_dir, register_count, coil_count
                ).await {
                    error!("Modbus server failed to start on port {}: {}", port, e);
                }
            });
            
            tokio::time::sleep(Duration::from_millis(100)).await;
            println!("  ✅ Modbus server started: port {}", port);
        }
        
        println!("  ✅ All Modbus servers started");
        Ok(())
    }

    async fn run_modbus_server(
        port: u16,
        channel_id: String,
        logger: ChannelLogger,
        log_dir: String,
        register_count: u16,
        coil_count: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        logger.info(&format!("Modbus server listening on port {}", port));
        
        while let Ok((stream, addr)) = listener.accept().await {
            logger.debug(&format!("Client connected: {}", addr));
            
            let channel_id_clone = channel_id.clone();
            let logger_clone = logger.clone();
            let log_dir_clone = log_dir.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_modbus_connection(
                    stream, channel_id_clone, logger_clone, log_dir_clone, register_count, coil_count
                ).await {
                    error!("Failed to handle Modbus connection: {}", e);
                }
            });
        }
        
        Ok(())
    }

    async fn handle_modbus_connection(
        mut stream: TcpStream,
        channel_id: String,
        logger: ChannelLogger,
        log_dir: String,
        register_count: u16,
        coil_count: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = [0u8; 1024];
        
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            let request = &buffer[0..n];
            
            log_modbus_packet_debug(request, "receive", &logger);
            
            logger.debug(&format!("Received Modbus request: {} bytes", n));
            
            if let Some(response) = Self::process_modbus_request(
                request, &channel_id, &logger, register_count, coil_count
            ) {
                stream.write_all(&response).await?;
                
                log_modbus_packet_debug(&response, "send", &logger);
                
                logger.debug(&format!("Sent Modbus response: {} bytes", response.len()));
            }
        }
        
        Ok(())
    }

    fn process_modbus_request(
        request: &[u8],
        channel_id: &str,
        logger: &ChannelLogger,
        register_count: u16,
        coil_count: u16,
    ) -> Option<Vec<u8>> {
        if request.len() < 8 {
            logger.warn("Modbus request too short");
            return None;
        }
        
        let transaction_id = u16::from_be_bytes([request[0], request[1]]);
        let protocol_id = u16::from_be_bytes([request[2], request[3]]);
        let length = u16::from_be_bytes([request[4], request[5]]);
        let unit_id = request[6];
        let function_code = request[7];
        
        if protocol_id != 0 {
            logger.warn(&format!("Invalid protocol ID: {}", protocol_id));
            return None;
        }
        
        logger.trace(&format!(
            "Channel[{}]: TxID:{:04x} Unit:{} FC:0x{:02x} Len:{}",
            channel_id, transaction_id, unit_id, function_code, length
        ));
        
        match function_code {
            0x01 => { // Read coils
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    
                    if start_addr + quantity <= coil_count {
                        let byte_count = (quantity + 7) / 8;
                        let mut response = vec![0u8; 9 + byte_count as usize];
                        
                        response[0..6].copy_from_slice(&request[0..6]);
                        response[4] = 0;
                        response[5] = 3 + byte_count as u8;
                        response[6] = unit_id;
                        response[7] = function_code;
                        response[8] = byte_count as u8;
                        
                        for i in 0..byte_count {
                            response[9 + i as usize] = 0x55;
                        }
                        
                        return Some(response);
                    }
                }
            },
            0x03 => { // Read holding registers
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    
                    if start_addr + quantity <= register_count {
                        let byte_count = quantity * 2;
                        let mut response = vec![0u8; 9 + byte_count as usize];
                        
                        response[0..6].copy_from_slice(&request[0..6]);
                        response[4] = 0;
                        response[5] = 3 + byte_count as u8;
                        response[6] = unit_id;
                        response[7] = function_code;
                        response[8] = byte_count as u8;
                        
                        for i in 0..quantity {
                            let value = (start_addr + i) as u16;
                            let offset = 9 + (i * 2) as usize;
                            response[offset] = (value >> 8) as u8;
                            response[offset + 1] = (value & 0xFF) as u8;
                        }
                        
                        return Some(response);
                    }
                }
            },
            0x04 => { // Read input registers
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    
                    if start_addr + quantity <= register_count {
                        let byte_count = quantity * 2;
                        let mut response = vec![0u8; 9 + byte_count as usize];
                        
                        response[0..6].copy_from_slice(&request[0..6]);
                        response[4] = 0;
                        response[5] = 3 + byte_count as u8;
                        response[6] = unit_id;
                        response[7] = function_code;
                        response[8] = byte_count as u8;
                        
                        for i in 0..quantity {
                            let value = 2000 + i;
                            let offset = 9 + (i * 2) as usize;
                            response[offset] = (value >> 8) as u8;
                            response[offset + 1] = (value & 0xFF) as u8;
                        }
                        
                        return Some(response);
                    }
                }
            },
            0x06 => { // Write single register
                if request.len() >= 12 {
                    let mut response = vec![0u8; 12];
                    response.copy_from_slice(request);
                    return Some(response);
                }
            },
            _ => {
                logger.warn(&format!("Unsupported function code: 0x{:02x}", function_code));
            }
        }
        
        None
    }

    async fn start_packet_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 Starting Modbus packet monitoring...");
        
        let packet_stats = self.packet_stats.clone();
        let monitor_interval = self.config.monitor_interval_ms;
        let test_duration = self.config.test_duration_secs;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(monitor_interval));
            let start = Instant::now();
            
            while start.elapsed() < Duration::from_secs(test_duration) {
                interval.tick().await;
                
                let stats = packet_stats.read().await;
                let packet_rate = stats.packet_rate();
                let avg_response_time = stats.avg_response_time();
                let error_rate = stats.error_rate();
                
                println!("📈 Modbus Packet Real-time Statistics:");
                println!("  📦 Total requests: {}", stats.total_requests);
                println!("  📦 Total responses: {}", stats.total_responses);
                println!("  📊 Packet rate: {:.2} packets/sec", packet_rate);
                println!("  ⏱️  Average response time: {:.2}ms", avg_response_time);
                println!("  ❌ Error rate: {:.2}%", error_rate * 100.0);
                println!("  📈 Data transfer: ↑{}bytes ↓{}bytes", stats.total_bytes_sent, stats.total_bytes_received);
                println!("  🔧 Function code statistics:");
                println!("    Read holding registers (0x03): {}", stats.read_holding_registers);
                println!("    Read input registers (0x04): {}", stats.read_input_registers);
                println!("    Read coils (0x01): {}", stats.read_coils);
                if stats.write_single_register > 0 {
                    println!("    Write single register (0x06): {}", stats.write_single_register);
                }
                println!();
            }
        });
        
        println!("  ✅ Packet monitoring started");
        Ok(())
    }

    async fn start_client_testing(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔧 Starting {} Modbus clients...", self.config.client_count);
        
        let mut tasks = Vec::new();
        
        for client_id in 0..self.config.client_count {
            let channel_id = format!("modbus_client_{}", client_id + 1);
            let mut logger = self.logger_manager.get_logger(&channel_id, LogLevel::Debug)?;
            
            // Set protocol type for packet parsing
            logger.set_protocol("Modbus");
            let packet_stats = self.packet_stats.clone();
            let log_dir = self.log_dir.clone();
            let test_duration = self.config.test_duration_secs;
            let base_port = self.config.base_port;
            let server_count = self.config.server_count;
            
            let task = tokio::spawn(async move {
                Self::run_modbus_client(
                    client_id, channel_id, logger, packet_stats, log_dir, test_duration, base_port, server_count
                ).await
            });
            
            tasks.push(task);
        }
        
        println!("  ✅ All clients started");
        
        for task in tasks {
            if let Err(e) = task.await {
                error!("Client task failed: {}", e);
            }
        }
        
        Ok(())
    }

    async fn run_modbus_client(
        client_id: usize,
        channel_id: String,
        logger: ChannelLogger,
        packet_stats: Arc<RwLock<ModbusPacketStats>>,
        log_dir: String,
        test_duration: u64,
        base_port: u16,
        server_count: usize,
    ) {
        let start = Instant::now();
        let mut request_interval = interval(Duration::from_millis(500 + client_id as u64 * 100));
        
        logger.info("Modbus client test started");
        
        while start.elapsed() < Duration::from_secs(test_duration) {
            request_interval.tick().await;
            
            let server_port = base_port + (client_id % server_count) as u16;
            let function_codes = [0x01, 0x03, 0x04, 0x06];
            let function_code = function_codes[client_id % function_codes.len()];
            
            if let Err(e) = Self::send_modbus_request(
                &channel_id, &logger, packet_stats.clone(), &log_dir, server_port, function_code
            ).await {
                logger.warn(&format!("Failed to send Modbus request: {}", e));
            }
        }
        
        logger.info("Modbus client test completed");
        println!("🔧 Client {} test completed", client_id);
    }

    async fn send_modbus_request(
        channel_id: &str,
        logger: &ChannelLogger,
        packet_stats: Arc<RwLock<ModbusPacketStats>>,
        log_dir: &str,
        port: u16,
        function_code: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
        
        let start_time = Instant::now();
        let request = Self::build_modbus_request(function_code);
        
        stream.write_all(&request).await?;
        
        log_modbus_packet_debug(&request, "send", logger);
        
        let mut response = [0u8; 1024];
        let n = stream.read(&mut response).await?;
        
        let response_time = start_time.elapsed().as_millis() as f64;
        
        log_modbus_packet_debug(&response[0..n], "receive", logger);
        
        {
            let mut stats = packet_stats.write().await;
            stats.total_requests += 1;
            stats.total_responses += 1;
            stats.total_bytes_sent += request.len() as u64;
            stats.total_bytes_received += n as u64;
            stats.total_response_time += response_time;
            
            match function_code {
                0x01 => stats.read_coils += 1,
                0x03 => stats.read_holding_registers += 1,
                0x04 => stats.read_input_registers += 1,
                0x06 => stats.write_single_register += 1,
                _ => {},
            }
        }
        
        logger.debug(&format!(
            "Completed Modbus request: FC=0x{:02x}, response_time={:.1}ms",
            function_code, response_time
        ));
        
        Ok(())
    }

    fn build_modbus_request(function_code: u8) -> Vec<u8> {
        let transaction_id = rand::random::<u16>();
        let unit_id = (rand::random::<u8>() % 8) + 1;
        
        match function_code {
            0x01 => {
                let start_addr = rand::random::<u8>() % 10;
                let quantity = 8u16;
                vec![
                    (transaction_id >> 8) as u8, (transaction_id & 0xFF) as u8,
                    0, 0, 0, 6, unit_id, function_code,
                    0, start_addr,
                    (quantity >> 8) as u8, (quantity & 0xFF) as u8,
                ]
            },
            0x03 => {
                let start_addr = rand::random::<u8>() % 10;
                let quantity = 10u16;
                vec![
                    (transaction_id >> 8) as u8, (transaction_id & 0xFF) as u8,
                    0, 0, 0, 6, unit_id, function_code,
                    0, start_addr,
                    (quantity >> 8) as u8, (quantity & 0xFF) as u8,
                ]
            },
            0x04 => {
                let start_addr = 1 + rand::random::<u8>() % 5;
                let quantity = 5u16;
                vec![
                    (transaction_id >> 8) as u8, (transaction_id & 0xFF) as u8,
                    0, 0, 0, 6, unit_id, function_code,
                    0, start_addr,
                    (quantity >> 8) as u8, (quantity & 0xFF) as u8,
                ]
            },
            0x06 => {
                let register_addr = 3;
                let value = rand::random::<u16>() % 1000;
                vec![
                    (transaction_id >> 8) as u8, (transaction_id & 0xFF) as u8,
                    0, 0, 0, 6, unit_id, function_code,
                    0, register_addr,
                    (value >> 8) as u8, (value & 0xFF) as u8,
                ]
            },
            _ => vec![],
        }
    }

    async fn generate_packet_analysis_report(&self) {
        println!("🎉 Modbus Protocol Test Completed!");
        println!("==========================================");
        
        let stats = self.packet_stats.read().await;
        
        if let Some(start_time) = stats.start_time {
            let duration = start_time.elapsed();
            println!("⏱️  Total test duration: {:.2} seconds", duration.as_secs_f64());
        }
        
        println!("📊 Modbus Packet Statistics:");
        println!("  📦 Total requests: {}", stats.total_requests);
        println!("  📦 Total responses: {}", stats.total_responses);
        println!("  📈 Packet rate: {:.2} packets/sec", stats.packet_rate());
        println!("  ⏱️  Average response time: {:.2}ms", stats.avg_response_time());
        println!("  ❌ Error rate: {:.2}%", stats.error_rate() * 100.0);
        
        println!("📋 Function Code Details:");
        println!("  Read holding registers (0x03): {}", stats.read_holding_registers);
        println!("  Read input registers (0x04): {}", stats.read_input_registers);
        println!("  Read coils (0x01): {}", stats.read_coils);
        println!("  Write single register (0x06): {}", stats.write_single_register);
        println!("  Error responses: {}", stats.error_responses);
        
        println!("💾 Data Transfer Statistics:");
        println!("  Total bytes sent: {}", stats.total_bytes_sent);
        println!("  Total bytes received: {}", stats.total_bytes_received);
        
        let packet_rate = stats.packet_rate();
        let avg_response = stats.avg_response_time();
        let error_rate = stats.error_rate();
        
        let rating = if packet_rate >= 50.0 && avg_response <= 100.0 && error_rate <= 0.01 {
            "⭐⭐⭐⭐⭐ Excellent"
        } else if packet_rate >= 20.0 && avg_response <= 200.0 && error_rate <= 0.1 {
            "⭐⭐⭐ Good"
        } else {
            "⭐⭐ Needs improvement"
        };
        
        println!("🏆 Modbus Protocol Performance Rating:");
        println!("  {} (≥20 packets/sec, ≤200ms response, ≤10% error)", rating);
        
        println!("==========================================");
        println!("📁 Log File Locations:");
        println!("  Server logs: logs/channels/modbus_server_*/");
        println!("  Client logs: logs/channels/modbus_client_*/");
        println!("  Packet records: logs/messages/modbus_*/");
    }
}

pub async fn run_modbus_protocol_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Modbus Protocol Packet Test");
    
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).try_init();
    
    let config = ModbusProtocolTestConfig::default();
    let mut test_manager = ModbusProtocolTestManager::new(config);
    
    test_manager.run_complete_test().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_modbus_protocol_test_config() {
        let config = ModbusProtocolTestConfig::default();
        assert_eq!(config.server_count, 2);
        assert_eq!(config.client_count, 8);
        assert_eq!(config.base_port, 5502);
    }

    #[tokio::test]
    async fn test_modbus_packet_stats() {
        let mut stats = ModbusPacketStats::default();
        stats.total_requests = 100;
        stats.error_responses = 5;
        stats.total_response_time = 1000.0;
        stats.total_responses = 95;
        
        assert_eq!(stats.error_rate(), 0.05);
        assert!((stats.avg_response_time() - 10.526).abs() < 0.1);
    }

    #[test]
    fn test_build_modbus_request() {
        let request = ModbusProtocolTestManager::build_modbus_request(0x03);
        assert_eq!(request.len(), 12);
        assert_eq!(request[7], 0x03);
    }

    #[test]
    fn test_protocol_integration() {
        // Test that protocol parsers are properly integrated
        use comsrv::core::protocols::common::combase::parse_protocol_packet;
        
        protocols::init_protocol_parsers();
        
        // Test Modbus packet parsing
        let request = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x00, 0x00, 0x05];
        let result = parse_protocol_packet("Modbus", &request, "send");
        
        assert!(result.success);
        assert_eq!(result.protocol, "Modbus");
        assert!(result.description.contains("TxID:0x0001"));
        assert!(result.description.contains("FC:0x03(Read Holding Registers)"));
    }
} 