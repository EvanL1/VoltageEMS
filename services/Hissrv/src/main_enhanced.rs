use crate::api::start_api_server;
use crate::batch_writer::{BatchWriteBuffer, BatchWriter};
use crate::config::Config;
use crate::enhanced_message_processor::EnhancedMessageProcessor;
use crate::error::{HisSrvError, Result};
use crate::monitoring::MetricsCollector;
use crate::redis_subscriber::RedisSubscriber;
use crate::retention_policy::RetentionPolicyManager;
use crate::storage::{
    influxdb_storage::{InfluxDBBatchWriter, InfluxDBStorage},
    redis_storage::RedisStorage,
    StorageManager,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

pub async fn main_enhanced() -> Result<()> {
    info!("Starting hissrv service (Enhanced Version)");

    // 初始化配置
    let config = Config::load().await?;

    // 初始化日志系统
    // Note: enhanced_logging field doesn't exist in config, always use standard logging
    crate::logging::init_logging(&config.logging)?;

    // 初始化存储管理器
    let storage_manager = Arc::new(RwLock::new(StorageManager::new()));

    // 配置 InfluxDB 存储
    let influx_config = &config.storage.backends.influxdb;
    if influx_config.enabled {
        let influx_storage = InfluxDBStorage::new(influx_config).await?;

        // 创建批量写入器
        let batch_writer = InfluxDBBatchWriter::new(Arc::new(influx_storage));
        let batch_buffer = BatchWriteBuffer::new(
            Box::new(batch_writer),
            influx_config.batch_size,
            Duration::from_secs(influx_config.flush_interval),
        );

        storage_manager
            .write()
            .await
            .add_backend("influxdb", Arc::new(batch_buffer))?;
    }

    // Note: Redis storage configuration is not in the config structure
    // This would need to be added if Redis storage is required

    // 初始化监控
    let metrics = Arc::new(MetricsCollector::new());
    let metrics_clone = metrics.clone();

    // 初始化保留策略管理器
    let retention_manager = Arc::new(RetentionPolicyManager::default());

    // 启动保留策略定时任务
    let retention_clone = retention_manager.clone();
    let storage_clone = storage_manager.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600)); // 每小时执行一次
        loop {
            interval.tick().await;
            info!("Running retention policy cleanup");
            if let Err(e) = retention_clone.execute_cleanup(&storage_clone).await {
                error!("Retention policy cleanup failed: {}", e);
            }
        }
    });

    // 初始化消息处理器
    // Note: data_processing config doesn't exist, using data config instead
    let processor = Arc::new(EnhancedMessageProcessor::new(
        storage_manager.clone(),
        config.data.clone(),
        metrics.clone(),
    ));

    // 初始化 Redis 订阅器
    let subscriber = RedisSubscriber::new(
        config.redis.connection.clone(),
        config.redis.subscription.clone(),
        processor,
    )?;

    // 启动订阅器
    let subscriber_handle = tokio::spawn(async move {
        if let Err(e) = subscriber.start().await {
            error!("Redis subscriber error: {}", e);
        }
    });

    // 启动 API 服务器
    let api_config = config.api.clone();
    let api_handle = tokio::spawn(async move {
        if let Err(e) = start_api_server(api_config, storage_manager, metrics_clone).await {
            error!("API server error: {}", e);
        }
    });

    // 等待退出信号
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = subscriber_handle => {
            warn!("Subscriber task ended unexpectedly");
        }
        _ = api_handle => {
            warn!("API server task ended unexpectedly");
        }
    }

    info!("hissrv service shutting down");
    Ok(())
}
