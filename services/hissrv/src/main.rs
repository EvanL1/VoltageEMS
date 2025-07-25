//! hissrv - 标准化Redis到InfluxDB数据桥接服务
//! 严格遵循Redis数据结构规范v3.2，专注处理comsrv数据

mod config;
mod error;
mod processor;
mod subscriber;

use config::Config;
use error::Result;
use processor::StandardDataProcessor;
use subscriber::StandardRedisSubscriber;
use tokio::{signal, sync::mpsc};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // 加载标准化配置
    let config = Config::load()?;

    // 初始化日志
    init_logging(&config.logging.level);

    info!(
        "启动标准化 {} v{} - Redis数据结构规范v3.2",
        config.service.name, config.service.version
    );

    // 输出多服务配置信息
    info!(
        "Redis配置: {}:{} - 订阅模式: {:?}",
        config.redis.connection.host,
        config.redis.connection.port,
        config.redis.get_all_patterns()
    );
    info!("InfluxDB配置: {}", config.influxdb.connection.url);
    info!("启用的服务: {:?}", config.redis.get_enabled_services());

    // 创建双通道：消息通知 + Hash批量数据
    let (message_sender, message_receiver) = mpsc::unbounded_channel();
    let (batch_sender, batch_receiver) = mpsc::unbounded_channel();

    // 创建标准化Redis订阅器
    let subscriber = StandardRedisSubscriber::new(config.redis.clone()).await?;

    // 创建标准化数据处理器
    let processor =
        StandardDataProcessor::new(config.influxdb.clone(), config.redis.clone()).await?;

    // 启动标准订阅任务
    let subscriber_handle = {
        tokio::spawn(async move {
            if let Err(e) = subscriber
                .start_standard_subscribe(message_sender, batch_sender)
                .await
            {
                error!("标准Redis订阅器错误: {}", e);
            }
        })
    };

    // 启动标准处理任务
    let processor_handle = {
        tokio::spawn(async move {
            if let Err(e) = processor
                .start_standard_processing(message_receiver, batch_receiver)
                .await
            {
                error!("标准数据处理器错误: {}", e);
            }
        })
    };

    info!("多服务hissrv启动完成，开始处理多服务数据...");

    // 等待关闭信号
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("收到关闭信号，正在停止多服务...");
        }
        Err(err) => {
            error!("监听关闭信号失败: {}", err);
        }
    }

    // 优雅关闭
    subscriber_handle.abort();
    processor_handle.abort();

    let _ = tokio::join!(subscriber_handle, processor_handle);

    info!("多服务hissrv已停止");
    Ok(())
}

/// 初始化日志系统
fn init_logging(level: &str) {
    let filter = match level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}
