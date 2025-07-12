//! modsrv异步版本入口
//!
//! 使用优化的异步引擎运行模型服务

use clap::{Parser, Subcommand};
use modsrv::{
    api::ApiServer,
    config::{Config, ConfigLoader},
    engine::{EngineConfig, OptimizedModelEngine},
    error::Result,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::time;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Run in async mode
    #[arg(long)]
    async_mode: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run the model service
    Run {
        /// Model execution interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },
    /// Show performance statistics
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config = if let Some(config_path) = args.config {
        let loader = ConfigLoader::new()
            .with_file(config_path.to_string_lossy())
            .with_env_prefix("MODSRV_");
        loader.load()?
    } else {
        Config::default()
    };

    // Initialize logging
    tracing_subscriber::fmt::init();

    match args.command {
        Some(Commands::Run { interval }) => run_async_engine(config, interval).await,
        Some(Commands::Stats) => show_stats().await,
        None => run_async_engine(config, 5).await,
    }
}

async fn run_async_engine(config: Config, interval_secs: u64) -> Result<()> {
    info!("Starting modsrv in async mode");

    // Create engine configuration
    let engine_config = EngineConfig {
        batch_size: config.performance.redis_batch_size,
        cache_ttl: Duration::from_secs(config.performance.cache_ttl_secs),
        execution_timeout: Duration::from_secs(config.performance.model_timeout_secs),
        parallel_execution: config.performance.enable_metrics,
        max_concurrent_models: config.performance.max_concurrent_models,
    };

    // Create optimized engine
    let engine = Arc::new(OptimizedModelEngine::new(engine_config).await?);

    // Load initial models
    let pattern = format!("{}model:*", config.redis.key_prefix);
    engine.load_models(&pattern).await?;

    // Start API server if enabled
    let api_handle = if true {
        // API always enabled for now
        let api_server = ApiServer::new(config.api.host.clone(), config.api.port, engine.clone());
        Some(tokio::spawn(async move {
            if let Err(e) = api_server.run().await {
                error!("API server error: {}", e);
            }
        }))
    } else {
        None
    };

    // Start cache cleanup task
    let cleanup_engine = engine.clone();
    let cleanup_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_engine.cleanup_cache().await;
        }
    });

    // Start model reload task
    let reload_engine = engine.clone();
    let reload_pattern = pattern.clone();
    let reload_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            info!("Reloading models...");
            if let Err(e) = reload_engine.load_models(&reload_pattern).await {
                error!("Failed to reload models: {}", e);
            }
        }
    });

    // Main execution loop
    let mut interval = time::interval(Duration::from_secs(interval_secs));
    let mut shutdown_rx = shutdown_signal();

    info!(
        "Starting main execution loop with {}s interval",
        interval_secs
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let start = std::time::Instant::now();

                match engine.execute_all_models().await {
                    Ok(_) => {
                        let duration = start.elapsed();
                        info!("Model execution completed in {:?}", duration);

                        // Log statistics periodically
                        let stats = engine.get_stats().await;
                        info!(
                            "Stats: {} executions, {} successful, cache hit rate: {:.2}%",
                            stats.total_executions,
                            stats.successful_executions,
                            (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses).max(1) as f64) * 100.0
                        );
                    }
                    Err(e) => {
                        error!("Model execution failed: {}", e);
                    }
                }
            }
            _ = &mut shutdown_rx => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    // Cleanup
    info!("Shutting down...");

    cleanup_handle.abort();
    reload_handle.abort();

    if let Some(api_handle) = api_handle {
        api_handle.abort();
    }

    info!("modsrv stopped");
    Ok(())
}

async fn show_stats() -> Result<()> {
    // TODO: Connect to running instance and show stats
    println!("Statistics display not yet implemented");
    Ok(())
}

async fn shutdown_signal() {
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
