use crate::config::LoggingConfig;
use std::fs;
use std::path::Path;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub fn init_logging(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create logs directory if it doesn't exist
    if let Some(parent) = Path::new(&config.file).parent() {
        fs::create_dir_all(parent)?;
    }

    // Set up the environment filter
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Create the subscriber based on format preference
    match config.format.as_str() {
        "json" => {
            // JSON format for structured logging
            Registry::default()
                .with(env_filter)
                .with(
                    fmt::layer()
                        // .json() // TODO: Enable with json feature
                        // .with_current_span(true) // TODO: Enable with proper feature
                        // .with_span_list(false) // TODO: Enable with proper feature
                        .with_writer(std::io::stdout),
                )
                .init();
        }
        "text" | _ => {
            // Text format for human-readable logs
            Registry::default()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(true)
                        .with_line_number(true)
                        .with_writer(std::io::stdout),
                )
                .init();
        }
    }

    tracing::info!(
        "Logging initialized with level: {}, format: {}",
        config.level,
        config.format
    );

    Ok(())
}

// Structured logging helpers
pub fn log_message_processed(
    channel: &str,
    message_id: &str,
    processing_time_ms: u64,
    success: bool,
) {
    if success {
        tracing::info!(
            channel = channel,
            message_id = message_id,
            processing_time_ms = processing_time_ms,
            "Message processed successfully"
        );
    } else {
        tracing::error!(
            channel = channel,
            message_id = message_id,
            processing_time_ms = processing_time_ms,
            "Message processing failed"
        );
    }
}

pub fn log_storage_operation(
    backend: &str,
    operation: &str,
    key: &str,
    duration_ms: u64,
    success: bool,
) {
    if success {
        tracing::debug!(
            backend = backend,
            operation = operation,
            key = key,
            duration_ms = duration_ms,
            "Storage operation completed"
        );
    } else {
        tracing::error!(
            backend = backend,
            operation = operation,
            key = key,
            duration_ms = duration_ms,
            "Storage operation failed"
        );
    }
}

pub fn log_api_request(
    method: &str,
    path: &str,
    status_code: u16,
    duration_ms: u64,
    user_agent: Option<&str>,
) {
    tracing::info!(
        method = method,
        path = path,
        status_code = status_code,
        duration_ms = duration_ms,
        user_agent = user_agent.unwrap_or("unknown"),
        "API request completed"
    );
}

// Error logging with context
pub fn log_error_with_context(
    error: &dyn std::error::Error,
    context: &str,
    _additional_fields: Option<&[(&str, &str)]>,
) {
    tracing::error!(
        error = %error,
        context = context,
    );

    // TODO: Handle additional fields when tracing supports dynamic fields
}

// Performance tracking
pub struct PerformanceTracker {
    start_time: std::time::Instant,
    operation: String,
}

impl PerformanceTracker {
    pub fn new(operation: String) -> Self {
        tracing::debug!(operation = %operation, "Starting operation");
        Self {
            start_time: std::time::Instant::now(),
            operation,
        }
    }

    pub fn complete(self) -> u64 {
        let duration = self.start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;

        tracing::debug!(
            operation = %self.operation,
            duration_ms = duration_ms,
            "Operation completed"
        );

        duration_ms
    }

    pub fn complete_with_result<T, E>(self, result: &Result<T, E>) -> u64
    where
        E: std::fmt::Display,
    {
        let duration = self.start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;

        match result {
            Ok(_) => {
                tracing::debug!(
                    operation = %self.operation,
                    duration_ms = duration_ms,
                    "Operation completed successfully"
                );
            }
            Err(e) => {
                tracing::error!(
                    operation = %self.operation,
                    duration_ms = duration_ms,
                    error = %e,
                    "Operation failed"
                );
            }
        }

        duration_ms
    }
}
