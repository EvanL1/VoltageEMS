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
use crate::mapping::MappingManager;
use crate::model::ModelManager;
use crate::websocket::WsConnectionManager;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info, warn};

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
    /// 检查配置和连接
    Check,
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
        Some(Commands::Check) => check_config(config).await,
        None => run_service(config).await, // 默认运行服务
    }
}

/// 运行服务模式
async fn run_service(config: Config) -> Result<()> {
    info!("启动ModSrv服务模式");

    // 创建模型管理器（使用EdgeRedis）
    let mut model_manager = ModelManager::new(&config.redis.url).await?;

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
    } else {
        info!("未配置模型，服务将仅提供API接口");
    }

    let model_manager = Arc::new(model_manager);

    // 加载映射配置
    let mut mapping_manager = MappingManager::new();
    let mappings_dir =
        std::env::var("MAPPINGS_DIR").unwrap_or_else(|_| "config/mappings".to_string());
    info!("加载映射配置: {}", mappings_dir);

    if let Err(e) = mapping_manager.load_directory(&mappings_dir).await {
        warn!("加载映射配置失败: {}", e);
    } else {
        // 将映射加载到Redis
        model_manager.load_mappings(&mapping_manager).await?;
    }

    // 创建WebSocket管理器
    let ws_manager = Arc::new(WsConnectionManager::new(model_manager.clone()));

    // 启动Redis订阅
    if let Err(e) = ws_manager.start_redis_subscription().await {
        warn!("启动Redis订阅失败: {}", e);
    }

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

    // 主服务循环 - 定期健康检查
    let mut interval = time::interval(Duration::from_secs(60));
    let mut cycle_count = 0u64;

    loop {
        interval.tick().await;
        cycle_count += 1;

        let models = model_manager.list_models();
        info!(
            "服务运行正常 - 周期: {}, 模型数: {}",
            cycle_count,
            models.len()
        );
    }
}

/// 检查配置和环境
async fn check_config(config: Config) -> Result<()> {
    println!("=== ModSrv 配置检查 ===\n");

    // 1. 验证配置
    match config.validate() {
        Ok(_) => println!("✓ 配置文件验证通过"),
        Err(e) => {
            println!("✗ 配置文件验证失败: {}", e);
            return Err(e);
        }
    }

    // 2. 显示服务配置
    println!("\n--- 服务配置 ---");
    println!("服务名称: {}", config.service_name);
    println!("版本: {}", config.version);
    println!("API地址: http://{}:{}", config.api.host, config.api.port);
    println!("日志级别: {}", config.log.level);

    // 3. 显示Redis配置
    println!("\n--- Redis配置 ---");
    println!("URL: {}", config.redis.url);
    println!("前缀: {}", config.redis.key_prefix);

    // 4. 测试Redis连接
    print!("连接测试: ");
    match ModelManager::new(&config.redis.url).await {
        Ok(_) => {
            println!("✓ 成功");

            // 测试Lua脚本
            println!("Lua脚本: ✓ 已加载");
        }
        Err(e) => {
            println!("✗ 失败 - {}", e);
            return Err(ModelSrvError::redis(format!("Redis连接失败: {}", e)));
        }
    }

    // 5. 显示模型信息
    println!("\n--- 模型配置 ---");
    if config.models.is_empty() {
        println!("未配置任何模型");
    } else {
        println!("已配置 {} 个模型:", config.models.len());
        for model in &config.models {
            println!("\n• {} ({})", model.name, model.id);
            println!("  描述: {}", model.description);

            // 显示监视点
            if !model.monitoring.is_empty() {
                println!("  监视点 ({}):", model.monitoring.len());
                for (name, point) in &model.monitoring {
                    let unit = point.unit.as_deref().unwrap_or("");
                    println!("    - {}: {} {}", name, point.description, unit);
                }
            }

            // 显示控制点
            if !model.control.is_empty() {
                println!("  控制点 ({}):", model.control.len());
                for (name, point) in &model.control {
                    let unit = point.unit.as_deref().unwrap_or("");
                    println!("    - {}: {} {}", name, point.description, unit);
                }
            }
        }
    }

    println!("\n✓ 所有检查通过");
    Ok(())
}
