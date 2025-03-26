use std::sync::Arc;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use chrono::Utc;
use dotenv::dotenv;
use tracing;
use tokio::sync::RwLock;

mod core;
mod utils;
mod api;

use crate::utils::logger;
use crate::utils::error::Result;
use crate::core::config::ConfigManager;
use crate::core::protocol_factory::ProtocolFactory;
use crate::api::routes::api_routes;
use crate::core::protocols::modbus::client::ModbusClientFactory;
use crate::utils::ComSrvError;

/// 为工厂创建Modbus TCP客户端的函数
fn create_modbus_tcp(config: crate::core::config::config_manager::ChannelConfig) 
    -> Result<Box<dyn crate::core::protocols::common::ComBase>> {
    let config_clone = config.clone();
    let result = ModbusClientFactory::create_client(config);
    
    match result {
        Ok(_) => {
            // 创建一个新的 Box<dyn ComBase> 对象
            let client = crate::core::protocols::modbus::tcp::ModbusTcpClient::new(config_clone);
            Ok(Box::new(client) as Box<dyn crate::core::protocols::common::ComBase>)
        },
        Err(e) => Err(e),
    }
}

/// 为工厂创建Modbus RTU客户端的函数
fn create_modbus_rtu(config: crate::core::config::config_manager::ChannelConfig) 
    -> Result<Box<dyn crate::core::protocols::common::ComBase>> {
    let config_clone = config.clone();
    let result = ModbusClientFactory::create_client(config);
    
    match result {
        Ok(_) => {
            // 创建一个新的 Box<dyn ComBase> 对象
            let client = crate::core::protocols::modbus::rtu::ModbusRtuClient::new(config_clone);
            Ok(Box::new(client) as Box<dyn crate::core::protocols::common::ComBase>)
        },
        Err(e) => Err(e),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 加载.env文件
    dotenv().ok();
    
    // 初始化日志系统
    let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
    crate::utils::logger::init_logger(Path::new(&log_dir), "comsrv", "info", true)?;
    tracing::info!("Starting Comsrv Service");
    
    // 记录启动时间
    let start_time = Arc::new(Utc::now());
    
    // 加载配置
    let config_path_env = env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let config_path = if Path::new(&config_path_env).is_dir() {
        // 如果是目录，则拼接默认配置文件名
        format!("{}/comsrv.yaml", config_path_env)
    } else {
        config_path_env
    };
    tracing::info!("Loading configuration from {}", config_path);
    let config_manager = ConfigManager::from_file(&config_path)?;
    
    // 创建协议工厂
    let protocol_factory = Arc::new(RwLock::new(ProtocolFactory::new()));
    
    // 注册各种协议实现
    {
        let mut factory = protocol_factory.write().await;
        // 这里注册各种协议实现
        tracing::info!("Registering protocol implementations");
        
        // 注册Modbus TCP和RTU协议
        factory.register_protocol("modbus_tcp", create_modbus_tcp).await?;
        factory.register_protocol("modbus_rtu", create_modbus_rtu).await?;
    }
    
    // 初始化通道
    {
        let mut factory = protocol_factory.write().await;
        // 从配置中加载通道
        tracing::info!("Initializing channels from configuration");
        
        for channel_config in config_manager.get_channels() {
            match factory.create_channel(channel_config.clone()).await {
                Ok(_) => tracing::info!("Channel {} initialized", channel_config.id),
                Err(e) => tracing::error!("Failed to initialize channel {}: {}", channel_config.id, e),
            }
        }
    }
    
    // 启动所有通道
    {
        let mut factory = protocol_factory.write().await;
        let channels = factory.get_all_channels_mut().await;
        for (id, channel) in channels.iter_mut() {
            match channel.start().await {
                Ok(_) => tracing::info!("Channel {} started", id),
                Err(e) => tracing::error!("Failed to start channel {}: {}", id, e),
            }
        }
    }
    
    // 启动指标服务
    if config_manager.get_metrics_enabled() {
        let metrics_addr = config_manager.get_metrics_address()
            .parse::<SocketAddr>()
            .unwrap_or_else(|_| "0.0.0.0:9100".parse().unwrap());
            
        tracing::info!("Starting metrics service on {}", metrics_addr);
        
        // 初始化指标系统
        crate::core::metrics::init_metrics(&config_manager.get_service_name());
        
        // 获取指标实例
        if let Some(metrics) = crate::core::metrics::get_metrics() {
            tokio::spawn(async move {
                if let Err(e) = metrics.start_server(&metrics_addr.to_string()).await {
                    tracing::error!("Failed to start metrics server: {}", e);
                }
            });
        } else {
            tracing::error!("Failed to get metrics instance");
        }
    }
    
    // 启动API服务
    let api_addr = config_manager.get_api_address()
        .parse::<SocketAddr>()
        .unwrap_or_else(|_| "0.0.0.0:3000".parse().unwrap());
    
    tracing::info!("Starting API service on {}", api_addr);
    
    // 创建API路由
    let api = api_routes(protocol_factory.clone(), start_time.clone());
    
    // 启动warp服务器
    warp::serve(api)
        .run(api_addr)
        .await;
    
    // 正常退出
    tracing::info!("Comsrv Service shutdown");
    Ok(())
}
