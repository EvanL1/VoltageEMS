mod config;
mod error;
mod formatter;
mod network;
mod redis;

use crate::config::Config;
use crate::error::Result;
use crate::formatter::create_formatter;
use crate::network::{create_client, NetworkClient};
use crate::redis::RedisDataFetcher;
use clap::Parser;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "netsrv.json")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 加载配置
    let config = match Config::new(args.config.to_str().unwrap_or("netsrv.json")) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Using default configuration");
            Config::default()
        }
    };

    // 初始化日志
    init_logging(&config);

    info!("Starting Network Service");
    info!("Redis configuration: {}:{}", config.redis.host, config.redis.port);
    info!("Found {} network configurations", config.networks.len());

    // 创建数据通道
    let (tx, mut rx) = mpsc::channel::<Value>(100);

    // 启动 Redis 数据获取器
    let redis_config = config.redis.clone();
    let mut data_fetcher = RedisDataFetcher::new(redis_config);
    
    tokio::spawn(async move {
        if let Err(e) = data_fetcher.start_polling(tx).await {
            error!("Redis data fetcher error: {}", e);
        }
    });

    // 创建网络客户端
    let mut clients = Vec::new();
    
    for network_config in &config.networks {
        if !network_config.enabled {
            info!("Network '{}' is disabled, skipping", network_config.name);
            continue;
        }

        info!("Initializing network: {}", network_config.name);
        
        // 创建格式化器
        let formatter = create_formatter(&network_config.format_type);
        
        // 创建客户端
        match create_client(network_config, formatter) {
            Ok(client) => {
                let client_name = network_config.name.clone();
                let client = Arc::new(tokio::sync::Mutex::new(client));
                clients.push((client_name, client));
            }
            Err(e) => {
                error!("Failed to create client for network '{}': {}", network_config.name, e);
            }
        }
    }

    // 连接所有客户端
    for (name, client) in &clients {
        let mut client = client.lock().await;
        match client.connect().await {
            Ok(_) => info!("Connected to network: {}", name),
            Err(e) => error!("Failed to connect to network '{}': {}", name, e),
        }
    }

    // 主循环：接收数据并发送到所有网络
    while let Some(data) = rx.recv().await {
        debug!("Received data from Redis");
        
        for (name, client) in &clients {
            let client = client.lock().await;
            
            if !client.is_connected() {
                warn!("Client '{}' is not connected, skipping", name);
                continue;
            }
            
            // 格式化数据
            let formatted_data = match client.format_data(&data) {
                Ok(formatted) => formatted,
                Err(e) => {
                    error!("Failed to format data for network '{}': {}", name, e);
                    continue;
                }
            };
            
            // 发送数据
            match client.send(&formatted_data).await {
                Ok(_) => debug!("Data sent to network: {}", name),
                Err(e) => error!("Failed to send data to network '{}': {}", name, e),
            }
        }
    }

    // 断开所有客户端连接
    for (name, client) in &clients {
        let mut client = client.lock().await;
        if let Err(e) = client.disconnect().await {
            error!("Error disconnecting from network '{}': {}", name, e);
        }
    }

    info!("Network Service stopped");
    Ok(())
}

fn init_logging(config: &Config) {
    use env_logger::{Builder, Env};
    
    let env = Env::default().filter_or("RUST_LOG", &config.logging.level);
    let mut builder = Builder::from_env(env);
    
    builder.format_timestamp_millis();
    
    if config.logging.console {
        builder.init();
    }
    
    info!("Logging initialized at level: {}", config.logging.level);
}

// 扩展 NetworkClient trait 以添加格式化方法
trait NetworkClientExt: NetworkClient {
    fn format_data(&self, data: &Value) -> Result<String>;
}

// 为所有 NetworkClient 实现 NetworkClientExt
impl<T: NetworkClient> NetworkClientExt for T {
    fn format_data(&self, data: &Value) -> Result<String> {
        // 这里应该使用客户端内部的格式化器，但由于我们无法直接访问它，
        // 所以这里只是一个示例实现，实际上应该在各个客户端实现中处理
        serde_json::to_string(data).map_err(|e| {
            crate::error::NetSrvError::FormatError(format!("JSON formatting error: {}", e))
        })
    }
} 