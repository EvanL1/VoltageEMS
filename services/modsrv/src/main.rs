//! ModSrv主程序
//!
//! 提供简洁的服务启动和命令行接口

#![allow(dead_code)]
#![allow(unused_imports)]

mod api;
mod config;
mod error;
mod mapping;
mod model;
mod websocket;

use crate::api::ApiServer;
use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::model::ModelManager;
use crate::websocket::WsConnectionManager;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use tracing::{error, info, warn};
use voltage_libs::redis::RedisClient;

#[derive(Parser, Debug)]
#[command(author, version, about = "ModSrv - 模型服务")]
struct Args {
    /// 配置文件路径
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// 运行服务模式
    Service,
    /// 显示模型信息
    Info,
    /// 检查配置文件
    CheckConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 加载配置
    let config = if let Some(config_path) = args.config {
        Config::from_file(config_path)?
    } else if let Ok(config_file) = std::env::var("CONFIG_FILE") {
        info!("从环境变量CONFIG_FILE加载配置: {}", config_file);
        Config::from_file(config_file)?
    } else {
        Config::load()?
    };

    // 验证配置
    config.validate()?;

    // 初始化日志
    voltage_libs::logging::init(&config.log.level)
        .map_err(|e| ModelSrvError::config(format!("日志初始化失败: {}", e)))?;

    info!("启动ModSrv v{}", config.version);

    // 执行命令
    match args.command {
        Some(Commands::Service) => run_service(config).await,
        Some(Commands::Info) => show_model_info(config).await,
        Some(Commands::CheckConfig) => check_config(config).await,
        None => run_service(config).await, // 默认运行服务
    }
}

/// 运行服务模式
async fn run_service(config: Config) -> Result<()> {
    info!("启动ModSrv服务模式");

    // 创建Redis连接
    let redis_client = Arc::new(Mutex::new(
        RedisClient::new(&config.redis.url)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Redis连接失败: {}", e)))?,
    ));

    // 创建模型管理器
    let model_manager = ModelManager::new(redis_client.clone());

    // 创建WebSocket管理器
    let ws_manager = Arc::new(WsConnectionManager::new(Arc::new(model_manager)));

    // 重新创建模型管理器并设置WebSocket管理器
    let mut model_manager = ModelManager::new(redis_client.clone());
    model_manager.set_ws_manager(ws_manager.clone());
    let model_manager = Arc::new(model_manager);

    // 加载模型配置
    let enabled_models = config.enabled_models();
    let model_configs: Vec<_> = enabled_models.into_iter().cloned().collect();

    info!("发现 {} 个模型配置", config.models.len());
    info!("已配置 {} 个模型", model_configs.len());

    if !model_configs.is_empty() {
        for model in &model_configs {
            info!("加载模型: {} ({})", model.id, model.name);
        }
        model_manager.load_models(model_configs).await?;
        info!("模型加载完成");

        // 加载映射配置
        let mappings_dir =
            std::env::var("MAPPINGS_DIR").unwrap_or_else(|_| "config/mappings".to_string());
        info!("加载映射配置: {}", mappings_dir);
        if let Err(e) = model_manager.load_mappings_directory(&mappings_dir).await {
            warn!("加载映射配置失败: {}", e);
        }
    } else {
        info!("未配置模型，服务将仅提供API接口");
    }

    // 启动数据订阅
    model_manager.subscribe_data_updates().await?;

    // 启动WebSocket心跳
    ws_manager.start_heartbeat().await;

    // 创建API服务器
    let api_server = ApiServer::new(model_manager.clone(), ws_manager.clone(), config.clone());

    // 启动API服务器
    let (startup_tx, mut startup_rx) = mpsc::channel::<std::result::Result<(), String>>(1);

    tokio::spawn(async move {
        if let Err(e) = api_server.start_with_notification(startup_tx).await {
            error!("API服务器启动失败: {}", e);
        }
    });

    // 等待API服务器启动确认
    info!("等待API服务器启动...");
    match tokio::time::timeout(Duration::from_secs(10), startup_rx.recv()).await {
        Ok(Some(Ok(_))) => {
            info!(
                "✓ API服务器启动成功: http://{}:{}",
                config.api.host, config.api.port
            );
        }
        Ok(Some(Err(e))) => {
            error!("✗ API服务器启动失败: {}", e);
            return Err(ModelSrvError::config("API服务器启动失败".to_string()));
        }
        Ok(None) => {
            error!("✗ API服务器启动通道关闭");
            return Err(ModelSrvError::config("API服务器启动通道关闭".to_string()));
        }
        Err(_) => {
            error!("✗ API服务器启动超时");
            return Err(ModelSrvError::config("API服务器启动超时".to_string()));
        }
    }

    info!(
        "ModSrv服务已启动，API地址: http://{}:{}",
        config.api.host, config.api.port
    );

    // 主服务循环 - 定期更新和健康检查
    let mut interval = time::interval(Duration::from_millis(config.update_interval_ms));
    let mut cycle_count = 0u64;

    loop {
        interval.tick().await;
        cycle_count += 1;

        // 每1000个周期输出一次状态信息
        if cycle_count % 1000 == 0 {
            let models = model_manager.list_models().await;
            info!(
                "服务运行正常 - 周期: {}, 模型数: {}",
                cycle_count,
                models.len()
            );
        }

        // 这里将来可以添加:
        // - 定期数据同步
        // - 健康检查
        // - 性能统计
    }
}

/// 显示模型信息
async fn show_model_info(config: Config) -> Result<()> {
    println!("=== ModSrv 模型信息 ===");
    println!("服务名称: {}", config.service_name);
    println!("版本: {}", config.version);
    println!("Redis地址: {}", config.redis.url);
    println!("API地址: http://{}:{}", config.api.host, config.api.port);
    println!();

    println!("=== 配置的模型 ===");
    if config.models.is_empty() {
        println!("未配置任何模型");
    } else {
        for (index, model) in config.models.iter().enumerate() {
            println!("{}. {} ({})", index + 1, model.name, model.id);
            println!("   描述: {}", model.description);
            println!("   监视点: {} 个", model.monitoring.len());
            println!("   控制点: {} 个", model.control.len());

            if !model.monitoring.is_empty() {
                println!("   监视点列表:");
                for (name, config) in &model.monitoring {
                    println!(
                        "     - {}: {} {}",
                        name,
                        config.description,
                        config.unit.as_deref().unwrap_or("")
                    );
                }
            }

            if !model.control.is_empty() {
                println!("   控制点列表:");
                for (name, config) in &model.control {
                    println!(
                        "     - {}: {} {}",
                        name,
                        config.description,
                        config.unit.as_deref().unwrap_or("")
                    );
                }
            }
            println!();
        }
    }

    Ok(())
}

/// 检查配置文件
async fn check_config(config: Config) -> Result<()> {
    println!("=== 配置文件检查 ===");

    match config.validate() {
        Ok(_) => {
            println!("✓ 配置文件验证通过");

            println!("\n=== 配置详情 ===");
            println!("服务名称: {}", config.service_name);
            println!("版本: {}", config.version);
            println!("Redis URL: {}", config.redis.url);
            println!("Redis前缀: {}", config.redis.key_prefix);
            println!("API地址: {}:{}", config.api.host, config.api.port);
            println!("日志级别: {}", config.log.level);
            println!("更新间隔: {}ms", config.update_interval_ms);
            println!("模型数量: {}", config.models.len());

            // 测试Redis连接
            println!("\n=== 连接测试 ===");
            match RedisClient::new(&config.redis.url).await {
                Ok(_) => println!("✓ Redis连接测试成功"),
                Err(e) => {
                    println!("✗ Redis连接测试失败: {}", e);
                    return Err(ModelSrvError::redis(format!("Redis连接失败: {}", e)));
                }
            }
        }
        Err(e) => {
            println!("✗ 配置文件验证失败: {}", e);
            return Err(e);
        }
    }

    println!("\n配置检查完成");
    Ok(())
}
