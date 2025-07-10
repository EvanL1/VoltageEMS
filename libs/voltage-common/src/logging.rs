//! Common logging configuration for VoltageEMS services

use crate::Result;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Enable console output
    pub console: bool,
    /// Enable file output
    pub file: Option<String>,
    /// Log format (json, pretty, compact)
    pub format: LogFormat,
    /// Enable ANSI colors in console output
    pub ansi: bool,
    /// Include span events
    pub span_events: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            console: true,
            file: None,
            format: LogFormat::Pretty,
            ansi: true,
            span_events: false,
        }
    }
}

/// Initialize logging with the given configuration
///
/// Returns a guard that must be kept alive for file logging to work
pub fn init_logging(config: &LogConfig) -> Result<Option<WorkerGuard>> {
    let mut layers = Vec::new();
    let mut guard = None;

    // Console layer
    if config.console {
        let env_filter = EnvFilter::try_new(&config.level)
            .or_else(|_| EnvFilter::try_new("info"))
            .map_err(|e| crate::Error::config(format!("Invalid log level: {}", e)))?;
        let console_layer = match config.format {
            LogFormat::Json => {
                let layer = fmt::layer()
                    .json()
                    .with_ansi(config.ansi)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
            LogFormat::Pretty => {
                let layer = fmt::layer()
                    .pretty()
                    .with_ansi(config.ansi)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
            LogFormat::Compact => {
                let layer = fmt::layer()
                    .compact()
                    .with_ansi(config.ansi)
                    .with_target(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
        };

        layers.push(console_layer.with_filter(env_filter).boxed());
    }

    // File layer
    if let Some(file_path) = &config.file {
        let env_filter = EnvFilter::try_new(&config.level)
            .or_else(|_| EnvFilter::try_new("info"))
            .map_err(|e| crate::Error::config(format!("Invalid log level: {}", e)))?;

        let path = Path::new(file_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::Error::Io)?;
        }

        let file_appender = tracing_appender::rolling::daily(
            path.parent().unwrap_or_else(|| Path::new(".")),
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("app.log"),
        );
        let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);
        guard = Some(file_guard);

        let file_layer = match config.format {
            LogFormat::Json => {
                let layer = fmt::layer()
                    .json()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
            LogFormat::Pretty => {
                let layer = fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
            LogFormat::Compact => {
                let layer = fmt::layer()
                    .compact()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_target(true);
                if config.span_events {
                    layer.with_span_events(FmtSpan::FULL).boxed()
                } else {
                    layer.boxed()
                }
            }
        };

        layers.push(file_layer.with_filter(env_filter).boxed());
    }

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(layers)
        .try_init()
        .map_err(|e| crate::Error::config(format!("Failed to initialize logging: {}", e)))?;

    Ok(guard)
}

/// Initialize logging with default configuration
pub fn init_default_logging() -> Result<Option<WorkerGuard>> {
    init_logging(&LogConfig::default())
}

/// Initialize logging for tests
pub fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}

/// A builder for constructing log configuration
pub struct LogConfigBuilder {
    config: LogConfig,
}

impl Default for LogConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LogConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: LogConfig::default(),
        }
    }

    pub fn level(mut self, level: impl Into<String>) -> Self {
        self.config.level = level.into();
        self
    }

    pub fn console(mut self, enable: bool) -> Self {
        self.config.console = enable;
        self
    }

    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.config.file = Some(path.into());
        self
    }

    pub fn format(mut self, format: LogFormat) -> Self {
        self.config.format = format;
        self
    }

    pub fn ansi(mut self, enable: bool) -> Self {
        self.config.ansi = enable;
        self
    }

    pub fn span_events(mut self, enable: bool) -> Self {
        self.config.span_events = enable;
        self
    }

    pub fn build(self) -> LogConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_builder() {
        let config = LogConfigBuilder::new()
            .level("debug")
            .console(true)
            .file("/tmp/test.log")
            .format(LogFormat::Json)
            .ansi(false)
            .build();

        assert_eq!(config.level, "debug");
        assert!(config.console);
        assert_eq!(config.file, Some("/tmp/test.log".to_string()));
        assert!(matches!(config.format, LogFormat::Json));
        assert!(!config.ansi);
    }
}
