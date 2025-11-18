//! Unified logging module for VoltageEMS services
//!
//! Provides multi-level logging support with automatic sub-logger creation

use std::fs::{self, File, OpenOptions};
#[allow(unused_imports)] // Used in Write trait impl for DailyRollingWriter
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use flate2::write::GzEncoder;
use flate2::Compression;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::FilterExt,
    fmt::{self, format::FmtSpan, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

// Global guards for keeping loggers alive
static GUARDS: OnceLock<Arc<Mutex<Vec<WorkerGuard>>>> = OnceLock::new();
// API logger guard - separate to allow independent lifecycle
static API_GUARD: OnceLock<Arc<Mutex<Option<WorkerGuard>>>> = OnceLock::new();

// Custom daily rolling file writer with naming format: {service}{YYYYMMDD}.log
struct DailyRollingWriter {
    service_name: String,
    log_dir: PathBuf,
    current_date: Arc<Mutex<String>>,
    current_file: Arc<Mutex<Option<File>>>,
}

impl DailyRollingWriter {
    fn new(service_name: String, log_dir: PathBuf) -> std::io::Result<Self> {
        let current_date = chrono::Local::now().format("%Y%m%d").to_string();
        let file_path = log_dir.join(format!("{}{}.log", service_name, current_date));

        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir)?;

        // Open or create the log file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        Ok(Self {
            service_name,
            log_dir,
            current_date: Arc::new(Mutex::new(current_date)),
            current_file: Arc::new(Mutex::new(Some(file))),
        })
    }

    fn get_writer(&self) -> std::io::Result<std::sync::MutexGuard<'_, Option<File>>> {
        // Check if date has changed
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let mut current_date = self
            .current_date
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;

        if *current_date != today {
            // Date changed, rotate to new file
            let new_file_path = self
                .log_dir
                .join(format!("{}{}.log", self.service_name, today));
            let new_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_file_path)?;

            // Update current date and file
            *current_date = today;
            let mut current_file = self
                .current_file
                .lock()
                .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;
            *current_file = Some(new_file);
        }

        self.current_file
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))
    }
}

impl std::io::Write for DailyRollingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(ref mut file) = *self.get_writer()? {
            file.write(buf)
        } else {
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut file) = *self.get_writer()? {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl Clone for DailyRollingWriter {
    fn clone(&self) -> Self {
        Self {
            service_name: self.service_name.clone(),
            log_dir: self.log_dir.clone(),
            current_date: Arc::clone(&self.current_date),
            current_file: Arc::clone(&self.current_file),
        }
    }
}

// Custom daily rolling file writer for API logs with naming format: {service}_api{YYYYMMDD}.log
struct ApiDailyRollingWriter {
    service_name: String,
    log_dir: PathBuf,
    current_date: Arc<Mutex<String>>,
    current_file: Arc<Mutex<Option<File>>>,
}

impl ApiDailyRollingWriter {
    fn new(service_name: String, log_dir: PathBuf) -> std::io::Result<Self> {
        let current_date = chrono::Local::now().format("%Y%m%d").to_string();
        let file_path = log_dir.join(format!("{}_api{}.log", service_name, current_date));

        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir)?;

        // Open or create the log file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;

        Ok(Self {
            service_name,
            log_dir,
            current_date: Arc::new(Mutex::new(current_date)),
            current_file: Arc::new(Mutex::new(Some(file))),
        })
    }

    fn get_writer(&self) -> std::io::Result<std::sync::MutexGuard<'_, Option<File>>> {
        // Check if date has changed
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let mut current_date = self
            .current_date
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;

        if *current_date != today {
            // Date changed, rotate to new file
            let new_file_path = self
                .log_dir
                .join(format!("{}_api{}.log", self.service_name, today));
            let new_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_file_path)?;

            // Update current date and file
            *current_date = today;
            let mut current_file = self
                .current_file
                .lock()
                .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;
            *current_file = Some(new_file);
        }

        self.current_file
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))
    }
}

impl std::io::Write for ApiDailyRollingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(ref mut file) = *self.get_writer()? {
            file.write(buf)
        } else {
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut file) = *self.get_writer()? {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl Clone for ApiDailyRollingWriter {
    fn clone(&self) -> Self {
        Self {
            service_name: self.service_name.clone(),
            log_dir: self.log_dir.clone(),
            current_date: Arc::clone(&self.current_date),
            current_file: Arc::clone(&self.current_file),
        }
    }
}

// Reloadable writer for file logging
struct ReloadableWriter {
    inner: Arc<Mutex<Option<tracing_appender::non_blocking::NonBlocking>>>,
}

impl ReloadableWriter {
    fn new(writer: tracing_appender::non_blocking::NonBlocking) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(writer))),
        }
    }

    fn reload(&self, new_writer: tracing_appender::non_blocking::NonBlocking) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(new_writer);
        }
    }
}

impl std::io::Write for ReloadableWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut writer) = *guard {
                return writer.write(buf);
            }
        }
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut writer) = *guard {
                return writer.flush();
            }
        }
        Ok(())
    }
}

impl MakeWriter<'_> for ReloadableWriter {
    type Writer = Self;

    fn make_writer(&'_ self) -> Self::Writer {
        ReloadableWriter {
            inner: self.inner.clone(),
        }
    }
}

// Newtype wrapper for Arc<ReloadableWriter> to bypass orphan rule
#[derive(Clone)]
struct ReloadableWriterHandle(Arc<ReloadableWriter>);

impl MakeWriter<'_> for ReloadableWriterHandle {
    type Writer = ReloadableWriter;

    fn make_writer(&'_ self) -> Self::Writer {
        ReloadableWriter {
            inner: self.0.inner.clone(),
        }
    }
}

// Keep runtime config and writer for reopen
#[derive(Clone)]
struct LogRuntime {
    service_name: String,
    log_dir: PathBuf,
    #[allow(dead_code)] // Kept for potential future use
    file_level: Level,
    #[allow(dead_code)] // Kept for potential future use
    enable_json: bool,
}

static LOG_RUNTIME: OnceLock<Arc<Mutex<LogRuntime>>> = OnceLock::new();
static RELOADABLE_WRITER: OnceLock<Arc<ReloadableWriter>> = OnceLock::new();
// API logger runtime configuration and writer
static API_LOG_RUNTIME: OnceLock<Arc<Mutex<LogRuntime>>> = OnceLock::new();
static API_RELOADABLE_WRITER: OnceLock<Arc<ReloadableWriter>> = OnceLock::new();

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
    /// Maximum number of log files to keep (for compression/cleanup)
    pub max_log_files: usize,
    /// Enable API log separation (default: true)
    pub enable_api_log: bool,
    /// API log level (default: INFO)
    pub api_log_level: Level,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            service_name: "unknown".to_string(),
            log_dir: PathBuf::from("logs"),
            console_level: Level::INFO,
            file_level: Level::DEBUG,
            enable_json: false,
            max_log_files: 30,
            enable_api_log: true,
            api_log_level: Level::INFO,
        }
    }
}

/// Initialize logging system with configuration
pub fn init_with_config(config: LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create log directory if it doesn't exist
    fs::create_dir_all(&config.log_dir)?;

    // Create custom daily rolling file writer with format: {service}{YYYYMMDD}.log
    let custom_writer =
        DailyRollingWriter::new(config.service_name.clone(), config.log_dir.clone())?;

    let (non_blocking, guard) = tracing_appender::non_blocking(custom_writer);

    // Store guard to prevent dropping
    let guards = GUARDS.get_or_init(|| Arc::new(Mutex::new(Vec::new())));
    match guards.lock() {
        Ok(mut guards) => guards.push(guard),
        Err(poisoned) => {
            // Lock was poisoned, but we can recover by using the data anyway
            eprintln!("Warning: GUARDS lock was poisoned, recovering...");
            poisoned.into_inner().push(guard);
        },
    }

    // Build subscriber with layers
    // IMPORTANT: Respect RUST_LOG environment variable, only add api_access if not specified
    let api_level = if config.enable_api_log {
        config.api_log_level.as_str()
    } else {
        "off"
    };

    let env_filter = if let Ok(env_str) = std::env::var("RUST_LOG") {
        // RUST_LOG is set - only append api_access if not already specified
        if env_str.contains("api_access") {
            // User explicitly set api_access level in RUST_LOG, respect it
            EnvFilter::new(env_str)
        } else {
            // api_access not in RUST_LOG, use config default
            // IMPORTANT: If RUST_LOG contains "debug" or "trace", upgrade api_access to debug for full visibility
            let effective_api_level = if env_str.contains("debug") || env_str.contains("trace") {
                "debug"
            } else {
                api_level
            };
            EnvFilter::new(format!("{},api_access={}", env_str, effective_api_level))
        }
    } else {
        // RUST_LOG not set - use default with api_access
        EnvFilter::new(format!(
            "info,{}=debug,api_access={}",
            config.service_name, api_level
        ))
    };

    let registry = tracing_subscriber::registry().with(env_filter);

    // Console layer - simplified for INFO and above, detailed for DEBUG and below
    let console_layer = if config.console_level >= Level::INFO {
        // Production mode: clean output for operations staff
        fmt::layer()
            .with_ansi(true)
            .with_level(true)
            .with_target(false)  // No module paths for INFO
            .with_thread_ids(false)  // No ThreadId for INFO
            .with_span_events(FmtSpan::NONE)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                config.console_level,
            ))
    } else {
        // Debug mode: full details for developers
        fmt::layer()
            .with_ansi(true)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::NONE)
            .with_filter(tracing_subscriber::filter::LevelFilter::from_level(
                config.console_level,
            ))
    };

    // Create reloadable writer
    let reloadable_writer = ReloadableWriter::new(non_blocking);
    let reloadable_writer_arc = Arc::new(reloadable_writer);

    // Store the reloadable writer globally
    RELOADABLE_WRITER.get_or_init(|| reloadable_writer_arc.clone());

    // Wrap in newtype for MakeWriter implementation
    let writer_handle = ReloadableWriterHandle(reloadable_writer_arc.clone());

    // Business file layer (exclude api_access target)
    use tracing_subscriber::filter;

    let business_file_layer = if config.enable_json {
        fmt::layer()
            .json()
            .with_writer(writer_handle.clone())
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::FULL)
            .with_filter(
                filter::filter_fn(|metadata| metadata.target() != "api_access").and(
                    tracing_subscriber::filter::LevelFilter::from_level(config.file_level),
                ),
            )
            .boxed()
    } else {
        fmt::layer()
            .with_writer(writer_handle)
            .with_ansi(false)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::NONE)
            .with_filter(
                filter::filter_fn(|metadata| metadata.target() != "api_access").and(
                    tracing_subscriber::filter::LevelFilter::from_level(config.file_level),
                ),
            )
            .boxed()
    };

    // API file layer (only api_access target) - created if enable_api_log is true
    let api_file_layer = if config.enable_api_log {
        // Create API daily rolling file writer
        let api_writer =
            ApiDailyRollingWriter::new(config.service_name.clone(), config.log_dir.clone())?;
        let (api_non_blocking, api_guard) = tracing_appender::non_blocking(api_writer);

        // Store API guard to prevent dropping
        let guard_storage = API_GUARD.get_or_init(|| Arc::new(Mutex::new(None)));
        match guard_storage.lock() {
            Ok(mut slot) => *slot = Some(api_guard),
            Err(poisoned) => {
                eprintln!("Warning: API_GUARD lock was poisoned, recovering...");
                *poisoned.into_inner() = Some(api_guard);
            },
        }

        // Create reloadable writer for API logs
        let api_reloadable_writer = ReloadableWriter::new(api_non_blocking);
        let api_reloadable_writer_arc = Arc::new(api_reloadable_writer);
        API_RELOADABLE_WRITER.get_or_init(|| api_reloadable_writer_arc.clone());

        let api_writer_handle = ReloadableWriterHandle(api_reloadable_writer_arc);

        Some(
            fmt::layer()
                .with_writer(api_writer_handle)
                .with_ansi(false)
                .with_level(true)
                .with_target(false) // Don't show target (we know it's api_access)
                .with_thread_ids(false)
                .with_span_events(FmtSpan::NONE)
                .with_filter(filter::filter_fn(|metadata| metadata.target() == "api_access"))
                .boxed(),
        )
    } else {
        None
    };

    // Register all layers
    let registry_with_layers = registry.with(console_layer).with(business_file_layer);

    if let Some(api_layer) = api_file_layer {
        registry_with_layers.with(api_layer).init();
    } else {
        registry_with_layers.init();
    }

    let runtime = LogRuntime {
        service_name: config.service_name.clone(),
        log_dir: config.log_dir.clone(),
        file_level: config.file_level,
        enable_json: config.enable_json,
    };
    let rt_store = LOG_RUNTIME.get_or_init(|| Arc::new(Mutex::new(runtime.clone())));
    if let Ok(mut slot) = rt_store.lock() {
        *slot = runtime;
    }

    tracing::info!(
        "Logging initialized for service: {} at {:?}",
        config.service_name,
        config.log_dir
    );

    if config.enable_api_log {
        tracing::info!(
            "API logging enabled: {}_api{{YYYYMMDD}}.log",
            config.service_name
        );
    }

    // Start background compression task after logging the initialization
    // Move the values since config is no longer needed
    start_log_compression_task(config.log_dir, config.service_name);

    Ok(())
}

/// Reopen log file writer (e.g., after manual deletion/rotation)
pub fn reopen_logs_now() -> Result<(), Box<dyn std::error::Error>> {
    let runtime_arc = LOG_RUNTIME
        .get()
        .ok_or("logging not initialized (runtime config missing)")?
        .clone();
    let runtime = runtime_arc.lock().map_err(|_| "poisoned lock")?.clone();

    // Ensure directory exists
    fs::create_dir_all(&runtime.log_dir)?;

    // Create new custom daily rolling file writer with format: {service}{YYYYMMDD}.log
    let custom_writer =
        DailyRollingWriter::new(runtime.service_name.clone(), runtime.log_dir.clone())?;
    let (non_blocking, guard) = tracing_appender::non_blocking(custom_writer);

    // Swap guard (drop old to close deleted handle)
    if let Ok(mut guards) = GUARDS
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .lock()
    {
        guards.clear();
        guards.push(guard);
    }

    // Reload the writer in the reloadable wrapper
    if let Some(writer) = RELOADABLE_WRITER.get() {
        writer.reload(non_blocking);
    } else {
        return Err("reloadable writer not initialized".into());
    }

    // Touch today's file to ensure it exists
    let log_file_path = runtime.log_dir.join(format!(
        "{}.log.{}",
        runtime.service_name,
        chrono::Local::now().format("%Y-%m-%d")
    ));
    if !log_file_path.exists() {
        let _ = fs::File::create(&log_file_path);
    }

    // Also reopen API logger if initialized
    if let Some(api_runtime_arc) = API_LOG_RUNTIME.get() {
        if let Ok(api_runtime) = api_runtime_arc.lock() {
            let api_runtime = api_runtime.clone();

            // Create new API daily rolling file writer
            let api_writer = ApiDailyRollingWriter::new(
                api_runtime.service_name.clone(),
                api_runtime.log_dir.clone(),
            )?;
            let (non_blocking, guard) = tracing_appender::non_blocking(api_writer);

            // Swap API guard
            if let Some(api_guard_storage) = API_GUARD.get() {
                if let Ok(mut api_guard) = api_guard_storage.lock() {
                    *api_guard = Some(guard);
                }
            }

            // Reload the API writer
            if let Some(api_writer) = API_RELOADABLE_WRITER.get() {
                api_writer.reload(non_blocking);
            }

            // Touch today's API file to ensure it exists
            let api_log_file_path = api_runtime.log_dir.join(format!(
                "{}_api{}.log",
                api_runtime.service_name,
                chrono::Local::now().format("%Y%m%d")
            ));
            if !api_log_file_path.exists() {
                let _ = fs::File::create(&api_log_file_path);
            }
        }
    }

    tracing::info!("Log writer reopened successfully");
    Ok(())
}

/// Install SIGHUP listener to reopen logs on demand (Unix only)
pub fn enable_sighup_log_reopen() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        tokio::spawn(async move {
            match signal(SignalKind::hangup()) {
                Ok(mut hup) => loop {
                    hup.recv().await;
                    if let Err(e) = reopen_logs_now() {
                        tracing::warn!("Failed to reopen logs on SIGHUP: {}", e);
                    }
                },
                Err(e) => tracing::warn!("Failed to install SIGHUP handler: {}", e),
            }
        });
    }
}

/// Sanitize filename to be filesystem-safe
/// - Replaces spaces with underscores
/// - Removes/replaces invalid characters: /\:*?"<>|
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ' ' => '_',
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Write directly to channel log file
pub fn write_to_channel_log(
    channel_id: u32,
    channel_name: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Local;
    use std::io::Write;

    // Sanitize channel name for filesystem
    let safe_name = sanitize_filename(channel_name);

    // Create channel-specific subdirectory. Include id to keep uniqueness
    let dir_name = format!("{}_{}", channel_id, safe_name);
    let log_dir = PathBuf::from(format!("logs/comsrv/channels/{}", dir_name));
    fs::create_dir_all(&log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d");
    // Include id in filename as well to avoid collisions if directory listing is flattened
    let log_file = log_dir.join(format!("{}_{}.log.{}", channel_id, safe_name, timestamp));

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

    // Use logs directory (relative to working directory)
    let log_dir = PathBuf::from("logs/modsrv/models");
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

/// Write directly to instance log file
pub fn write_to_instance_log(
    instance_name: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Local;
    use std::io::Write;

    // Use logs directory (relative to working directory)
    let log_dir = PathBuf::from(format!("logs/modsrv/instances/{}", instance_name));
    fs::create_dir_all(&log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d");
    let log_file = log_dir.join(format!("{}.log.{}", instance_name, timestamp));

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

/// Write directly to rule log file
pub fn write_to_rule_log(rule_id: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::Local;
    use std::io::Write;

    // Use logs directory (relative to working directory)
    let log_dir = PathBuf::from(format!("logs/rulesrv/rules/{}", rule_id));
    fs::create_dir_all(&log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d");
    let log_file = log_dir.join(format!("{}.log.{}", rule_id, timestamp));

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

/// Log to a specific channel file only (for comsrv)
#[macro_export]
macro_rules! log_to_channel {
    ($channel_id:expr, $channel_name:expr, $level:expr, $($arg:tt)*) => {{
        let channel_id_val = $channel_id;
        let channel_name_val = $channel_name;
        let message = format!($($arg)*);

        // Only write to channel-specific log file
        if let Err(e) = $crate::logging::write_to_channel_log(channel_id_val as u32, &channel_name_val, &message) {
            tracing::warn!("Failed to write to channel {} log: {}", channel_id_val, e);
        }
    }};
}

/// Log to service system log only (for comsrv)
#[macro_export]
macro_rules! log_to_service {
    ($channel_id:expr, $level:expr, $($arg:tt)*) => {{
        let channel_id_val = $channel_id;
        let message = format!($($arg)*);

        // Only log to main service log with channel_id field
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

/// Log to a specific instance (for modsrv)
#[macro_export]
macro_rules! log_to_instance {
    ($instance_name:expr, $level:expr, $($arg:tt)*) => {{
        let instance_name_val = $instance_name;
        let message = format!($($arg)*);

        // Log to main service log with instance_name field
        match $level {
            tracing::Level::ERROR => {
                tracing::error!(target: "modsrv", instance = %instance_name_val, "{}", &message);
            },
            tracing::Level::WARN => {
                tracing::warn!(target: "modsrv", instance = %instance_name_val, "{}", &message);
            },
            tracing::Level::INFO => {
                tracing::info!(target: "modsrv", instance = %instance_name_val, "{}", &message);
            },
            tracing::Level::DEBUG => {
                tracing::debug!(target: "modsrv", instance = %instance_name_val, "{}", &message);
            },
            tracing::Level::TRACE => {
                tracing::trace!(target: "modsrv", instance = %instance_name_val, "{}", &message);
            },
        }

        // Also write to instance-specific log file
        if let Err(e) = $crate::logging::write_to_instance_log(&instance_name_val.to_string(), &message) {
            tracing::debug!("Failed to write to instance {} log: {}", instance_name_val, e);
        }
    }};
}

// ==================== Log Compression Support ====================

use tokio::time::{interval, Duration};

/// Start background log compression task
pub fn start_log_compression_task(log_dir: PathBuf, service_name: String) {
    tokio::spawn(async move {
        // Initial delay of 1 minute to let service fully start
        tokio::time::sleep(Duration::from_secs(60)).await;

        // Then run compression task every 24 hours
        let mut interval = interval(Duration::from_secs(86400)); // 24 hours

        loop {
            interval.tick().await;
            if let Err(e) = compress_old_logs(&log_dir, &service_name).await {
                tracing::error!("Log compression error for {}: {}", service_name, e);
            }
        }
    });
}

/// Compress log files older than 7 days, delete compressed logs older than 365 days
async fn compress_old_logs(
    log_dir: &Path,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};

    let mut entries = tokio::fs::read_dir(log_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Skip non-log files (check both regular and API log patterns)
        let is_regular_log = file_name.starts_with(service_name) && file_name.contains(".log.");
        let is_api_log =
            file_name.starts_with(&format!("{}_api", service_name)) && file_name.ends_with(".log");

        if !is_regular_log && !is_api_log && !file_name.ends_with(".log.gz") {
            continue;
        }

        let metadata = tokio::fs::metadata(&path).await?;
        let modified = metadata.modified()?;
        let age = SystemTime::now().duration_since(modified)?;

        // Process uncompressed log files
        if !file_name.ends_with(".gz") {
            // Compress logs older than 7 days
            if age > Duration::from_secs(7 * 86400) {
                compress_file(&path).await?;
                tokio::fs::remove_file(&path).await?; // Remove original file
                tracing::info!("Compressed log file: {}", file_name);
            }
        } else {
            // Delete compressed logs older than 365 days
            if age > Duration::from_secs(365 * 86400) {
                tokio::fs::remove_file(&path).await?;
                tracing::info!("Deleted old compressed log: {}", file_name);
            }
        }
    }

    // Also process channels and models subdirectories if they exist
    if service_name == "comsrv" {
        compress_channels_logs(log_dir, 7, 365).await?;
    }
    if service_name == "modsrv" {
        compress_subdirectory_logs(log_dir, "models", 7, 365).await?;
        compress_instances_logs(log_dir, 7, 365).await?;
    }
    if service_name == "rulesrv" {
        compress_rules_logs(log_dir, 7, 365).await?;
    }

    Ok(())
}

/// Compress log files in subdirectories
async fn compress_subdirectory_logs(
    log_dir: &Path,
    subdir: &str,
    compress_after_days: u64,
    delete_after_days: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};

    let subdir_path = log_dir.join(subdir);
    if !subdir_path.exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&subdir_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Skip non-log files
        if !file_name.contains(".log.") {
            continue;
        }

        let metadata = tokio::fs::metadata(&path).await?;
        let modified = metadata.modified()?;
        let age = SystemTime::now().duration_since(modified)?;

        if !file_name.ends_with(".gz") {
            // Compress old logs
            if age > Duration::from_secs(compress_after_days * 86400) {
                compress_file(&path).await?;
                tokio::fs::remove_file(&path).await?;
                tracing::info!("Compressed log file: {}/{}", subdir, file_name);
            }
        } else {
            // Delete expired compressed logs
            if age > Duration::from_secs(delete_after_days * 86400) {
                tokio::fs::remove_file(&path).await?;
                tracing::info!("Deleted old compressed log: {}/{}", subdir, file_name);
            }
        }
    }

    Ok(())
}

/// Compress log files in channels subdirectories (nested structure for comsrv)
async fn compress_channels_logs(
    log_dir: &Path,
    compress_after_days: u64,
    delete_after_days: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};

    let channels_dir = log_dir.join("channels");
    if !channels_dir.exists() {
        return Ok(());
    }

    // Iterate through all channel directories
    let mut channel_entries = tokio::fs::read_dir(&channels_dir).await?;

    while let Some(channel_entry) = channel_entries.next_entry().await? {
        let channel_path = channel_entry.path();

        // Skip if not a directory
        if !channel_path.is_dir() {
            continue;
        }

        let channel_name = match channel_path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Process log files in this channel directory
        let mut log_entries = tokio::fs::read_dir(&channel_path).await?;

        while let Some(log_entry) = log_entries.next_entry().await? {
            let path = log_entry.path();
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };

            // Skip non-log files
            if !file_name.contains(".log.") {
                continue;
            }

            let metadata = tokio::fs::metadata(&path).await?;
            let modified = metadata.modified()?;
            let age = SystemTime::now().duration_since(modified)?;

            if !file_name.ends_with(".gz") {
                // Compress old logs
                if age > Duration::from_secs(compress_after_days * 86400) {
                    compress_file(&path).await?;
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!(
                        "Compressed channel log file: channels/{}/{}",
                        channel_name,
                        file_name
                    );
                }
            } else {
                // Delete expired compressed logs
                if age > Duration::from_secs(delete_after_days * 86400) {
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!(
                        "Deleted old compressed channel log: channels/{}/{}",
                        channel_name,
                        file_name
                    );
                }
            }
        }
    }

    Ok(())
}

/// Compress log files in instances subdirectories (nested structure)
async fn compress_instances_logs(
    log_dir: &Path,
    compress_after_days: u64,
    delete_after_days: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};

    let instances_dir = log_dir.join("instances");
    if !instances_dir.exists() {
        return Ok(());
    }

    // Iterate through all instance directories
    let mut instance_entries = tokio::fs::read_dir(&instances_dir).await?;

    while let Some(instance_entry) = instance_entries.next_entry().await? {
        let instance_path = instance_entry.path();

        // Skip if not a directory
        if !instance_path.is_dir() {
            continue;
        }

        let instance_name = match instance_path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Process log files in this instance directory
        let mut log_entries = tokio::fs::read_dir(&instance_path).await?;

        while let Some(log_entry) = log_entries.next_entry().await? {
            let path = log_entry.path();
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };

            // Skip non-log files
            if !file_name.contains(".log.") {
                continue;
            }

            let metadata = tokio::fs::metadata(&path).await?;
            let modified = metadata.modified()?;
            let age = SystemTime::now().duration_since(modified)?;

            if !file_name.ends_with(".gz") {
                // Compress old logs
                if age > Duration::from_secs(compress_after_days * 86400) {
                    compress_file(&path).await?;
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!(
                        "Compressed instance log file: instances/{}/{}",
                        instance_name,
                        file_name
                    );
                }
            } else {
                // Delete expired compressed logs
                if age > Duration::from_secs(delete_after_days * 86400) {
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!(
                        "Deleted old compressed instance log: instances/{}/{}",
                        instance_name,
                        file_name
                    );
                }
            }
        }
    }

    Ok(())
}

/// Compress log files in rules subdirectories (nested structure for rulesrv)
async fn compress_rules_logs(
    log_dir: &Path,
    compress_after_days: u64,
    delete_after_days: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};

    let rules_dir = log_dir.join("rules");
    if !rules_dir.exists() {
        return Ok(());
    }

    // Iterate through all rule directories
    let mut rule_entries = tokio::fs::read_dir(&rules_dir).await?;

    while let Some(rule_entry) = rule_entries.next_entry().await? {
        let rule_path = rule_entry.path();

        // Skip if not a directory
        if !rule_path.is_dir() {
            continue;
        }

        let rule_id = match rule_path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Process log files in this rule directory
        let mut log_entries = tokio::fs::read_dir(&rule_path).await?;

        while let Some(log_entry) = log_entries.next_entry().await? {
            let path = log_entry.path();
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };

            // Skip non-log files
            if !file_name.contains(".log.") {
                continue;
            }

            let metadata = tokio::fs::metadata(&path).await?;
            let modified = metadata.modified()?;
            let age = SystemTime::now().duration_since(modified)?;

            if !file_name.ends_with(".gz") {
                // Compress old logs
                if age > Duration::from_secs(compress_after_days * 86400) {
                    compress_file(&path).await?;
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!("Compressed rule log file: rules/{}/{}", rule_id, file_name);
                }
            } else {
                // Delete expired compressed logs
                if age > Duration::from_secs(delete_after_days * 86400) {
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!(
                        "Deleted old compressed rule log: rules/{}/{}",
                        rule_id,
                        file_name
                    );
                }
            }
        }
    }

    Ok(())
}

/// Compress a single file
async fn compress_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use tokio::io::AsyncReadExt;

    // Read original file
    let mut input = tokio::fs::File::open(path).await?;
    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer).await?;

    // Compress to new file
    let output_path = format!("{}.gz", path.display());
    let output = std::fs::File::create(&output_path)?;
    let mut encoder = GzEncoder::new(output, Compression::best());
    encoder.write_all(&buffer)?;
    encoder.finish()?;

    Ok(())
}

// ============================================================================
// HTTP API Request Logging Middleware
// ============================================================================

/// HTTP API request logger middleware
///
/// Provides selective HTTP request logging based on the current tracing log level:
/// - **INFO level**: Logs only POST and PUT requests
/// - **DEBUG level**: Logs all HTTP requests
///
/// Logs are routed to dedicated API log files via the "api_access" target.
///
/// # Design Decisions
///
/// - **Active Level Checking**: Uses `tracing::level_enabled!()` to avoid unnecessary
///   parameter extraction when logging is disabled (performance optimization)
/// - **Dedicated Target**: All logs use `target: "api_access"` for file routing separation
/// - **Selective Logging**: INFO level filters to only POST/PUT to reduce noise
///
/// # Logged Information
/// - HTTP method (POST, GET, PUT, DELETE)
/// - Request path (e.g., `/api/channels`, `/api/instances`)
/// - HTTP status code (e.g., 200, 404, 500)
/// - Response duration in milliseconds
/// - Request headers (User-Agent, Content-Type, Content-Length)
/// - Error status (is_error: true/false)
///
/// # Example Log Output
///
/// INFO level (written to `{service}_api{YYYYMMDD}.log`):
/// ```text
/// INFO  HTTP request method=POST path=/api/channels status=201 duration_ms=45 user_agent="curl/8.1.0" content_type="application/json"
/// INFO  HTTP request method=PUT path=/api/channels/1 status=200 duration_ms=23
/// ```
///
/// DEBUG level (written to `{service}_api{YYYYMMDD}.log`):
/// ```text
/// DEBUG HTTP request method=GET path=/health status=200 duration_ms=5
/// DEBUG HTTP request method=POST path=/api/channels status=201 duration_ms=45 user_agent="curl/8.1.0"
/// DEBUG HTTP request method=DELETE path=/api/channels/999 status=404 duration_ms=8
/// ```
///
/// # Usage
///
/// Add this middleware to your Axum router **before** `.with_state()`:
/// ```rust,ignore
/// use axum::{Router, middleware};
/// use common::logging::http_request_logger;
///
/// let app = Router::new()
///     // ... routes ...
///     .layer(middleware::from_fn(http_request_logger))  // BEFORE .with_state()
///     .with_state(state);
/// ```
pub async fn http_request_logger(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use std::time::Instant;
    use tracing::{debug, info};

    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    // Extract request headers (must clone before moving req)
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "-".to_string());
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "-".to_string());
    let content_length = req
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "-".to_string());

    // Execute the request
    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();
    let is_error = status.is_client_error() || status.is_server_error();

    // Log API requests with appropriate level
    // The tracing subscriber's filter will decide which logs are actually written based on api_access target level

    // Always try DEBUG level first (logs all requests if api_access is at DEBUG)
    debug!(
        target: "api_access",
        method = %method,
        path = %uri.path(),
        status = %status.as_u16(),
        duration_ms = %duration.as_millis(),
        user_agent = %user_agent,
        content_type = %content_type,
        content_length = %content_length,
        is_error = %is_error,
        "HTTP request"
    );

    // Fallback to INFO level for POST/PUT (if api_access is at INFO but not DEBUG)
    // This ensures POST/PUT are logged even when api_access target is set to INFO
    if method == "POST" || method == "PUT" {
        info!(
            target: "api_access",
            method = %method,
            path = %uri.path(),
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            user_agent = %user_agent,
            content_type = %content_type,
            content_length = %content_length,
            is_error = %is_error,
            "HTTP request"
        );
    }

    response
}
