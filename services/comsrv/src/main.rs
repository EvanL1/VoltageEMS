use std::env;
use std::sync::Arc;
use std::net::SocketAddr;

use clap::Parser;
use dotenv::dotenv;
use tokio::sync::RwLock;
use tokio::signal;
use axum::serve;

use tracing::{error, info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use comsrv::core::config::ConfigManager;
use comsrv::core::protocols::common::combase::protocol_factory::ProtocolFactory;
use comsrv::api::openapi_routes::create_api_routes;
use comsrv::service_impl::{start_communication_service, start_cleanup_task, shutdown_handler};
use comsrv::utils::error::Result;

/// Custom formatter that shows target only for DEBUG and ERROR levels
struct ConditionalTargetFormatter;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for ConditionalTargetFormatter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        let level = metadata.level();
        
        // Write timestamp
        write!(writer, "{} ", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6fZ"))?;
        
        // Write level with color
        match *level {
            Level::ERROR => write!(writer, "\x1b[31mERROR\x1b[0m")?,
            Level::WARN => write!(writer, "\x1b[33m WARN\x1b[0m")?,
            Level::INFO => write!(writer, "\x1b[32m INFO\x1b[0m")?,
            Level::DEBUG => write!(writer, "\x1b[34mDEBUG\x1b[0m")?,
            Level::TRACE => write!(writer, "\x1b[35mTRACE\x1b[0m")?,
        }
        
        // Show target only for DEBUG and ERROR levels
        if *level == Level::DEBUG || *level == Level::ERROR {
            write!(writer, " \x1b[2m{}\x1b[0m", metadata.target())?;
        }
        
        write!(writer, " ")?;
        
        // Write message
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        
        writeln!(writer)?;
        Ok(())
    }
}

/// Command line arguments for the Communication Service
#[derive(Parser)]
#[command(
    name = "comsrv",
    version = env!("CARGO_PKG_VERSION"),
    about = "Communication Service for Industrial Protocols",
    long_about = "A high-performance communication service supporting Modbus, IEC 60870-5-104, and other industrial protocols"
)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config/comsrv.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load environment variables
    dotenv().ok();

    // Load configuration (with basic console output)
    eprintln!("Loading configuration from: {}", args.config);
    if let Ok(url) = std::env::var("CONFIG_CENTER_URL") {
        eprintln!("Config center URL detected: {}", url);
    }
    
    let config_manager = Arc::new(
        ConfigManager::load_async(&args.config).await
            .map_err(|e| {
                eprintln!("Failed to load configuration: {}", e);
                e
            })?
    );

    // Initialize logging/tracing with configuration and channels
    initialize_logging(&config_manager.config().service.logging, &config_manager.config().channels)?;

    info!("Starting Communication Service v{}", env!("CARGO_PKG_VERSION"));
    
    // Display configuration summary
    info!("Configuration loaded successfully:");
    info!("  - Service name: {}", config_manager.config().service.name);
    info!("  - Channels configured: {}", config_manager.config().channels.len());
    info!("  - API enabled: {}", config_manager.config().service.api.enabled);
    info!("  - Redis enabled: {}", config_manager.config().service.redis.enabled);

    // Initialize plugin system
    info!("Initializing plugin system...");
    comsrv::core::plugins::init_plugin_system()
        .map_err(|e| {
            error!("Failed to initialize plugin system: {}", e);
            e
        })?;

    // Create protocol factory
    let factory = Arc::new(RwLock::new(ProtocolFactory::new()));

    // Start communication service (initializes channels, Redis, etc.)
    info!("Starting communication channels...");
    start_communication_service(config_manager.clone(), factory.clone()).await?;

    // Start cleanup task
    let cleanup_factory = factory.clone();
    let cleanup_handle = tokio::spawn(async move {
        if let Err(e) = start_cleanup_task(cleanup_factory).await {
            error!("Cleanup task error: {}", e);
        }
    });

    // Start API server if enabled
    let api_handle = if config_manager.config().service.api.enabled {
        let bind_address = &config_manager.config().service.api.bind_address;
        let addr: SocketAddr = bind_address.parse()
            .map_err(|e| {
                error!("Invalid API bind address '{}': {}", bind_address, e);
                comsrv::utils::error::ComSrvError::ConfigError(
                    format!("Invalid API bind address: {}", e)
                )
            })?;

        info!("Starting API server on {}", addr);
        
        let app = create_api_routes(factory.clone());
        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        Some(tokio::spawn(async move {
            if let Err(e) = serve(listener, app).await {
                error!("API server error: {}", e);
            }
        }))
    } else {
        info!("API server disabled in configuration");
        None
    };

    info!("Communication service started successfully");
    info!("Press Ctrl+C to shutdown");

    // Wait for shutdown signal
    shutdown_signal().await;

    info!("Shutting down communication service...");

    // Shutdown channels
    shutdown_handler(factory.clone()).await;

    // Cancel background tasks
    cleanup_handle.abort();
    if let Some(api_handle) = api_handle {
        api_handle.abort();
    }

    // Give tasks time to cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("Communication service shutdown complete");
    Ok(())
}

/// Initialize logging/tracing with configuration
fn initialize_logging(
    logging_config: &comsrv::core::config::types::LoggingConfig,
    channels: &[comsrv::core::config::types::ChannelConfig],
) -> Result<()> {
    use std::path::Path;
    use tracing_subscriber::filter::FilterFn;
    
    // Create log level filter - allow RUST_LOG to override, then use config, then default
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            let level = &logging_config.level;
            // Default filter
            format!("comsrv={},tower_http=info", level).into()
        });

    // Start with the registry 
    let subscriber = tracing_subscriber::registry();

    // Add console layer if enabled
    if logging_config.console && logging_config.file.is_some() {
        // Both console and file logging
        
        // Create main filter - exclude channel-specific logs from main logs
        let main_filter = FilterFn::new(|metadata| {
            // Check if this is a modbus packet log by looking for specific field names
            let fields = metadata.fields();
            // Check if the field names contain "direction" which indicates a packet log
            for field in fields.iter() {
                if field.name() == "direction" {
                    return false; // Exclude packet logs from main log
                }
            }
            true // Include everything else
        });
        
        // Console layer with filter
        let console_layer = tracing_subscriber::fmt::layer()
            .event_format(ConditionalTargetFormatter)
            .with_filter(main_filter.clone());

        let log_file_path = logging_config.file.as_ref().unwrap();
        let log_path = Path::new(log_file_path);
        let log_dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let log_filename = log_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("comsrv");
        
        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            eprintln!("Warning: Could not create log directory {:?}: {}", log_dir, e);
        }

        // Create rolling file appender for main log
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(log_filename)
            .filename_suffix("log")
            .max_log_files(logging_config.max_files as usize)
            .build(log_dir)
            .map_err(|e| {
                eprintln!("Failed to create file appender: {}", e);
                comsrv::utils::error::ComSrvError::ConfigError(
                    format!("Failed to create log file appender: {}", e)
                )
            })?;

        // File layer with filter
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_ansi(false)
            .json()
            .with_filter(main_filter);

        // Apply env filter and layers
        let subscriber = subscriber
            .with(env_filter)
            .with(console_layer)
            .with(file_layer);
        
        // Initialize the subscriber first
        subscriber.init();
        
        eprintln!("Logging configured:");
        eprintln!("  - Console: enabled");
        eprintln!("  - File: {}", log_file_path);
        eprintln!("  - Level: {}", logging_config.level);
        eprintln!("  - Max files: {}", logging_config.max_files);
        
    } else if logging_config.console {
        // Console only
        let console_layer = tracing_subscriber::fmt::layer()
            .event_format(ConditionalTargetFormatter);

        subscriber
            .with(env_filter)
            .with(console_layer)
            .init();
        eprintln!("Logging configured: Console only, Level: {}", logging_config.level);
        
    } else if let Some(ref log_file_path) = logging_config.file {
        // File only
        let log_path = Path::new(log_file_path);
        let log_dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let log_filename = log_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("comsrv");
        
        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            eprintln!("Warning: Could not create log directory {:?}: {}", log_dir, e);
        }

        // Create rolling file appender
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(log_filename)
            .filename_suffix("log")
            .max_log_files(logging_config.max_files as usize)
            .build(log_dir)
            .map_err(|e| {
                eprintln!("Failed to create file appender: {}", e);
                comsrv::utils::error::ComSrvError::ConfigError(
                    format!("Failed to create log file appender: {}", e)
                )
            })?;

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_ansi(false)
            .json();

        subscriber
            .with(env_filter)
            .with(file_layer)
            .init();
        
        eprintln!("Logging configured:");
        eprintln!("  - Console: disabled");
        eprintln!("  - File: {}", log_file_path);
        eprintln!("  - Level: {}", logging_config.level);
        eprintln!("  - Max files: {}", logging_config.max_files);
        
    } else {
        eprintln!("Warning: No logging outputs configured!");
        return Ok(());
    }

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
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
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }
}