//! Unified logging module for VoltageEMS services
//!
//! Provides multi-level logging support with automatic sub-logger creation

use std::fs::{self, File, OpenOptions};
#[allow(unused_imports)] // Used in Write trait impl for DailyRollingWriter
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use flate2::write::GzEncoder;
use flate2::Compression;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{
        self,
        format::{FmtSpan, Writer},
        FmtContext, FormatEvent, FormatFields, MakeWriter,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    reload,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Custom format for log level with brackets: `[INFO]`, `[WARN]`, etc.
fn format_level(level: &Level) -> &'static str {
    match *level {
        Level::TRACE => "[TRACE]",
        Level::DEBUG => "[DEBUG]",
        Level::INFO => "[INFO]",
        Level::WARN => "[WARN]",
        Level::ERROR => "[ERROR]",
    }
}

/// Custom event formatter that outputs: `timestamp [LEVEL] message`
///
/// Example output: `2025-12-02T00:50:44.809Z [INFO] Service started`
struct BracketedLevelFormat;

impl<S, N> FormatEvent<S, N> for BracketedLevelFormat
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // Format timestamp
        let now = chrono::Utc::now();
        write!(writer, "{} ", now.format("%Y-%m-%dT%H:%M:%S%.6fZ"))?;

        // Format level with brackets and color
        let level = *event.metadata().level();
        if writer.has_ansi_escapes() {
            let color = match level {
                Level::TRACE => "\x1b[35m", // magenta
                Level::DEBUG => "\x1b[34m", // blue
                Level::INFO => "\x1b[32m",  // green
                Level::WARN => "\x1b[33m",  // yellow
                Level::ERROR => "\x1b[31m", // red
            };
            write!(writer, "{}{}\x1b[0m ", color, format_level(&level))?;
        } else {
            write!(writer, "{} ", format_level(&level))?;
        }

        // Format the event message and fields
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

// Global guards for keeping loggers alive
static GUARDS: OnceLock<Arc<Mutex<Vec<WorkerGuard>>>> = OnceLock::new();
// API logger guard - separate to allow independent lifecycle
static API_GUARD: OnceLock<Arc<Mutex<Option<WorkerGuard>>>> = OnceLock::new();

// ============================================================================
// Log Root Directory Configuration
// ============================================================================

/// Global log root directory (initialized once from config or env)
/// Priority: VOLTAGE_LOG_DIR env > config_dir > default "logs"
static LOG_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Initialize log root directory from config or environment
///
/// This should be called early during service bootstrap, before any logging
/// functions that write to files are invoked.
///
/// Priority:
/// 1. `VOLTAGE_LOG_DIR` environment variable (highest)
/// 2. `config_dir` parameter (from SQLite config)
/// 3. Default value "logs" (lowest)
pub fn init_log_root(config_dir: Option<&str>) {
    LOG_ROOT.get_or_init(|| {
        std::env::var("VOLTAGE_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                config_dir
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("logs"))
            })
    });
}

/// Get log root directory
///
/// Returns the configured log root directory. If `init_log_root` was not called,
/// falls back to checking environment variable or default "logs".
///
/// When running under `cargo test` (detected via CARGO_TARGET_TMPDIR or test binary path),
/// defaults to system temp directory to avoid polluting the project directory.
pub fn get_log_root() -> PathBuf {
    LOG_ROOT.get().cloned().unwrap_or_else(|| {
        std::env::var("VOLTAGE_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                // Detect test environment: cargo sets CARGO_TARGET_TMPDIR during tests
                // or we can check if running from target/debug/deps (test binaries)
                if is_test_environment() {
                    std::env::temp_dir().join("voltage-test-logs")
                } else {
                    PathBuf::from("logs")
                }
            })
    })
}

/// Detect if we're running in a test environment
fn is_test_environment() -> bool {
    // Method 1: Check CARGO_TARGET_TMPDIR (set by cargo during test runs)
    if std::env::var("CARGO_TARGET_TMPDIR").is_ok() {
        return true;
    }

    // Method 2: Check if executable is in target/debug/deps (test binary location)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(path_str) = exe.to_str() {
            if path_str.contains("target/debug/deps") || path_str.contains("target/release/deps") {
                return true;
            }
        }
    }

    false
}

/// Default max file size: 100MB
const DEFAULT_MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

// Custom daily rolling file writer with naming format: {service}{YYYYMMDD}.log
// Also supports size-based rotation within a day
struct DailyRollingWriter {
    service_name: String,
    log_dir: PathBuf,
    current_date: Arc<Mutex<String>>,
    current_file: Arc<Mutex<Option<File>>>,
    /// Current file size in bytes (tracked for size-based rotation)
    current_size: Arc<AtomicU64>,
    /// Max file size before rotation (default 100MB)
    max_file_size: u64,
    /// Rotation counter within the same day (e.g., .1, .2, .3)
    rotation_count: Arc<AtomicU32>,
}

impl DailyRollingWriter {
    fn new(service_name: String, log_dir: PathBuf) -> std::io::Result<Self> {
        Self::with_max_size(service_name, log_dir, DEFAULT_MAX_FILE_SIZE)
    }

    fn with_max_size(
        service_name: String,
        log_dir: PathBuf,
        max_file_size: u64,
    ) -> std::io::Result<Self> {
        let current_date = chrono::Local::now().format("%Y%m%d").to_string();
        let file_path = log_dir.join(format!("{}_{}.log", current_date, service_name));

        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir)?;

        // Open or create the log file and get its current size
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        let initial_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        Ok(Self {
            service_name,
            log_dir,
            current_date: Arc::new(Mutex::new(current_date)),
            current_file: Arc::new(Mutex::new(Some(file))),
            current_size: Arc::new(AtomicU64::new(initial_size)),
            max_file_size,
            rotation_count: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Rotate the log file due to size limit
    fn rotate_by_size(&self) -> std::io::Result<()> {
        let current_date = self
            .current_date
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;

        // Increment rotation counter
        let count = self.rotation_count.fetch_add(1, Ordering::SeqCst) + 1;

        // New file path: YYYYMMDD_service.N.log
        let new_file_path = self.log_dir.join(format!(
            "{}_{}.{}.log",
            *current_date, self.service_name, count
        ));

        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&new_file_path)?;

        // Reset size counter
        self.current_size.store(0, Ordering::SeqCst);

        // Update current file
        let mut current_file = self
            .current_file
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;
        *current_file = Some(new_file);

        Ok(())
    }

    fn get_writer(&self) -> std::io::Result<std::sync::MutexGuard<'_, Option<File>>> {
        // Check if date has changed
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let mut current_date = self
            .current_date
            .lock()
            .map_err(|e| std::io::Error::other(format!("Mutex poisoned: {}", e)))?;

        // Build current file path
        let current_file_path = self
            .log_dir
            .join(format!("{}_{}.log", *current_date, self.service_name));

        // Check if date changed OR file was deleted
        let file_deleted = !current_file_path.exists();

        if *current_date != today || file_deleted {
            // Date changed or file deleted, rotate to new file and reset rotation counter
            let new_date = if *current_date != today {
                today.clone()
            } else {
                current_date.clone()
            };
            let new_file_path = self
                .log_dir
                .join(format!("{}_{}.log", new_date, self.service_name));

            // Ensure directory exists (in case it was also deleted)
            fs::create_dir_all(&self.log_dir)?;

            let new_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_file_path)?;
            let initial_size = new_file.metadata().map(|m| m.len()).unwrap_or(0);

            // Update current date, file, and reset counters
            if *current_date != today {
                *current_date = today;
                self.rotation_count.store(0, Ordering::SeqCst);
            }
            self.current_size.store(initial_size, Ordering::SeqCst);

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
        // Check if we need to rotate due to size limit
        let current_size = self.current_size.load(Ordering::Relaxed);
        if current_size + buf.len() as u64 > self.max_file_size {
            self.rotate_by_size()?;
        }

        if let Some(ref mut file) = *self.get_writer()? {
            let written = file.write(buf)?;
            // Update size counter
            self.current_size
                .fetch_add(written as u64, Ordering::Relaxed);
            Ok(written)
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
            current_size: Arc::clone(&self.current_size),
            max_file_size: self.max_file_size,
            rotation_count: Arc::clone(&self.rotation_count),
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
        let file_path = log_dir.join(format!("{}_{}_api.log", current_date, service_name));

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
                .join(format!("{}_{}_api.log", today, self.service_name));
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

// Dynamic log level reload support
type EnvFilterReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;
static LOG_FILTER_HANDLE: OnceLock<EnvFilterReloadHandle> = OnceLock::new();
static CURRENT_LOG_LEVEL: OnceLock<Mutex<String>> = OnceLock::new();

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
            log_dir: get_log_root(),
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

    let (env_filter, initial_level_str) = if let Ok(env_str) = std::env::var("RUST_LOG") {
        // RUST_LOG is set - only append api_access if not already specified
        if env_str.contains("api_access") {
            // User explicitly set api_access level in RUST_LOG, respect it
            (EnvFilter::new(env_str.clone()), env_str)
        } else {
            // api_access not in RUST_LOG, use config default
            // IMPORTANT: If RUST_LOG contains "debug" or "trace", upgrade api_access to debug for full visibility
            let effective_api_level = if env_str.contains("debug") || env_str.contains("trace") {
                "debug"
            } else {
                api_level
            };
            let filter_str = format!("{},api_access={}", env_str, effective_api_level);
            (EnvFilter::new(filter_str.clone()), filter_str)
        }
    } else {
        // RUST_LOG not set - use default with api_access
        let filter_str = format!(
            "info,{}=debug,api_access={}",
            config.service_name, api_level
        );
        (EnvFilter::new(filter_str.clone()), filter_str)
    };

    // Wrap EnvFilter with reload::Layer for dynamic level changes
    let (reload_filter, reload_handle) = reload::Layer::new(env_filter);
    let _ = LOG_FILTER_HANDLE.set(reload_handle);
    let _ = CURRENT_LOG_LEVEL.set(Mutex::new(initial_level_str));

    let registry = tracing_subscriber::registry().with(reload_filter);

    // Console layer - format only, level filtering handled by reload_filter
    // NOTE: Removed per-layer LevelFilter to allow dynamic log level changes via API
    // Custom format: 2025-12-02T00:50:44.809Z [INFO] message
    let console_layer = fmt::layer()
        .with_ansi(true)
        .event_format(BracketedLevelFormat)
        .boxed();

    // Create reloadable writer
    let reloadable_writer = ReloadableWriter::new(non_blocking);
    let reloadable_writer_arc = Arc::new(reloadable_writer);

    // Store the reloadable writer globally
    RELOADABLE_WRITER.get_or_init(|| reloadable_writer_arc.clone());

    // Wrap in newtype for MakeWriter implementation
    let writer_handle = ReloadableWriterHandle(reloadable_writer_arc.clone());

    // Business file layer (exclude api_access target)
    use tracing_subscriber::filter;

    // Business file layer - excludes api_access target only
    // NOTE: Removed LevelFilter to allow dynamic log level changes via API
    // Level filtering is now handled by the top-level reload_filter
    let business_file_layer = if config.enable_json {
        fmt::layer()
            .json()
            .with_writer(writer_handle.clone())
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::FULL)
            .with_filter(filter::filter_fn(|metadata| {
                metadata.target() != "api_access"
            }))
            .boxed()
    } else {
        // Simplified format: no module paths, no thread IDs (saves ~40 chars/line)
        fmt::layer()
            .with_writer(writer_handle)
            .with_ansi(false)
            .event_format(BracketedLevelFormat) // Use [INFO] format like console
            .with_filter(filter::filter_fn(|metadata| {
                metadata.target() != "api_access"
            }))
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
                .event_format(BracketedLevelFormat) // Use [INFO] format like console
                .with_filter(filter::filter_fn(|metadata| metadata.target() == "api_access"))
                .boxed(),
        )
    } else {
        None
    };

    // Register all layers
    // Note: Using .with(Option<Layer>) which acts as identity when None
    // Console layer handles both business and API logs (api_access target)
    // API file layer only handles api_access target for separate API log file
    registry
        .with(console_layer)
        .with(business_file_layer)
        .with(api_file_layer)
        .init();

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

    tracing::info!("Logging: {} @ {:?}", config.service_name, config.log_dir);

    if config.enable_api_log {
        let current_date = chrono::Local::now().format("%Y%m%d");
        tracing::debug!("API log: {}_{}_api.log", current_date, config.service_name);
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
        "{}_{}.log",
        chrono::Local::now().format("%Y%m%d"),
        runtime.service_name
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
                "{}_{}_api.log",
                chrono::Local::now().format("%Y%m%d"),
                api_runtime.service_name
            ));
            if !api_log_file_path.exists() {
                let _ = fs::File::create(&api_log_file_path);
            }
        }
    }

    tracing::debug!("Log reopened");
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
                        tracing::warn!("SIGHUP reopen: {}", e);
                    }
                },
                Err(e) => tracing::warn!("SIGHUP handler: {}", e),
            }
        });
    }
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

/// Dynamically set log filter level at runtime
///
/// # Arguments
/// * `level` - Log level string (e.g., "debug", "info", "warn", "error", "trace")
///   or full filter spec (e.g., "info,comsrv=debug")
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(String)` with error message on failure
///
/// # Example
/// ```ignore
/// common::logging::set_log_level("debug")?;
/// common::logging::set_log_level("info,comsrv=debug")?;
/// ```
pub fn set_log_level(level: &str) -> Result<(), String> {
    let handle = LOG_FILTER_HANDLE
        .get()
        .ok_or("Logging not initialized with reload support")?;

    let new_filter =
        EnvFilter::try_new(level).map_err(|e| format!("Invalid log level '{}': {}", level, e))?;

    handle
        .reload(new_filter)
        .map_err(|e| format!("Failed to reload log filter: {}", e))?;

    // Update stored level
    if let Some(current) = CURRENT_LOG_LEVEL.get() {
        if let Ok(mut guard) = current.lock() {
            *guard = level.to_string();
        }
    }

    tracing::info!("Log level changed to: {}", level);
    Ok(())
}

/// Get current log filter level
///
/// # Returns
/// Current log filter string
pub fn get_log_level() -> String {
    CURRENT_LOG_LEVEL
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.clone())
        .unwrap_or_else(|| "unknown".to_string())
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
        // New format: {YYYYMMDD}_{service}.log or {YYYYMMDD}_{service}_api.log
        let is_regular_log =
            file_name.contains(&format!("_{}.log", service_name)) && !file_name.contains("_api");
        let is_api_log = file_name.contains(&format!("_{}_api.log", service_name));

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
                tracing::debug!("Compressed: {}", file_name);
            }
        } else {
            // Delete compressed logs older than 365 days
            if age > Duration::from_secs(365 * 86400) {
                tokio::fs::remove_file(&path).await?;
                tracing::debug!("Deleted: {}", file_name);
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

/// Redact sensitive fields in JSON string
///
/// Recursively searches for sensitive field names and replaces their values with "***REDACTED***".
/// Handles nested objects and arrays.
///
/// # Sensitive Fields
/// - password
/// - token
/// - api_key
/// - secret
/// - authorization
///
/// # Example
/// ```rust,ignore
/// let json = r#"{"username":"admin","password":"secret123"}"#;
/// let redacted = redact_sensitive_fields(json);
/// // Result: r#"{"username":"admin","password":"***REDACTED***"}"#
/// ```
#[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (compile-time safe, never panics)
fn redact_sensitive_fields(json_str: &str) -> String {
    use serde_json::{json, Value};

    const SENSITIVE_KEYS: &[&str] = &["password", "token", "api_key", "secret", "authorization"];

    // Try to parse as JSON
    let Ok(mut value) = serde_json::from_str::<Value>(json_str) else {
        // If not valid JSON, return as-is
        return json_str.to_string();
    };

    // Recursive redaction function
    fn redact_recursive(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let key_lower = key.to_lowercase();
                    if SENSITIVE_KEYS.iter().any(|&k| key_lower.contains(k)) {
                        // Replace sensitive value with redacted marker
                        *val = json!("***REDACTED***");
                    } else {
                        // Recursively process nested objects/arrays
                        redact_recursive(val);
                    }
                }
            },
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    redact_recursive(item);
                }
            },
            _ => {},
        }
    }

    redact_recursive(&mut value);

    // Serialize back to string (compact format)
    serde_json::to_string(&value).unwrap_or_else(|_| json_str.to_string())
}

/// Truncate body string to maximum length
///
/// If the body exceeds max_length, it will be truncated and a suffix will be added
/// indicating how many bytes were truncated.
///
/// # Example
/// ```rust,ignore
/// let long_body = "a".repeat(1000);
/// let truncated = truncate_body(&long_body, 500);
/// // Result: "aaa...aaa[truncated 500 bytes]"
/// ```
fn truncate_body(body: &str, max_length: usize) -> String {
    if body.len() <= max_length {
        body.to_string()
    } else {
        let truncated_bytes = body.len() - max_length;
        format!(
            "{}[truncated {} bytes]",
            &body[..max_length],
            truncated_bytes
        )
    }
}

/// HTTP API request logger middleware
///
/// Provides selective HTTP request logging with request body recording:
/// - **INFO level**: Logs only POST/PUT/PATCH/DELETE requests (no body)
/// - **DEBUG level**: Logs all requests with body content (truncated & redacted)
///
/// Logs are routed to dedicated API log files via the "api_access" target.
///
/// # Design Decisions
///
/// - **Body Recording at DEBUG**: Request body is only read and logged at DEBUG level
/// - **Sensitive Field Redaction**: password, token, api_key, secret, authorization are filtered
/// - **Body Truncation**: Body limited to 500 characters to prevent log bloat
/// - **Simplified Fields**: Removed redundant headers (user_agent, content_type, content_length, is_error)
/// - **No Duplicate Logging**: INFO and DEBUG levels are mutually exclusive
///
/// # Logged Information
/// - HTTP method (POST, GET, PUT, DELETE, PATCH)
/// - Request path (e.g., `/api/channels`, `/api/instances`)
/// - HTTP status code (e.g., 200, 404, 500)
/// - Response duration in milliseconds
/// - Request body (DEBUG only, truncated to 500 chars, sensitive fields redacted)
///
/// # Example Log Output
///
/// INFO level (production, written to `{service}_api{YYYYMMDD}.log`):
/// ```text
/// INFO  HTTP request method=POST path=/api/instances status=200 duration_ms=15
/// INFO  HTTP request method=PUT path=/api/channels/1 status=200 duration_ms=23
/// ```
///
/// DEBUG level (development, written to `{service}_api{YYYYMMDD}.log`):
/// ```text
/// DEBUG HTTP request method=POST path=/api/instances status=200 duration_ms=15 request_body={"instance_name":"test","properties":{...}}[truncated 234 bytes]
/// DEBUG HTTP request method=GET path=/health status=200 duration_ms=5 request_body=-
/// DEBUG HTTP request method=POST path=/api/auth/login status=200 duration_ms=50 request_body={"username":"admin","password":"***REDACTED***"}
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
    use axum::body::Body;
    use std::time::Instant;
    use tracing::{debug, info, level_enabled, Level};

    const MAX_BODY_LENGTH: usize = 500;

    let method = req.method().clone();
    let uri = req.uri().clone();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let start = Instant::now();

    // Only read body at DEBUG level and for modifying methods (POST/PUT/PATCH/DELETE)
    let should_read_body = level_enabled!(Level::DEBUG)
        && matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE")
        && content_type.contains("application/json");

    let (req, body_str) = if should_read_body {
        // Read body bytes
        let (parts, body) = req.into_parts();
        let bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Failed to read request body: {}", e);
                // Reconstruct request with empty body and continue
                let new_req = axum::extract::Request::from_parts(parts, Body::empty());
                return next.run(new_req).await;
            },
        };

        // Convert to string
        let body_str = match std::str::from_utf8(&bytes) {
            Ok(s) => {
                // Apply redaction and truncation
                let redacted = redact_sensitive_fields(s);
                truncate_body(&redacted, MAX_BODY_LENGTH)
            },
            Err(_) => "<binary data>".to_string(),
        };

        // Reconstruct request with original bytes
        let new_req = axum::extract::Request::from_parts(parts, Body::from(bytes));
        (new_req, body_str)
    } else {
        // Don't read body, use placeholder
        (req, "-".to_string())
    };

    // Execute the request
    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    // Log based on level and method
    // INFO level: Log only modifying methods (POST/PUT/PATCH/DELETE) without body
    // DEBUG level: Log ALL requests with body content
    //
    // Note: We use separate info! and debug! calls because they have different fields.
    // The tracing subscriber will filter based on the configured level for api_access target.

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        // Always log modifying methods at INFO level (no body)
        info!(
            target: "api_access",
            method = %method,
            path = %uri.path(),
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "HTTP request"
        );
    }

    // Additionally, log all requests at DEBUG level with body (if body was read)
    if body_str != "-" {
        debug!(
            target: "api_access",
            method = %method,
            path = %uri.path(),
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            request_body = %body_str,
            "HTTP request (detailed)"
        );
    } else if !matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        // For GET requests, only log at DEBUG level
        debug!(
            target: "api_access",
            method = %method,
            path = %uri.path(),
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "HTTP request"
        );
    }

    response
}
