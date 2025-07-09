//! Logging configuration types

use serde::{Deserialize, Serialize};

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log file path
    pub file: Option<String>,

    /// Console logging
    #[serde(default = "default_true")]
    pub console: bool,

    /// Max log file size in bytes
    #[serde(default = "default_log_max_size")]
    pub max_size: u64,

    /// Max number of log files
    #[serde(default = "default_log_max_files")]
    pub max_files: u32,

    /// Log retention days (how many days to keep log files)
    #[serde(default = "default_log_retention_days")]
    pub retention_days: u32,

    /// Enable channel-specific logging
    #[serde(default = "default_true")]
    pub enable_channel_logging: bool,

    /// Channel log directory (relative to service log root)
    #[serde(default = "default_channel_log_dir")]
    pub channel_log_dir: String,
}

/// Channel-specific logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLoggingConfig {
    /// Whether logging is enabled for this channel
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Log level for this channel
    pub level: Option<String>,

    /// Channel log directory (relative to service log root)
    pub log_dir: Option<String>,

    /// Max log file size in bytes
    pub max_file_size: Option<u64>,

    /// Max number of log files
    pub max_files: Option<u32>,

    /// Log retention days (how many days to keep log files)
    pub retention_days: Option<u32>,

    /// Whether to output to console
    pub console_output: Option<bool>,

    /// Whether to log protocol messages
    #[serde(default = "default_true")]
    pub log_messages: bool,
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_max_size() -> u64 {
    104_857_600 // 100MB
}

fn default_log_max_files() -> u32 {
    5
}

fn default_log_retention_days() -> u32 {
    30 // Keep logs for 30 days by default
}

fn default_channel_log_dir() -> String {
    "channels".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            console: default_true(),
            max_size: default_log_max_size(),
            max_files: default_log_max_files(),
            retention_days: default_log_retention_days(),
            enable_channel_logging: default_true(),
            channel_log_dir: default_channel_log_dir(),
        }
    }
}

impl Default for ChannelLoggingConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            level: None,
            log_dir: None,
            max_file_size: None,
            max_files: None,
            retention_days: None,
            console_output: None,
            log_messages: default_true(),
        }
    }
}
