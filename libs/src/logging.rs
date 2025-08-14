//! Unified logging module for VoltageEMS services
//!
//! Provides multi-level logging support with automatic sub-logger creation
//! for channels (comsrv) and models (modsrv).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
pub use tracing_appender::rolling::Rotation;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

// Global logger registry for sub-loggers
lazy_static::lazy_static! {
    static ref SUB_LOGGERS: Arc<Mutex<HashMap<String, SubLogger>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref GUARDS: Arc<Mutex<Vec<WorkerGuard>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Logger configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Service name (e.g., "comsrv", "modsrv")
    pub service_name: String,
    /// Base directory for logs
    pub log_dir: PathBuf,
    /// Console log level
    pub console_level: Level,
    /// File log level
    pub file_level: Level,
    /// Enable JSON format for structured logging
    pub enable_json: bool,
    /// Log rotation strategy
    pub rotation: Rotation,
    /// Maximum number of log files to keep
    pub max_log_files: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            service_name: "unknown".to_string(),
            log_dir: PathBuf::from("logs"),
            console_level: Level::INFO,
            file_level: Level::DEBUG,
            enable_json: false,
            rotation: Rotation::DAILY,
            max_log_files: 30,
        }
    }
}

/// Sub-logger for channels, models, etc.
pub struct SubLogger {
    pub name: String,
    pub writer: tracing_appender::non_blocking::NonBlocking,
    _guard: WorkerGuard,
}

/// Initialize logging system with configuration
pub fn init_with_config(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create log directory if it doesn't exist
    fs::create_dir_all(&config.log_dir)?;

    // Create service log file
    let file_appender = RollingFileAppender::new(
        config.rotation,
        &config.log_dir,
        format!("{}.log", config.service_name),
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Store guard to prevent dropping
    GUARDS.lock().unwrap().push(guard);

    // Create the log file immediately to ensure it exists
    let log_file_path = config.log_dir.join(format!(
        "{}.log.{}",
        config.service_name,
        chrono::Local::now().format("%Y-%m-%d")
    ));
    if !log_file_path.exists() {
        fs::File::create(&log_file_path)?;
    }

    // Build subscriber with layers
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("info,{}=debug", config.service_name)));

    let registry = tracing_subscriber::registry().with(env_filter);

    // Console layer
    let console_layer = fmt::layer()
        .with_ansi(true)
        .with_level(true)
        .with_target(true)
        .with_thread_ids(false)
        .with_span_events(FmtSpan::NONE)
        .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
            config.console_level,
        ));

    // File layer
    let file_layer = if config.enable_json {
        fmt::layer()
            .json()
            .with_writer(non_blocking)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::FULL)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                config.file_level,
            ))
            .boxed()
    } else {
        fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::NONE)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                config.file_level,
            ))
            .boxed()
    };

    registry.with(console_layer).with(file_layer).init();

    tracing::info!(
        "Logging initialized for service: {} at {:?}",
        config.service_name,
        config.log_dir
    );

    Ok(())
}

/// Create a channel-specific logger (for comsrv)
pub fn create_channel_logger(
    service_dir: &Path,
    channel_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let logger_key = format!("channel_{}", channel_id);

    // Check if already exists
    if SUB_LOGGERS.lock().unwrap().contains_key(&logger_key) {
        return Ok(());
    }

    // Create channels directory
    let channels_dir = service_dir.join("channels");
    fs::create_dir_all(&channels_dir)?;

    // Create channel log file
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        channels_dir,
        format!("channel_{}.log", channel_id),
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Store the writer and guard for this channel
    let sub_logger = SubLogger {
        name: logger_key.clone(),
        writer: non_blocking,
        _guard: guard,
    };

    SUB_LOGGERS.lock().unwrap().insert(logger_key, sub_logger);

    tracing::info!("Created channel logger for: {}", channel_id);
    Ok(())
}

/// Create a model-specific logger (for modsrv)
pub fn create_model_logger(
    service_dir: &Path,
    model_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let logger_key = format!("model_{}", model_id);

    // Check if already exists
    if SUB_LOGGERS.lock().unwrap().contains_key(&logger_key) {
        return Ok(());
    }

    // Create models directory
    let models_dir = service_dir.join("models");
    fs::create_dir_all(&models_dir)?;

    // Create model log file
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        models_dir,
        format!("model_{}.log", model_id),
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let sub_logger = SubLogger {
        name: logger_key.clone(),
        writer: non_blocking,
        _guard: guard,
    };

    SUB_LOGGERS.lock().unwrap().insert(logger_key, sub_logger);

    tracing::info!("Created model logger for: {}", model_id);
    Ok(())
}

/// Create a generic sub-logger for any service
pub fn create_sub_logger(
    service_dir: &Path,
    sub_type: &str,
    sub_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let logger_key = format!("{}_{}", sub_type, sub_id);

    // Check if already exists
    if SUB_LOGGERS.lock().unwrap().contains_key(&logger_key) {
        return Ok(());
    }

    // Create sub-type directory
    let sub_dir = service_dir.join(format!("{}s", sub_type));
    fs::create_dir_all(&sub_dir)?;

    // Create log file
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        sub_dir,
        format!("{}_{}.log", sub_type, sub_id),
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let sub_logger = SubLogger {
        name: logger_key.clone(),
        writer: non_blocking,
        _guard: guard,
    };

    SUB_LOGGERS.lock().unwrap().insert(logger_key, sub_logger);

    tracing::info!("Created {} logger for: {}", sub_type, sub_id);
    Ok(())
}

/// Write directly to channel log file
pub fn write_to_channel_log(
    channel_id: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Local;
    use std::io::Write;

    // Use /app/logs in Docker, logs in local development
    let log_base = if Path::new("/app/logs").exists() {
        PathBuf::from("/app/logs")
    } else {
        PathBuf::from("logs")
    };
    let log_dir = log_base.join("channels");
    fs::create_dir_all(&log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d");
    let log_file = log_dir.join(format!("channel_{}.log.{}", channel_id, timestamp));

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    let log_entry = format!(
        "[{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        message
    );
    file.write_all(log_entry.as_bytes())?;

    Ok(())
}

/// Write directly to model log file  
pub fn write_to_model_log(model_id: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Local;
    use std::io::Write;

    // Use /app/logs in Docker, logs in local development
    let log_base = if Path::new("/app/logs").exists() {
        PathBuf::from("/app/logs")
    } else {
        PathBuf::from("logs")
    };
    let log_dir = log_base.join("models");
    fs::create_dir_all(&log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d");
    let log_file = log_dir.join(format!("model_{}.log.{}", model_id, timestamp));

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    let log_entry = format!(
        "[{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        message
    );
    file.write_all(log_entry.as_bytes())?;

    Ok(())
}

/// Legacy init function for backward compatibility
pub fn init(level: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = LogConfig {
        console_level: level.parse().unwrap_or(Level::INFO),
        file_level: level.parse().unwrap_or(Level::DEBUG),
        ..Default::default()
    };
    init_with_config(config)
}

/// Set logging level (placeholder for dynamic level changes)
pub fn set_level(level: &str) {
    tracing::info!("Log level set to: {}", level);
    // Note: Dynamic level changes require more complex implementation
}

/// Log to a specific channel (for comsrv)
#[macro_export]
macro_rules! log_to_channel {
    ($channel_id:expr, $level:expr, $($arg:tt)*) => {{
        let channel_id_val = $channel_id;
        let message = format!($($arg)*);

        // Log to main service log with channel_id field
        match $level {
            tracing::Level::ERROR => {
                tracing::error!(target: "comsrv", channel_id = %channel_id_val, "{}", &message);
            },
            tracing::Level::WARN => {
                tracing::warn!(target: "comsrv", channel_id = %channel_id_val, "{}", &message);
            },
            tracing::Level::INFO => {
                tracing::info!(target: "comsrv", channel_id = %channel_id_val, "{}", &message);
            },
            tracing::Level::DEBUG => {
                tracing::debug!(target: "comsrv", channel_id = %channel_id_val, "{}", &message);
            },
            tracing::Level::TRACE => {
                tracing::trace!(target: "comsrv", channel_id = %channel_id_val, "{}", &message);
            },
        }

        // Also write to channel-specific log file
        if let Err(e) = $crate::logging::write_to_channel_log(&channel_id_val.to_string(), &message) {
            tracing::warn!("Failed to write to channel {} log: {}", channel_id_val, e);
        }
    }};
}

/// Log to a specific model (for modsrv)
#[macro_export]
macro_rules! log_to_model {
    ($model_id:expr, $level:expr, $($arg:tt)*) => {{
        let model_id_val = $model_id;
        let message = format!($($arg)*);

        // Log to main service log with model_id field
        match $level {
            tracing::Level::ERROR => {
                tracing::error!(target: "modsrv", model_id = %model_id_val, "{}", &message);
            },
            tracing::Level::WARN => {
                tracing::warn!(target: "modsrv", model_id = %model_id_val, "{}", &message);
            },
            tracing::Level::INFO => {
                tracing::info!(target: "modsrv", model_id = %model_id_val, "{}", &message);
            },
            tracing::Level::DEBUG => {
                tracing::debug!(target: "modsrv", model_id = %model_id_val, "{}", &message);
            },
            tracing::Level::TRACE => {
                tracing::trace!(target: "modsrv", model_id = %model_id_val, "{}", &message);
            },
        }

        // Also write to model-specific log file
        if let Err(e) = $crate::logging::write_to_model_log(&model_id_val.to_string(), &message) {
            tracing::warn!("Failed to write to model {} log: {}", model_id_val, e);
        }
    }};
}
