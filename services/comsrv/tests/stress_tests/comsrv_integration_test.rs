///! comsrv 集成测试
///! 
///! 该测试通过启动外部Modbus服务器，然后让comsrv服务连接并进行通信测试
///! 验证comsrv服务的多通道日志功能

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tokio::time::sleep;
use log::{info, debug, warn, error};

use comsrv::utils::logger::{ChannelLoggerManager, LogLevel};
use comsrv::core::protocols::init_protocol_parsers;

/// comsrv集成测试配置
#[derive(Debug, Clone)]
pub struct ComsrvIntegrationTestConfig {
    /// 外部Modbus服务器数量
    pub external_server_count: usize,
    /// 基础端口号
    pub base_port: u16,
    /// 测试持续时间（秒）
    pub test_duration_secs: u64,
    /// comsrv配置文件路径
    pub comsrv_config_path: String,
    /// 监控间隔（毫秒）
    pub monitor_interval_ms: u64,
}

impl Default for ComsrvIntegrationTestConfig {
    fn default() -> Self {
        Self {
            external_server_count: 2,
            base_port: 5502,
            test_duration_secs: 30,
            comsrv_config_path: "config/comsrv.yaml".to_string(),
            monitor_interval_ms: 5000,
        }
    }
}

/// 集成测试统计
#[derive(Debug, Default, Clone)]
pub struct IntegrationTestStats {
    pub start_time: Option<Instant>,
    pub comsrv_channels_created: u64,
    pub external_servers_started: u64,
    pub total_connections: u64,
    pub total_requests_processed: u64,
    pub comsrv_log_files_created: u64,
}

impl IntegrationTestStats {
    pub fn test_duration(&self) -> f64 {
        if let Some(start) = self.start_time {
            start.elapsed().as_secs_f64()
        } else {
            0.0
        }
    }
}

/// comsrv集成测试管理器
pub struct ComsrvIntegrationTestManager {
    config: ComsrvIntegrationTestConfig,
    stats: Arc<RwLock<IntegrationTestStats>>,
    external_logger_manager: ChannelLoggerManager,
}

impl ComsrvIntegrationTestManager {
    pub fn new(config: ComsrvIntegrationTestConfig) -> Self {
        // 外部服务器使用单独的日志目录
        let external_log_dir = "tests/logs/external_servers";
        let external_logger_manager = ChannelLoggerManager::new(&external_log_dir);
        
        // 初始化协议解析器
        init_protocol_parsers();
        
        Self {
            config,
            stats: Arc::new(RwLock::new(IntegrationTestStats::default())),
            external_logger_manager,
        }
    }

    /// 运行完整的集成测试
    pub async fn run_integration_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 启动comsrv集成测试");
        println!("配置：{}个外部服务器，端口{}-{}", 
                 self.config.external_server_count, 
                 self.config.base_port,
                 self.config.base_port + self.config.external_server_count as u16 - 1);
        
        {
            let mut stats = self.stats.write().await;
            stats.start_time = Some(Instant::now());
        }
        
        // 步骤1：启动外部Modbus服务器
        self.start_external_modbus_servers().await?;
        
        // 步骤2：等待服务器启动
        sleep(Duration::from_secs(2)).await;
        
        // 步骤3：启动comsrv服务（在后台）
        self.start_comsrv_service().await?;
        
        // 步骤4：等待comsrv启动并连接
        sleep(Duration::from_secs(3)).await;
        
        // 步骤5：开始监控
        self.start_monitoring().await?;
        
        // 步骤6：等待测试完成
        sleep(Duration::from_secs(self.config.test_duration_secs)).await;
        
        // 步骤7：生成报告
        self.generate_integration_report().await;
        
        println!("✅ comsrv集成测试完成");
        Ok(())
    }

    /// 启动外部Modbus服务器
    async fn start_external_modbus_servers(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔧 启动{}个外部Modbus服务器供comsrv连接...", self.config.external_server_count);
        
        for server_id in 0..self.config.external_server_count {
            let port = self.config.base_port + server_id as u16;
            let channel_id = format!("external_modbus_server_{}", server_id + 1);
            let mut logger = self.external_logger_manager.get_logger(&channel_id, LogLevel::Debug)?;
            logger.set_protocol("Modbus");
            
            let stats = self.stats.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::run_external_modbus_server(port, channel_id, logger, stats).await {
                    error!("外部Modbus服务器启动失败，端口{}: {}", port, e);
                }
            });
            
            sleep(Duration::from_millis(200)).await;
            println!("  ✅ 外部Modbus服务器启动：端口{}", port);
        }
        
        {
            let mut stats = self.stats.write().await;
            stats.external_servers_started = self.config.external_server_count as u64;
        }
        
        println!("  ✅ 所有外部Modbus服务器已启动");
        Ok(())
    }

    /// 运行外部Modbus服务器
    async fn run_external_modbus_server(
        port: u16,
        channel_id: String,
        logger: comsrv::utils::logger::ChannelLogger,
        stats: Arc<RwLock<IntegrationTestStats>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        logger.info(&format!("外部Modbus服务器监听端口：{}", port));
        
        while let Ok((stream, addr)) = listener.accept().await {
            logger.info(&format!("comsrv客户端连接：{}", addr));
            
            {
                let mut stats = stats.write().await;
                stats.total_connections += 1;
            }
            
            let logger_clone = logger.clone();
            let stats_clone = stats.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_comsrv_connection(stream, logger_clone, stats_clone).await {
                    error!("处理comsrv连接失败：{}", e);
                }
            });
        }
        
        Ok(())
    }

    /// 处理来自comsrv的连接
    async fn handle_comsrv_connection(
        mut stream: TcpStream,
        logger: comsrv::utils::logger::ChannelLogger,
        stats: Arc<RwLock<IntegrationTestStats>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = [0u8; 1024];
        
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            let request = &buffer[0..n];
            
            // 记录来自comsrv的请求
            logger.log_packet("receive_from_comsrv", request);
            
            // 处理Modbus请求并生成响应
            if let Some(response) = Self::process_comsrv_modbus_request(request, &logger) {
                stream.write_all(&response).await?;
                
                // 记录发送给comsrv的响应
                logger.log_packet("send_to_comsrv", &response);
                
                {
                    let mut stats = stats.write().await;
                    stats.total_requests_processed += 1;
                }
            }
        }
        
        logger.info("comsrv客户端断开连接");
        Ok(())
    }

    /// 处理来自comsrv的Modbus请求
    fn process_comsrv_modbus_request(
        request: &[u8],
        logger: &comsrv::utils::logger::ChannelLogger,
    ) -> Option<Vec<u8>> {
        if request.len() < 8 {
            logger.warn("Modbus请求太短");
            return None;
        }
        
        let transaction_id = u16::from_be_bytes([request[0], request[1]]);
        let protocol_id = u16::from_be_bytes([request[2], request[3]]);
        let length = u16::from_be_bytes([request[4], request[5]]);
        let unit_id = request[6];
        let function_code = request[7];
        
        logger.debug(&format!(
            "处理comsrv请求：TxID:{:04x} Unit:{} FC:0x{:02x} Len:{}",
            transaction_id, unit_id, function_code, length
        ));
        
        // 生成简单的模拟响应
        match function_code {
            0x03 => { // Read holding registers
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    
                    let byte_count = quantity * 2;
                    let mut response = vec![0u8; 9 + byte_count as usize];
                    
                    response[0..6].copy_from_slice(&request[0..6]);
                    response[4] = 0;
                    response[5] = 3 + byte_count as u8;
                    response[6] = unit_id;
                    response[7] = function_code;
                    response[8] = byte_count as u8;
                    
                    // 填充模拟数据
                    for i in 0..quantity {
                        let value = start_addr + i + 1000; // 模拟数据
                        let offset = 9 + (i * 2) as usize;
                        response[offset] = (value >> 8) as u8;
                        response[offset + 1] = (value & 0xFF) as u8;
                    }
                    
                    return Some(response);
                }
            },
            _ => {
                logger.warn(&format!("不支持的功能码：0x{:02x}", function_code));
            }
        }
        
        None
    }

    /// 启动comsrv服务
    async fn start_comsrv_service(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 启动comsrv服务...");
        
        // 在后台启动comsrv服务
        let config_path = self.config.comsrv_config_path.clone();
        tokio::spawn(async move {
            let mut cmd = tokio::process::Command::new("./target/debug/comsrv")
                .args(&["--config", &config_path, "--log-level", "debug"])
                .spawn()
                .expect("无法启动comsrv服务");
            
            let _ = cmd.wait().await;
        });
        
        println!("  ✅ comsrv服务已在后台启动");
        Ok(())
    }

    /// 开始监控
    async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 开始监控comsrv集成测试...");
        
        let stats = self.stats.clone();
        let monitor_interval = self.config.monitor_interval_ms;
        let test_duration = self.config.test_duration_secs;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(monitor_interval));
            let start = Instant::now();
            
            while start.elapsed() < Duration::from_secs(test_duration) {
                interval.tick().await;
                
                let stats = stats.read().await;
                
                println!("📈 comsrv集成测试实时统计：");
                println!("  ⏱️  运行时间：{:.1}秒", stats.test_duration());
                println!("  🔗 外部服务器数量：{}", stats.external_servers_started);
                println!("  🔌 comsrv连接数：{}", stats.total_connections);
                println!("  📦 处理请求数：{}", stats.total_requests_processed);
                println!();
            }
        });
        
        println!("  ✅ 监控已启动");
        Ok(())
    }

    /// 生成集成测试报告
    async fn generate_integration_report(&self) {
        println!("🎉 comsrv集成测试完成！");
        println!("==========================================");
        
        let stats = self.stats.read().await;
        
        println!("⏱️  总测试时间：{:.2}秒", stats.test_duration());
        println!("📊 集成测试统计：");
        println!("  🖥️  外部服务器启动：{}", stats.external_servers_started);
        println!("  🔗 comsrv连接数：{}", stats.total_connections);
        println!("  📦 处理请求数：{}", stats.total_requests_processed);
        
        if stats.total_requests_processed > 0 {
            let request_rate = stats.total_requests_processed as f64 / stats.test_duration();
            println!("  📈 请求处理速率：{:.2} req/sec", request_rate);
        }
        
        println!("📁 日志文件位置：");
        println!("  🔧 comsrv服务日志：logs/channels/modbus_tcp_*");
        println!("  🖥️  外部服务器日志：tests/logs/external_servers/");
        
        println!("==========================================");
        
        // 检查comsrv日志文件
        if let Ok(entries) = std::fs::read_dir("logs/channels") {
            let count = entries.count();
            println!("✅ comsrv生成了{}个通道日志文件", count);
        } else {
            println!("⚠️  未找到comsrv通道日志目录");
        }
    }
}

/// 运行comsrv集成测试
pub async fn run_comsrv_integration_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 启动comsrv集成测试");
    
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).try_init();
    
    let config = ComsrvIntegrationTestConfig::default();
    let mut test_manager = ComsrvIntegrationTestManager::new(config);
    
    test_manager.run_integration_test().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_config() {
        let config = ComsrvIntegrationTestConfig::default();
        assert_eq!(config.external_server_count, 2);
        assert_eq!(config.base_port, 5502);
    }

    #[test]
    fn test_stats_duration() {
        let mut stats = IntegrationTestStats::default();
        stats.start_time = Some(Instant::now());
        
        std::thread::sleep(Duration::from_millis(100));
        
        let duration = stats.test_duration();
        assert!(duration > 0.0);
    }
} 