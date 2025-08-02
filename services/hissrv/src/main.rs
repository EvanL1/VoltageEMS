//! hissrv - Minimal Redis to InfluxDB data bridge service
//! Designed for edge devices, implements data archival using polling mode

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
    // Initialize logging
    init_logging();

    // Load configuration
    let config = config::Config::load()?;

    // Get configuration information
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

    // Create shared configuration
    let shared_config = Arc::new(RwLock::new(config));
    let config_path = "config/hissrv.yaml".to_string();

    // Create configuration update channel (if API is enabled)
    let (tx, rx) = if enable_api {
        let (tx, rx) = mpsc::channel::<()>(10);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Create poller
    let poller = if let Some(rx) = rx {
        Poller::with_update_channel(shared_config.clone(), rx).await?
    } else {
        Poller::new(shared_config.clone()).await?
    };

    // Start API server (if enabled)
    let api_handle = if enable_api {
        let api_config = shared_config.clone();
        let api_tx = tx.clone().expect("tx should be Some when API is enabled");
        let api_config_path = config_path.clone();

        tracing::info!("Starting configuration API server on port {}", api_port);

        Some(tokio::spawn(async move {
            // Create API state with notification feature
            let state = api::ApiState::with_update_channel(api_config, api_config_path, api_tx);
            let app = api::create_router(state);

            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], api_port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Failed to bind API server: {}", e);
                    return;
                },
            };

            if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                tracing::error!("API server error: {}", e);
            }
        }))
    } else {
        None
    };

    // Run main loop
    let poller_handle = tokio::spawn(async move {
        if let Err(e) = poller.run().await {
            tracing::error!("Poller error: {}", e);
        }
    });

    // Set up signal handling
    let reload_tx = tx.clone();
    let shared_config_for_signal = shared_config.clone();

    tokio::spawn(async move {
        // Listen for SIGHUP signal for configuration reload
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sighup = match signal(SignalKind::hangup()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to create SIGHUP listener: {}", e);
                    return;
                },
            };

            loop {
                sighup.recv().await;
                tracing::info!("Received SIGHUP, reloading configuration...");

                // Reload configuration
                match config::Config::reload() {
                    Ok(new_config) => {
                        if let Err(e) = new_config.validate() {
                            tracing::error!("Invalid configuration: {}", e);
                            continue;
                        }

                        // Update shared configuration
                        match shared_config_for_signal.write() {
                            Ok(mut config) => {
                                *config = new_config;
                                tracing::info!("Configuration updated successfully");
                            },
                            Err(e) => {
                                tracing::error!("Failed to acquire write lock: {}", e);
                                continue;
                            },
                        }

                        // Notify poller (after lock is released)
                        if let Some(tx) = &reload_tx {
                            if let Err(e) = tx.send(()).await {
                                tracing::error!("Failed to notify poller: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to reload configuration: {}", e);
                    },
                }
            }
        }
    });

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Received shutdown signal");
        },
        Err(e) => {
            tracing::error!("Failed to listen for shutdown signal: {}", e);
        },
    }

    // Graceful shutdown
    poller_handle.abort();
    let _ = poller_handle.await;

    if let Some(api_handle) = api_handle {
        api_handle.abort();
        let _ = api_handle.await;
    }

    tracing::info!("{} stopped", SERVICE_NAME);
    Ok(())
}

/// Initialize logging system
fn init_logging() {
    // Read log level from environment variable, default to info
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
