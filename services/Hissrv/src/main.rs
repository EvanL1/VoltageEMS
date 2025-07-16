mod api;
mod config;
mod data_processor;
mod error;
mod influx_client;
mod redis_client;

use crate::api::start_api_server;
use crate::config::Config;
use crate::data_processor::DataProcessor;
use crate::error::{HisSrvError, Result};
use crate::influx_client::InfluxDBClient;
use crate::redis_client::{create_message_channel, RedisSubscriber};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    init_logging()?;

    // 显示启动信息
    print_banner();

    // 加载配置
    let config = match Config::load() {
        Ok(config) => {
            info!("配置加载成功");
            Arc::new(config)
        }
        Err(e) => {
            error!("配置加载失败: {}", e);
            std::process::exit(1);
        }
    };

    // 验证 InfluxDB 是否启用
    if !config.influxdb.enabled {
        error!("InfluxDB 未启用，HisSrv 需要 InfluxDB 才能工作");
        std::process::exit(1);
    }

    // 初始化 InfluxDB 客户端
    let influx_client = InfluxDBClient::new(config.influxdb.clone());
    
    // 测试 InfluxDB 连接
    info!("测试 InfluxDB 连接...");
    if let Err(e) = influx_client.ping().await {
        error!("InfluxDB 连接失败: {}", e);
        warn!("请确保 InfluxDB 3.2 正在运行并且配置正确");
        std::process::exit(1);
    }
    info!("InfluxDB 连接成功");

    let influx_client = Arc::new(influx_client);

    // 创建消息通道
    let (message_sender, message_receiver) = create_message_channel();

    // 创建数据处理器
    let data_processor = DataProcessor::new(
        (*influx_client).clone(),
        message_receiver,
        config.influxdb.flush_interval_seconds,
        config.points.clone(),
    );

    // 创建处理器统计的共享状态
    let processing_stats = Arc::new(Mutex::new(crate::data_processor::ProcessingStats::default()));

    // 启动数据处理器
    let processor_stats_clone = Arc::clone(&processing_stats);
    let processor_handle = tokio::spawn(async move {
        if let Err(e) = data_processor.start_processing().await {
            error!("数据处理器错误: {}", e);
        }
    });

    // 初始化 Redis 订阅器
    let mut redis_subscriber = RedisSubscriber::new(config.redis.clone(), message_sender);

    // 连接到 Redis
    info!("连接到 Redis...");
    if let Err(e) = redis_subscriber.connect().await {
        error!("Redis 连接失败: {}", e);
        warn!("请确保 Redis 正在运行并且配置正确");
        std::process::exit(1);
    }
    info!("Redis 连接成功");

    // 启动 Redis 订阅器
    let subscriber_handle = tokio::spawn(async move {
        if let Err(e) = redis_subscriber.start_listening().await {
            error!("Redis 订阅器错误: {}", e);
        }
    });

    // 创建点位配置的共享状态
    let points_config = Arc::new(Mutex::new(config.points.clone()));

    // 启动 API 服务器（如果配置了）
    let api_handle = if config.service.port > 0 {
        let api_config = Arc::clone(&config);
        let api_influx_client = Arc::clone(&influx_client);
        let api_stats = Arc::clone(&processing_stats);
        let api_points_config = Arc::clone(&points_config);

        Some(tokio::spawn(async move {
            info!("启动 API 服务器在端口 {}", api_config.service.port);
            if let Err(e) = start_api_server(api_config, api_influx_client, api_stats, api_points_config).await {
                error!("API 服务器错误: {}", e);
            }
        }))
    } else {
        info!("API 服务器已禁用 (port = 0)");
        None
    };

    // 启动完成
    info!("HisSrv 启动完成！");
    print_service_info(&config);

    // 等待退出信号
    wait_for_shutdown_signal().await;

    // 优雅关闭
    info!("收到关闭信号，开始优雅关闭...");

    // 等待任务完成或超时
    let shutdown_timeout = std::time::Duration::from_secs(10);
    
    tokio::select! {
        _ = processor_handle => info!("数据处理器已停止"),
        _ = subscriber_handle => info!("Redis 订阅器已停止"),
        _ = async { if let Some(handle) = api_handle { handle.await.ok(); } } => info!("API 服务器已停止"),
        _ = tokio::time::sleep(shutdown_timeout) => warn!("关闭超时"),
    }

    // 最终刷新 InfluxDB 缓冲区
    if let Err(e) = influx_client.flush().await {
        error!("最终刷新失败: {}", e);
    } else {
        info!("最终刷新完成");
    }

    info!("HisSrv 已完全关闭");
    Ok(())
}

/// 初始化日志系统
fn init_logging() -> Result<()> {
    // 设置默认的环境变量（如果没有设置）
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,hissrv=debug");
    }

    // 创建日志订阅器
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,hissrv=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false));

    subscriber.init();

    Ok(())
}

/// 显示启动横幅
fn print_banner() {
    println!();
    println!("██╗  ██╗██╗███████╗███████╗██████╗ ██╗   ██╗");
    println!("██║  ██║██║██╔════╝██╔════╝██╔══██╗██║   ██║");
    println!("███████║██║███████╗███████╗██████╔╝██║   ██║");
    println!("██╔══██║██║╚════██║╚════██║██╔══██╗╚██╗ ██╔╝");
    println!("██║  ██║██║███████║███████║██║  ██║ ╚████╔╝ ");
    println!("╚═╝  ╚═╝╚═╝╚══════╝╚══════╝╚═╝  ╚═╝  ╚═══╝  ");
    println!();
    println!("              历史数据服务 v0.2.0");
    println!("          Redis → InfluxDB 3.2 数据传输");
    println!();
}

/// 显示服务信息
fn print_service_info(config: &Config) {
    info!("服务配置:");
    info!("  名称: {}", config.service.name);
    info!("  版本: {}", config.service.version);
    if config.service.port > 0 {
        info!("  API 端口: {}", config.service.port);
        info!("  API 地址: http://{}:{}", config.service.host, config.service.port);
    }
    
    info!("Redis 配置:");
    info!("  地址: {}:{}", config.redis.connection.host, config.redis.connection.port);
    info!("  数据库: {}", config.redis.connection.database);
    info!("  订阅模式: {:?}", config.redis.subscription.patterns);
    if let Some(ref channel_ids) = config.redis.subscription.channel_ids {
        info!("  监控通道: {:?}", channel_ids);
    } else {
        info!("  监控通道: 全部");
    }
    
    info!("InfluxDB 配置:");
    info!("  地址: {}", config.influxdb.url);
    info!("  数据库: {}", config.influxdb.database);
    info!("  批量大小: {}", config.influxdb.batch_size);
    info!("  刷新间隔: {}s", config.influxdb.flush_interval_seconds);
    
    info!("日志配置:");
    info!("  级别: {}", config.logging.level);
    info!("  格式: {}", config.logging.format);
    if let Some(ref file) = config.logging.file {
        info!("  文件: {}", file);
    }
    
    info!("点位配置:");
    info!("  启用: {}", config.points.enabled);
    info!("  默认策略: {:?}", config.points.default_policy);
    info!("  通道规则: {} 个", config.points.rules.channels.len());
    info!("  点位规则: {} 个", config.points.rules.points.len());
    info!("  过滤器: {} 个", config.points.filters.len());
    let configured_channels = config.points.get_configured_channels();
    if !configured_channels.is_empty() {
        info!("  已配置通道: {:?}", configured_channels);
    }
    
    println!();
}

/// 等待关闭信号
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("收到 Ctrl+C 信号");
        },
        _ = terminate => {
            info!("收到终止信号");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging() {
        // 测试日志初始化不会崩溃
        // 注意：这可能会与其他测试冲突，因为日志只能初始化一次
        // 在实际项目中，通常会使用更复杂的测试设置
        std::env::set_var("RUST_LOG", "debug");
        // init_logging().unwrap(); // 注释掉以避免重复初始化
    }

    #[tokio::test]
    async fn test_config_loading() {
        // 测试默认配置可以正常加载
        let config = Config::default();
        assert_eq!(config.service.name, "hissrv");
        assert_eq!(config.service.version, "0.2.0");
        assert!(config.influxdb.enabled);
    }
}