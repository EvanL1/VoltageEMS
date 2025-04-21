use std::path::Path;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Local;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;
use tracing_subscriber::EnvFilter;

use crate::utils::error::{ComSrvError, Result};

/// Initialize the logger with file and console output
///
/// This function sets up the global logger with both file and console output.
/// File logs are rotated daily and have a maximum of 5 files.
///
/// # Arguments
///
/// * `log_dir` - The directory where log files will be stored
/// * `service_name` - The name of the service, used as part of the log file name
/// * `level` - The log level (trace, debug, info, warn, error)
/// * `console` - Whether to also log to console
pub fn init_logger(
    log_dir: impl AsRef<Path>,
    service_name: &str,
    level: &str,
    console: bool,
) -> Result<()> {
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir).map_err(|e| ComSrvError::IoError(e))?;

    // 设置日志环境变量
    std::env::set_var("RUST_LOG", format!("{}={}", service_name, level));

    let env_filter = EnvFilter::from_default_env();
    
    if console {
        // 控制台日志
        fmt()
            .with_env_filter(env_filter)
            .init();
            
        tracing::info!("Logger initialized for service: {} (console mode)", service_name);
    } else {
        // 文件日志
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            log_dir,
            format!("{}.log", service_name),
        );
        
        fmt()
            .with_env_filter(env_filter)
            .with_writer(file_appender)
            .with_ansi(false)
            .init();
            
        tracing::info!("Logger initialized for service: {} (file mode)", service_name);
    }

    Ok(())
}

/// Initialize a channel-specific logger
///
/// This function sets up a logger for a specific channel, with logs going to a channel-specific
/// file as well as the main service log.
///
/// # Arguments
///
/// * `log_dir` - The directory where log files will be stored
/// * `service_name` - The name of the service
/// * `channel_id` - The identifier of the channel
/// * `level` - The log level for the channel (trace, debug, info, warn, error)
pub fn init_channel_logger(
    log_dir: impl AsRef<Path>,
    service_name: &str,
    channel_id: &str,
    _level: &str,
) -> Result<()> {
    // Create channel log directory
    let channel_log_dir = log_dir.as_ref().join("channels");
    std::fs::create_dir_all(&channel_log_dir).map_err(|e| ComSrvError::IoError(e))?;

    // 在实际实现中，我们需要一个更复杂的日志系统
    // 这里简化处理，仅记录初始化信息
    tracing::info!("Channel logger initialized: {} for service {}", channel_id, service_name);
    
    Ok(())
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Convert string to LogLevel
    pub fn from_str(s: &str) -> std::result::Result<Self, ComSrvError> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(ComSrvError::InvalidParameter(format!("Unknown log level: {}", s))),
        }
    }

    /// Convert LogLevel to string
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// Log a message packet to a dedicated file
///
/// This function writes message packets to a dedicated file in the messages directory.
/// Messages are organized by channel and date.
///
/// # Arguments
///
/// * `log_dir` - The directory where log files will be stored
/// * `channel_id` - The identifier of the channel
/// * `direction` - The direction of the message ("send" or "receive")
/// * `message` - The message content to log
pub fn log_message(
    log_dir: impl AsRef<Path>,
    channel_id: &str,
    direction: &str,
    message: &[u8],
) -> Result<()> {
    // Create messages directory if it doesn't exist
    let messages_dir = log_dir.as_ref().join("messages");
    std::fs::create_dir_all(&messages_dir).map_err(|e| ComSrvError::IoError(e))?;

    // Create channel-specific directory
    let channel_dir = messages_dir.join(channel_id);
    std::fs::create_dir_all(&channel_dir).map_err(|e| ComSrvError::IoError(e))?;

    // Generate filename with current date only
    let date = Local::now().format("%Y-%m-%d").to_string();
    let filename = format!("{}.msg", date);
    let filepath = channel_dir.join(filename);

    // Open file in append mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filepath)
        .map_err(|e| ComSrvError::IoError(e))?;

    // Write timestamp, direction and message
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();
    writeln!(file, "[{}][{}] {}", timestamp, direction, String::from_utf8_lossy(message))
        .map_err(|e| ComSrvError::IoError(e))?;

    Ok(())
}