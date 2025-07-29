//! hissrv - 极简的 Redis 到 InfluxDB 数据桥接服务
//! 专为边端设备设计，使用轮询模式实现数据归档

#![allow(dependency_on_unit_never_type_fallback)]

mod api;
mod config;
mod poller;

use hissrv::{Result, SERVICE_NAME, SERVICE_VERSION};
use poller::Poller;
use std::sync::{Arc, RwLock};
use tokio::signal;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    init_logging();

    // 加载配置
    let config = config::Config::load()?;

    // 获取配置信息
    let (polling_interval, enable_api, api_port) = (
        config.service.polling_interval,
        config.service.enable_api,
        config.service.api_port,
    );

    tracing::info!(
        "Starting {} v{} - Polling interval: {:?}",
        SERVICE_NAME,
        SERVICE_VERSION,
        polling_interval
    );

    // 创建共享配置
    let shared_config = Arc::new(RwLock::new(config));
    let config_path = "config/hissrv.yaml".to_string();

    // 创建配置更新通道（如果启用 API）
    let (tx, rx) = if enable_api {
        let (tx, rx) = mpsc::channel::<()>(10);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // 创建轮询器
    let poller = if let Some(rx) = rx {
        Poller::with_update_channel(shared_config.clone(), rx).await?
    } else {
        Poller::new(shared_config.clone()).await?
    };

    // 启动 API 服务器（如果启用）
    let api_handle = if enable_api {
        let api_config = shared_config.clone();
        let api_tx = tx.clone().unwrap();
        let api_config_path = config_path.clone();

        tracing::info!("Starting configuration API server on port {}", api_port);

        Some(tokio::spawn(async move {
            // 创建带通知功能的 API 状态
            let state = api::ApiState::with_update_channel(api_config, api_config_path, api_tx);
            let app = api::create_router(state);

            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], api_port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Failed to bind API server: {}", e);
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                tracing::error!("API server error: {}", e);
            }
        }))
    } else {
        None
    };

    // 运行主循环
    let poller_handle = tokio::spawn(async move {
        if let Err(e) = poller.run().await {
            tracing::error!("Poller error: {}", e);
        }
    });

    // 设置信号处理
    let reload_tx = tx.clone();
    let shared_config_for_signal = shared_config.clone();

    tokio::spawn(async move {
        // 监听 SIGHUP 信号用于配置重载
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sighup = match signal(SignalKind::hangup()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to create SIGHUP listener: {}", e);
                    return;
                }
            };

            loop {
                sighup.recv().await;
                tracing::info!("Received SIGHUP, reloading configuration...");

                // 重载配置
                match config::Config::reload() {
                    Ok(new_config) => {
                        if let Err(e) = new_config.validate() {
                            tracing::error!("Invalid configuration: {}", e);
                            continue;
                        }

                        // 更新共享配置
                        match shared_config_for_signal.write() {
                            Ok(mut config) => {
                                *config = new_config;
                                tracing::info!("Configuration updated successfully");
                            }
                            Err(e) => {
                                tracing::error!("Failed to acquire write lock: {}", e);
                                continue;
                            }
                        }

                        // 通知 poller (在锁释放后)
                        if let Some(tx) = &reload_tx {
                            if let Err(e) = tx.send(()).await {
                                tracing::error!("Failed to notify poller: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to reload configuration: {}", e);
                    }
                }
            }
        }
    });

    // 等待关闭信号
    match signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Received shutdown signal");
        }
        Err(e) => {
            tracing::error!("Failed to listen for shutdown signal: {}", e);
        }
    }

    // 优雅关闭
    poller_handle.abort();
    let _ = poller_handle.await;

    if let Some(api_handle) = api_handle {
        api_handle.abort();
        let _ = api_handle.await;
    }

    tracing::info!("{} stopped", SERVICE_NAME);
    Ok(())
}

/// 初始化日志系统
fn init_logging() {
    // 从环境变量读取日志级别，默认为 info
    let log_level =
        std::env::var("RUST_LOG").unwrap_or_else(|_| format!("{}=info", env!("CARGO_PKG_NAME")));

    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();
}
