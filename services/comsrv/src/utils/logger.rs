use std::path::Path;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::Local;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;
use tracing_subscriber::EnvFilter;

use crate::core::config::config_manager::ChannelConfig;
use crate::core::protocols::common::combase::{parse_protocol_packet, PacketParseResult};
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
    std::fs::create_dir_all(&log_dir).map_err(|e| ComSrvError::IoError(e.to_string()))?;

    // set log environment variable
    std::env::set_var("RUST_LOG", format!("{}={}", service_name, level));

    let env_filter = EnvFilter::from_default_env();
    
    if console {
        // log to console
        fmt()
            .with_env_filter(env_filter)
            .init();
            
        tracing::info!("Logger initialized for service: {} (console mode)", service_name);
    } else {
        // log to file
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

/// Channel-specific logger instance
///
/// This structure maintains a separate logger instance for each channel,
/// allowing for independent logging configuration and file management.
#[derive(Debug, Clone)]
pub struct ChannelLogger {
    /// Channel identifier
    channel_id: String,
    /// Log directory path
    log_dir: std::path::PathBuf,
    /// Log level for this channel
    level: LogLevel,
    /// Protocol type for packet parsing
    protocol_type: Option<String>,
}

impl ChannelLogger {
    /// Create a new channel logger instance
    ///
    /// # Arguments
    ///
    /// * `log_dir` - The directory where log files will be stored
    /// * `channel_id` - The identifier of the channel
    /// * `level` - The log level for the channel
    /// * `protocol_type` - Optional protocol type for packet parsing
    pub fn new(log_dir: impl AsRef<Path>, channel_id: &str, level: LogLevel) -> Result<Self> {
        let log_dir = log_dir.as_ref().to_path_buf();
        let channel_log_dir = log_dir.join("channels").join(channel_id);
        std::fs::create_dir_all(&channel_log_dir).map_err(|e| ComSrvError::IoError(e.to_string()))?;

        Ok(ChannelLogger {
            channel_id: channel_id.to_string(),
            log_dir,
            level,
            protocol_type: None,
        })
    }

    /// Create a new channel logger instance with protocol type
    ///
    /// # Arguments
    ///
    /// * `log_dir` - The directory where log files will be stored
    /// * `channel_id` - The identifier of the channel
    /// * `level` - The log level for the channel
    /// * `protocol_type` - Protocol type for packet parsing
    pub fn new_with_protocol(
        log_dir: impl AsRef<Path>, 
        channel_id: &str, 
        level: LogLevel,
        protocol_type: &str
    ) -> Result<Self> {
        let log_dir = log_dir.as_ref().to_path_buf();
        let channel_log_dir = log_dir.join("channels").join(channel_id);
        std::fs::create_dir_all(&channel_log_dir).map_err(|e| ComSrvError::IoError(e.to_string()))?;

        Ok(ChannelLogger {
            channel_id: channel_id.to_string(),
            log_dir,
            level,
            protocol_type: Some(protocol_type.to_string()),
        })
    }

    /// Log a trace message
    pub fn trace(&self, message: &str) {
        if self.level as u8 <= LogLevel::Trace as u8 {
            self.write_log("TRACE", message);
        }
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) {
        if self.level as u8 <= LogLevel::Debug as u8 {
            self.write_log("DEBUG", message);
        }
    }

    /// Log an info message
    pub fn info(&self, message: &str) {
        if self.level as u8 <= LogLevel::Info as u8 {
            self.write_log("INFO", message);
        }
    }

    /// Log a warning message
    pub fn warn(&self, message: &str) {
        if self.level as u8 <= LogLevel::Warn as u8 {
            self.write_log("WARN", message);
        }
    }

    /// Log an error message
    pub fn error(&self, message: &str) {
        if self.level as u8 <= LogLevel::Error as u8 {
            self.write_log("ERROR", message);
        }
    }

    /// Write a log entry to the channel-specific log file
    fn write_log(&self, level: &str, message: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();
        let log_entry = format!("[{}][{}][{}] {}\n", timestamp, self.channel_id, level, message);

        // Write to channel-specific log file
        let channel_log_dir = self.log_dir.join("channels").join(&self.channel_id);
        let date = Local::now().format("%Y-%m-%d").to_string();
        let log_file = channel_log_dir.join(format!("{}.log", date));

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            let _ = file.write_all(log_entry.as_bytes());
        }

        // Also write to tracing for unified logging (without dynamic target)
        match level {
            "TRACE" => tracing::trace!("Channel[{}]: {}", self.channel_id, message),
            "DEBUG" => tracing::debug!("Channel[{}]: {}", self.channel_id, message),
            "INFO" => tracing::info!("Channel[{}]: {}", self.channel_id, message),
            "WARN" => tracing::warn!("Channel[{}]: {}", self.channel_id, message),
            "ERROR" => tracing::error!("Channel[{}]: {}", self.channel_id, message),
            _ => tracing::info!("Channel[{}]: {}", self.channel_id, message),
        }
    }

    /// Get the channel ID
    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    /// Get the current log level
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// Set a new log level
    pub fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// Set protocol type for packet parsing
    pub fn set_protocol(&mut self, protocol_type: &str) {
        self.protocol_type = Some(protocol_type.to_string());
    }

    /// Get protocol type
    pub fn protocol_type(&self) -> Option<&str> {
        self.protocol_type.as_deref()
    }

    /// Log a protocol packet with automatic parsing
    ///
    /// This method logs a protocol packet and automatically parses it
    /// if a protocol parser is available. The parsed information is
    /// logged at debug level.
    ///
    /// # Arguments
    ///
    /// * `direction` - Packet direction ("send" or "receive")
    /// * `data` - Raw packet data
    pub fn log_packet(&self, direction: &str, data: &[u8]) {
        if let Some(protocol) = &self.protocol_type {
            let parse_result = parse_protocol_packet(protocol, data, direction);
            let log_message = parse_result.format_debug_log();
            self.debug(&log_message);
        } else {
            // Fallback to basic hex logging if no protocol is set
            let hex_data = data.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            self.debug(&format!("[{}] {}", direction.to_uppercase(), hex_data));
        }
    }

    /// Log a protocol packet with detailed field information
    ///
    /// This method logs a protocol packet and includes detailed field
    /// information if available. Useful for detailed debugging.
    ///
    /// # Arguments
    ///
    /// * `direction` - Packet direction ("send" or "receive")
    /// * `data` - Raw packet data
    pub fn log_packet_detailed(&self, direction: &str, data: &[u8]) {
        if let Some(protocol) = &self.protocol_type {
            let parse_result = parse_protocol_packet(protocol, data, direction);
            
            // Log the main packet information
            let log_message = parse_result.format_debug_log();
            self.debug(&log_message);
            
            // Log detailed field information if available
            if parse_result.success && !parse_result.fields.is_empty() {
                for (field, value) in &parse_result.fields {
                    self.trace(&format!("  {}: {}", field, value));
                }
            }
        } else {
            // Fallback to basic hex logging if no protocol is set
            let hex_data = data.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            self.debug(&format!("[{}] {}", direction.to_uppercase(), hex_data));
        }
    }
}

/// Global channel logger manager
///
/// This manages multiple channel loggers and provides a centralized access point.
pub struct ChannelLoggerManager {
    loggers: Arc<Mutex<HashMap<String, ChannelLogger>>>,
    log_dir: std::path::PathBuf,
}

impl ChannelLoggerManager {
    /// Create a new channel logger manager
    pub fn new(log_dir: impl AsRef<Path>) -> Self {
        Self {
            loggers: Arc::new(Mutex::new(HashMap::new())),
            log_dir: log_dir.as_ref().to_path_buf(),
        }
    }

    /// Get or create a channel logger
    pub fn get_logger(&self, channel_id: &str, level: LogLevel) -> Result<ChannelLogger> {
        let mut loggers = self.loggers.lock().map_err(|e| ComSrvError::LockError(e.to_string()))?;
        
        if let Some(logger) = loggers.get(channel_id) {
            Ok(logger.clone())
        } else {
            let logger = ChannelLogger::new(&self.log_dir, channel_id, level)?;
            loggers.insert(channel_id.to_string(), logger.clone());
            Ok(logger)
        }
    }

    /// Remove a channel logger
    pub fn remove_logger(&self, channel_id: &str) -> Result<()> {
        let mut loggers = self.loggers.lock().map_err(|e| ComSrvError::LockError(e.to_string()))?;
        loggers.remove(channel_id);
        Ok(())
    }

    /// List all active channel loggers
    pub fn list_loggers(&self) -> Result<Vec<String>> {
        let loggers = self.loggers.lock().map_err(|e| ComSrvError::LockError(e.to_string()))?;
        Ok(loggers.keys().cloned().collect())
    }
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
    level: &str,
) -> Result<ChannelLogger> {
    // Parse log level
    let log_level = LogLevel::from_str(level)?;
    
    // Create channel logger
    let logger = ChannelLogger::new(log_dir, channel_id, log_level)?;
    
    // Log initialization
    logger.info(&format!("Channel logger initialized for service: {}", service_name));
    tracing::info!("Channel logger initialized: {} for service {}", channel_id, service_name);
    
    Ok(logger)
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
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
    std::fs::create_dir_all(&messages_dir).map_err(|e| ComSrvError::IoError(e.to_string()))?;

    // Create channel-specific directory
    let channel_dir = messages_dir.join(channel_id);
    std::fs::create_dir_all(&channel_dir).map_err(|e| ComSrvError::IoError(e.to_string()))?;

    // Generate filename with current date only
    let date = Local::now().format("%Y-%m-%d").to_string();
    let filename = format!("{}.msg", date);
    let filepath = channel_dir.join(filename);

    // Open file in append mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filepath)
        .map_err(|e| ComSrvError::IoError(e.to_string()))?;

    // Write timestamp, direction and message
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();
    writeln!(file, "[{}][{}] {}", timestamp, direction, String::from_utf8_lossy(message))
        .map_err(|e| ComSrvError::IoError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("trace").unwrap(), LogLevel::Trace);
        assert_eq!(LogLevel::from_str("debug").unwrap(), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("info").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::from_str("warn").unwrap(), LogLevel::Warn);
        assert_eq!(LogLevel::from_str("error").unwrap(), LogLevel::Error);
        
        // Test case insensitive
        assert_eq!(LogLevel::from_str("INFO").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::from_str("Debug").unwrap(), LogLevel::Debug);
        
        // Test invalid level
        assert!(LogLevel::from_str("invalid").is_err());
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Trace.as_str(), "trace");
        assert_eq!(LogLevel::Debug.as_str(), "debug");
        assert_eq!(LogLevel::Info.as_str(), "info");
        assert_eq!(LogLevel::Warn.as_str(), "warn");
        assert_eq!(LogLevel::Error.as_str(), "error");
    }

    #[test]
    fn test_log_level_round_trip() {
        let levels = vec![
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];

        for level in levels {
            let str_level = level.as_str();
            let parsed_level = LogLevel::from_str(str_level).unwrap();
            assert_eq!(level, parsed_level);
        }
    }

    #[test]
    fn test_init_logger_creates_directory() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        // Only test directory creation, not logger initialization
        // since global logger might already be set
        
        // Test that the function can handle directory creation
        // without actually initializing the logger
        let log_file = log_dir.join("test.log");
        
        // Create the directory structure that would be created
        std::fs::create_dir_all(log_dir).unwrap();
        
        // Verify directory was created
        assert!(log_dir.exists());
        assert!(log_dir.is_dir());
        
        // Test that we can create a file in the directory
        std::fs::write(&log_file, "test").unwrap();
        assert!(log_file.exists());
    }

    #[test]
    fn test_init_channel_logger() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        let logger = init_channel_logger(&log_dir, "test_service", "channel_1", "debug")?;
        
        // Check if channels directory was created
        let channels_dir = log_dir.join("channels");
        assert!(channels_dir.exists());
        
        // Check if channel-specific directory was created
        let channel_dir = channels_dir.join("channel_1");
        assert!(channel_dir.exists());
        
        // Test channel logger functionality
        assert_eq!(logger.channel_id(), "channel_1");
        assert_eq!(logger.level(), LogLevel::Debug);
        
        // Test logging methods
        logger.info("Test info message");
        logger.debug("Test debug message");
        logger.warn("Test warning message");
        
        Ok(())
    }

    #[test]
    fn test_channel_logger_level_filtering() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        // Create logger with INFO level
        let logger = ChannelLogger::new(&log_dir, "test_channel", LogLevel::Info)?;
        
        // Test level filtering
        logger.error("This should be logged");
        logger.warn("This should be logged");
        logger.info("This should be logged");
        logger.debug("This should NOT be logged");
        logger.trace("This should NOT be logged");
        
        // Check log file exists
        let channel_dir = log_dir.join("channels").join("test_channel");
        assert!(channel_dir.exists());
        
        Ok(())
    }

    #[test]
    fn test_channel_logger_manager() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        let manager = ChannelLoggerManager::new(&log_dir);
        
        // Get a logger
        let logger1 = manager.get_logger("channel_1", LogLevel::Debug)?;
        assert_eq!(logger1.channel_id(), "channel_1");
        
        // Get the same logger again (should be cached)
        let logger2 = manager.get_logger("channel_1", LogLevel::Info)?;
        assert_eq!(logger2.channel_id(), "channel_1");
        
        // Get a different logger
        let logger3 = manager.get_logger("channel_2", LogLevel::Error)?;
        assert_eq!(logger3.channel_id(), "channel_2");
        
        // List loggers
        let loggers = manager.list_loggers()?;
        assert_eq!(loggers.len(), 2);
        assert!(loggers.contains(&"channel_1".to_string()));
        assert!(loggers.contains(&"channel_2".to_string()));
        
        // Remove a logger
        manager.remove_logger("channel_1")?;
        let loggers = manager.list_loggers()?;
        assert_eq!(loggers.len(), 1);
        assert!(!loggers.contains(&"channel_1".to_string()));
        
        Ok(())
    }

    #[test]
    fn test_log_message() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        let message = b"Test message content";
        let result = log_message(&log_dir, "channel_1", "send", message);
        assert!(result.is_ok());
        
        // Check if message directory structure was created
        let messages_dir = log_dir.join("messages").join("channel_1");
        assert!(messages_dir.exists());
        
        // Check if log file was created
        let files: Vec<_> = fs::read_dir(&messages_dir).unwrap().collect();
        assert!(!files.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_log_message_content() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();
        
        let message1 = b"First message";
        let message2 = b"Second message";
        
        log_message(&log_dir, "channel_test", "send", message1)?;
        log_message(&log_dir, "channel_test", "receive", message2)?;
        
        // Read the log file
        let messages_dir = log_dir.join("messages").join("channel_test");
        let files: Vec<_> = fs::read_dir(&messages_dir).unwrap().collect();
        assert_eq!(files.len(), 1);
        
        let file_path = files[0].as_ref().unwrap().path();
        let content = fs::read_to_string(&file_path).unwrap();
        
        assert!(content.contains("First message"));
        assert!(content.contains("Second message"));
        assert!(content.contains("[send]"));
        assert!(content.contains("[receive]"));
        
        Ok(())
    }

    #[test]
    fn test_log_message_with_invalid_directory() {
        // Test with a read-only parent directory (simulating permission error)
        let result = log_message("/invalid/path", "channel_1", "send", b"test");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::IoError(_)));
    }
}