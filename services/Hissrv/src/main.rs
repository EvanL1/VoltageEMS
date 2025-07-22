use crate::api::start_api_server;
use crate::config::Config;
use crate::error::{HisSrvError, Result};
use crate::monitoring::MetricsCollector;
use crate::message_processor::MessageProcessor;
use crate::redis_subscriber::RedisSubscriber;
use crate::storage::StorageManager;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

mod config;
// mod config_center; // TODO: Implement config center support
mod api;
mod batch_writer;
mod enhanced_message_processor;
mod error;
mod influx_client;
mod influxdb_handler;
mod logging;
mod main_enhanced;
mod message_processor;
mod monitoring;
mod query_optimizer;
mod redis_client;
mod redis_subscriber;
mod retention_policy;
mod storage;
mod types;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    // 检查是否使用增强版本
    if std::env::var("HISSRV_ENHANCED").unwrap_or_default() == "true" {
        return main_enhanced::main_enhanced().await;
    }

    // 设置panic处理器
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };

        eprintln!("PANIC at {}: {}", location, message);
        tracing::error!(
            location = location,
            message = message,
            "Application panicked"
        );
    }));

    // Load configuration from config center or local file
    let config = match Config::load().await {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            if let Some(suggestion) = e.recovery_suggestion() {
                eprintln!("建议: {}", suggestion);
            }
            std::process::exit(1);
        }
    };

    // 初始化增强日志系统（如果启用）
    let _enhanced_logger = if std::env::var("HISSRV_ENHANCED_LOGGING").unwrap_or_default() == "true"
    {
        match crate::logging::EnhancedLogger::init(config.logging.clone()) {
            Ok(logger) => {
                tracing::info!("Enhanced logging system initialized");
                Some(logger)
            }
            Err(e) => {
                eprintln!(
                    "Failed to initialize enhanced logging, falling back to standard: {}",
                    e
                );
                // 回退到标准日志系统
                if let Err(e) = crate::logging::init_logging(&config.logging) {
                    eprintln!("Failed to initialize standard logging: {}", e);
                    std::process::exit(1);
                }
                None
            }
        }
    } else {
        // Initialize standard logging
        if let Err(e) = crate::logging::init_logging(&config.logging) {
            eprintln!("Failed to initialize logging: {}", e);
            std::process::exit(1);
        }
        None
    };

    // 记录启动信息
    tracing::info!(
        service_name = config.service.name,
        version = config.service.version,
        pid = std::process::id(),
        "Starting HisSrv"
    );
    tracing::info!(
        config_file = config.config_file,
        environment = std::env::var("HISSRV_ENV").unwrap_or_else(|_| "production".to_string()),
        "Configuration loaded"
    );

    // 验证配置
    if let Err(e) = validate_config(&config) {
        tracing::error!(error = %e, "Configuration validation failed");
        return Err(e);
    }

    // Initialize storage manager with error handling
    let mut storage_manager = StorageManager::new();

    // Setup InfluxDB storage backend
    if config.storage.backends.influxdb.enabled {
        tracing::info!("Initializing InfluxDB storage backend");
        storage_manager
            .add_influxdb_backend(
                "influxdb",
                &config.storage.backends.influxdb.url,
                &config.storage.backends.influxdb.database,
                config.storage.backends.influxdb.username.as_deref(),
                config.storage.backends.influxdb.password.as_deref(),
            )
            .await?;
        tracing::info!(
            url = config.storage.backends.influxdb.url,
            database = config.storage.backends.influxdb.database,
            "InfluxDB backend configured"
        );
    }

    // Setup Redis storage backend
    tracing::info!("Initializing Redis storage backend");
    let redis_url = format!(
        "redis://{}:{}",
        config.redis.connection.host, config.redis.connection.port
    );
    storage_manager
        .add_redis_backend("redis", &redis_url)
        .await?;
    tracing::info!(
        host = config.redis.connection.host,
        port = config.redis.connection.port,
        "Redis backend configured"
    );

    // Default backend is set automatically in voltage-storage
    tracing::info!(
        default_backend = config.storage.default,
        "Default storage backend configured"
    );

    // Connect to all storage backends with retry logic
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 3;
    loop {
        match storage_manager.connect_all().await {
            Ok(_) => {
                tracing::info!("All storage backends connected successfully");
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    tracing::error!(
                        error = %e,
                        retries = retry_count,
                        "Failed to connect to storage backends after maximum retries"
                    );
                    return Err(e);
                }

                let retry_delay = 2u64.pow(retry_count - 1);
                tracing::warn!(
                    error = %e,
                    retry_count = retry_count,
                    retry_delay_secs = retry_delay,
                    "Storage connection failed, retrying..."
                );
                tokio::time::sleep(Duration::from_secs(retry_delay)).await;
            }
        }
    }

    // Wrap storage manager in Arc<RwLock> for shared access
    let storage_manager = Arc::new(RwLock::new(storage_manager));

    // Initialize metrics collector
    let metrics_collector = MetricsCollector::new();
    tracing::info!("Metrics collector initialized");

    // Setup message processing pipeline
    let (message_sender, message_receiver) = mpsc::unbounded_channel();

    // Clone storage_manager for the message processor
    let storage_manager_for_processor = Arc::clone(&storage_manager);
    let mut message_processor =
        MessageProcessor::new(storage_manager_for_processor, message_receiver);

    // Setup Redis subscriber with connection validation
    let mut redis_subscriber = RedisSubscriber::new(config.redis.clone(), message_sender);

    match redis_subscriber.connect().await {
        Ok(_) => {
            tracing::info!(
                channels = ?config.redis.subscription.channels,
                "Redis subscriber connected"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect Redis subscriber");
            if let Some(suggestion) = e.recovery_suggestion() {
                tracing::error!(suggestion = suggestion, "Recovery suggestion");
            }
            return Err(e);
        }
    }

    // Setup graceful shutdown
    let shutdown_signal = setup_shutdown_signal();

    // Start background tasks with error handling
    let processor_handle = {
        let shutdown = shutdown_signal.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = message_processor.start_processing() => {
                    match result {
                        Ok(_) => tracing::info!("Message processor completed"),
                        Err(e) => {
                            tracing::error!(error = %e, "Message processor error");
                            crate::logging::enhanced::log_error_with_context(&e);
                        }
                    }
                }
                _ = shutdown => {
                    tracing::info!("Message processor shutting down");
                }
            }
        })
    };

    let subscriber_handle = {
        let shutdown = shutdown_signal.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = redis_subscriber.start_listening() => {
                    match result {
                        Ok(_) => tracing::info!("Redis subscriber completed"),
                        Err(e) => {
                            tracing::error!(error = %e, "Redis subscriber error");
                            crate::logging::enhanced::log_error_with_context(&e);
                        }
                    }
                }
                _ = shutdown => {
                    tracing::info!("Redis subscriber shutting down");
                }
            }
        })
    };

    // Start API server if enabled
    if config.api.enabled {
        let api_config = config.clone();
        let api_storage_manager = Arc::clone(&storage_manager);
        let api_metrics = metrics_collector.clone();

        let api_handle = {
            let shutdown = shutdown_signal.clone();
            tokio::spawn(async move {
                tokio::select! {
                    result = start_api_server(api_config, api_storage_manager) => {
                        match result {
                            Ok(_) => tracing::info!("API server completed"),
                            Err(e) => {
                                tracing::error!(error = %e, "API server error");
                                crate::logging::enhanced::log_error_with_context(&e);
                            }
                        }
                    }
                    _ = shutdown => {
                        tracing::info!("API server shutting down");
                    }
                }
            })
        };

        tracing::info!(
            address = format!("{}:{}", config.service.host, config.service.port),
            "API server started"
        );
        tracing::info!(
            swagger_url = format!(
                "http://{}:{}/api/v1/swagger-ui",
                config.service.host, config.service.port
            ),
            "Swagger UI available"
        );

        // Health check endpoint log
        tracing::info!(
            health_check_url = format!(
                "http://{}:{}/health",
                config.service.host, config.service.port
            ),
            "Health check endpoint available"
        );

        // Wait for shutdown signal or task completion
        tokio::select! {
            _ = shutdown_signal => {
                tracing::info!("Received shutdown signal");
            }
            _ = processor_handle => {
                tracing::warn!("Message processor stopped unexpectedly");
            }
            _ = subscriber_handle => {
                tracing::warn!("Redis subscriber stopped unexpectedly");
            }
            _ = api_handle => {
                tracing::warn!("API server stopped unexpectedly");
            }
        }
    } else {
        tracing::info!("API server is disabled");

        // Wait for shutdown signal or task completion
        tokio::select! {
            _ = shutdown_signal => {
                tracing::info!("Received shutdown signal");
            }
            _ = processor_handle => {
                tracing::warn!("Message processor stopped unexpectedly");
            }
            _ = subscriber_handle => {
                tracing::warn!("Redis subscriber stopped unexpectedly");
            }
        }
    }

    // Graceful shutdown
    tracing::info!("Initiating graceful shutdown");

    // 给予任务一些时间来完成当前操作
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Cleanup with timeout
    let cleanup_timeout = Duration::from_secs(10);
    match tokio::time::timeout(
        cleanup_timeout,
        storage_manager.write().await.disconnect_all(),
    )
    .await
    {
        Ok(Ok(_)) => {
            tracing::info!("All storage backends disconnected successfully");
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "Error disconnecting storage backends");
        }
        Err(_) => {
            tracing::error!("Timeout while disconnecting storage backends");
        }
    }

    // 记录最终指标
    let final_metrics = metrics_collector.get_snapshot();
    tracing::info!(
        total_messages = final_metrics.total_messages_processed,
        total_points = final_metrics.total_points_written,
        uptime_seconds = final_metrics.uptime_seconds,
        "Final metrics"
    );

    tracing::info!("HisSrv shutdown complete");
    Ok(())
}

/// 验证配置
fn validate_config(config: &Config) -> Result<()> {
    // 验证Redis配置
    if config.redis.connection.host.is_empty() && config.redis.connection.socket.is_empty() {
        return Err(HisSrvError::MissingConfig {
            field: "redis.connection.host or redis.connection.socket".to_string(),
        });
    }

    // 验证存储后端配置
    if config.storage.backends.influxdb.enabled {
        if config.storage.backends.influxdb.url.is_empty() {
            return Err(HisSrvError::MissingConfig {
                field: "storage.backends.influxdb.url".to_string(),
            });
        }
        if config.storage.backends.influxdb.database.is_empty() {
            return Err(HisSrvError::MissingConfig {
                field: "storage.backends.influxdb.database".to_string(),
            });
        }
    }

    // 验证API配置
    if config.api.enabled {
        if config.service.port == 0 {
            return Err(HisSrvError::ConfigError {
                message: "Invalid port number".to_string(),
                field: Some("service.port".to_string()),
                suggestion: Some("Port must be between 1 and 65535".to_string()),
            });
        }
    }

    // 验证日志配置
    if let Err(_) = config.logging.level.parse::<tracing::Level>() {
        return Err(HisSrvError::ConfigError {
            message: format!("Invalid log level: {}", config.logging.level),
            field: Some("logging.level".to_string()),
            suggestion: Some("Valid levels: trace, debug, info, warn, error".to_string()),
        });
    }

    Ok(())
}

/// 设置优雅关闭信号
fn setup_shutdown_signal() -> tokio::sync::watch::Receiver<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());

    tokio::spawn(async move {
        // 等待Ctrl+C信号
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Received SIGINT (Ctrl+C), initiating shutdown");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to listen for SIGINT");
            }
        }

        // Unix系统上监听SIGTERM
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to install SIGTERM handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, initiating shutdown");
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received SIGINT, initiating shutdown");
                }
            }
        }

        // 发送关闭信号
        let _ = shutdown_tx.send(());
    });

    shutdown_rx
}
