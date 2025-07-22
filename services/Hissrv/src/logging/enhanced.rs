use crate::config::LoggingConfig;
use crate::error::{ErrorContext, ErrorSeverity, HisSrvError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{error, info, warn, Level, Metadata, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, Layer, Registry};

/// 增强的日志系统
pub struct EnhancedLogger {
    config: LoggingConfig,
    _guards: Vec<WorkerGuard>,
    metrics: Arc<LogMetrics>,
    sampler: Arc<LogSampler>,
    filter_registry: Arc<RwLock<SensitiveDataFilter>>,
}

/// 日志指标
#[derive(Default)]
struct LogMetrics {
    total_logs: AtomicU64,
    error_count: AtomicU64,
    warning_count: AtomicU64,
    dropped_logs: AtomicU64,
}

/// 日志采样器
struct LogSampler {
    sample_rates: RwLock<HashMap<String, SampleRate>>,
}

#[derive(Clone)]
struct SampleRate {
    rate: f64,
    counter: Arc<AtomicUsize>,
}

/// 敏感数据过滤器
struct SensitiveDataFilter {
    patterns: Vec<regex::Regex>,
    replacements: HashMap<String, String>,
}

impl EnhancedLogger {
    /// 初始化增强的日志系统
    pub fn init(config: LoggingConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // 创建日志目录
        if let Some(parent) = Path::new(&config.file).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut guards = Vec::new();
        let metrics = Arc::new(LogMetrics::default());
        let sampler = Arc::new(LogSampler::new());
        let filter_registry = Arc::new(RwLock::new(SensitiveDataFilter::new()));

        // 创建文件追加器
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            Path::new(&config.file).parent().unwrap_or(Path::new(".")),
            Path::new(&config.file)
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("hissrv.log"),
        );
        let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
        guards.push(file_guard);

        // 创建控制台写入器
        let (console_writer, console_guard) = tracing_appender::non_blocking(std::io::stdout());
        guards.push(console_guard);

        // 解析日志级别
        let log_level = config.level.parse::<Level>().unwrap_or(Level::INFO);
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(format!("hissrv={}", config.level)));

        // 构建日志订阅器
        let subscriber = Registry::default().with(env_filter);

        match config.format.as_str() {
            "json" => {
                // JSON格式日志层
                let json_layer = fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(false)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_writer(file_writer)
                    .with_filter(LevelFilter::from_level(log_level));

                // 控制台层（人类可读格式）
                let console_layer = fmt::layer()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_writer(console_writer)
                    .with_filter(LevelFilter::INFO);

                subscriber
                    .with(json_layer)
                    .with(console_layer)
                    .with(MetricsLayer::new(Arc::clone(&metrics)))
                    .with(SamplingLayer::new(Arc::clone(&sampler)))
                    .with(FilterLayer::new(Arc::clone(&filter_registry)))
                    .init();
            }
            _ => {
                // 文本格式日志层
                let file_layer = fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_writer(file_writer)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(LevelFilter::from_level(log_level));

                // 控制台层
                let console_layer = fmt::layer()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_writer(console_writer)
                    .with_filter(LevelFilter::INFO);

                subscriber
                    .with(file_layer)
                    .with(console_layer)
                    .with(MetricsLayer::new(Arc::clone(&metrics)))
                    .with(SamplingLayer::new(Arc::clone(&sampler)))
                    .with(FilterLayer::new(Arc::clone(&filter_registry)))
                    .init();
            }
        }

        info!(
            "Enhanced logging initialized with level: {}, format: {}",
            config.level, config.format
        );

        Ok(Self {
            config,
            _guards: guards,
            metrics,
            sampler,
            filter_registry,
        })
    }

    /// 动态调整日志级别
    pub fn set_log_level(&self, level: &str) -> Result<(), HisSrvError> {
        // 注意：tracing不支持运行时动态修改全局过滤器
        // 这里仅作为配置更新的示例
        info!("Log level change requested to: {}", level);
        Ok(())
    }

    /// 设置特定模块的采样率
    pub fn set_sampling_rate(&self, module: &str, rate: f64) -> Result<(), HisSrvError> {
        if !(0.0..=1.0).contains(&rate) {
            return Err(HisSrvError::ValidationError {
                field: "rate".to_string(),
                message: "采样率必须在0.0到1.0之间".to_string(),
                value: Some(rate.to_string()),
            });
        }

        self.sampler.set_rate(module, rate);
        info!("Set sampling rate for {} to {}", module, rate);
        Ok(())
    }

    /// 添加敏感数据过滤规则
    pub fn add_filter_pattern(&self, pattern: &str) -> Result<(), HisSrvError> {
        let regex = regex::Regex::new(pattern).map_err(|e| HisSrvError::ParseError {
            message: format!("无效的正则表达式: {}", e),
            location: "filter_pattern".to_string(),
            raw_data: Some(pattern.to_string()),
        })?;

        self.filter_registry.write().unwrap().add_pattern(regex);
        info!("Added sensitive data filter pattern: {}", pattern);
        Ok(())
    }

    /// 获取日志指标
    pub fn get_metrics(&self) -> LogMetricsSnapshot {
        LogMetricsSnapshot {
            total_logs: self.metrics.total_logs.load(Ordering::Relaxed),
            error_count: self.metrics.error_count.load(Ordering::Relaxed),
            warning_count: self.metrics.warning_count.load(Ordering::Relaxed),
            dropped_logs: self.metrics.dropped_logs.load(Ordering::Relaxed),
        }
    }
}

/// 日志指标快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMetricsSnapshot {
    pub total_logs: u64,
    pub error_count: u64,
    pub warning_count: u64,
    pub dropped_logs: u64,
}

// 日志采样器实现
impl LogSampler {
    fn new() -> Self {
        Self {
            sample_rates: RwLock::new(HashMap::new()),
        }
    }

    fn set_rate(&self, module: &str, rate: f64) {
        let mut rates = self.sample_rates.write().unwrap();
        rates.insert(
            module.to_string(),
            SampleRate {
                rate,
                counter: Arc::new(AtomicUsize::new(0)),
            },
        );
    }

    fn should_sample(&self, module: &str) -> bool {
        let rates = self.sample_rates.read().unwrap();
        if let Some(sample_rate) = rates.get(module) {
            if sample_rate.rate >= 1.0 {
                return true;
            }
            if sample_rate.rate <= 0.0 {
                return false;
            }

            // 简单的计数采样
            let count = sample_rate.counter.fetch_add(1, Ordering::Relaxed);
            let threshold = (1.0 / sample_rate.rate) as usize;
            count % threshold == 0
        } else {
            true // 默认不采样
        }
    }
}

// 敏感数据过滤器实现
impl SensitiveDataFilter {
    fn new() -> Self {
        let mut filter = Self {
            patterns: Vec::new(),
            replacements: HashMap::new(),
        };

        // 添加默认的敏感数据模式
        let default_patterns = vec![
            r#"password["']?\s*[:=]\s*["']?[\w\-]+["']?"#,
            r#"token["']?\s*[:=]\s*["']?[\w\-]+["']?"#,
            r#"api[_-]?key["']?\s*[:=]\s*["']?[\w\-]+["']?"#,
            r"\b\d{4}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}\b", // 信用卡号
            r"\b\d{3}-\d{2}-\d{4}\b",                         // SSN
        ];

        for pattern in default_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                filter.patterns.push(regex);
            }
        }

        filter
    }

    fn add_pattern(&mut self, pattern: regex::Regex) {
        self.patterns.push(pattern);
    }

    fn filter(&self, message: &str) -> String {
        let mut filtered = message.to_string();
        for pattern in &self.patterns {
            filtered = pattern.replace_all(&filtered, "[REDACTED]").to_string();
        }
        filtered
    }
}

// 自定义日志层：指标收集
struct MetricsLayer {
    metrics: Arc<LogMetrics>,
}

impl MetricsLayer {
    fn new(metrics: Arc<LogMetrics>) -> Self {
        Self { metrics }
    }
}

impl<S> Layer<S> for MetricsLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.metrics.total_logs.fetch_add(1, Ordering::Relaxed);

        let metadata = event.metadata();
        match *metadata.level() {
            Level::ERROR => {
                self.metrics.error_count.fetch_add(1, Ordering::Relaxed);
            }
            Level::WARN => {
                self.metrics.warning_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

// 自定义日志层：采样
struct SamplingLayer {
    sampler: Arc<LogSampler>,
}

impl SamplingLayer {
    fn new(sampler: Arc<LogSampler>) -> Self {
        Self { sampler }
    }
}

impl<S> Layer<S> for SamplingLayer
where
    S: Subscriber,
{
    fn enabled(
        &self,
        metadata: &Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        // 始终记录ERROR和WARN级别
        if *metadata.level() <= Level::WARN {
            return true;
        }

        // 对其他级别应用采样
        self.sampler.should_sample(metadata.target())
    }
}

// 自定义日志层：敏感数据过滤
struct FilterLayer {
    filter: Arc<RwLock<SensitiveDataFilter>>,
}

impl FilterLayer {
    fn new(filter: Arc<RwLock<SensitiveDataFilter>>) -> Self {
        Self { filter }
    }
}

impl<S> Layer<S> for FilterLayer
where
    S: Subscriber,
{
    // 注意：tracing的Layer trait不提供修改事件内容的直接方法
    // 在实际应用中，敏感数据过滤应该在记录日志之前完成
}

// 结构化日志助手函数
pub fn log_error_with_context(error: &HisSrvError) {
    let context = error.context();

    match context.severity {
        ErrorSeverity::Critical => {
            error!(
                error = %error,
                error_type = context.error_type,
                is_retryable = context.is_retryable,
                retry_after = ?context.retry_after,
                suggestion = ?context.suggestion,
                severity = %context.severity,
                "Critical error occurred"
            );
        }
        ErrorSeverity::Error => {
            error!(
                error = %error,
                error_type = context.error_type,
                is_retryable = context.is_retryable,
                retry_after = ?context.retry_after,
                suggestion = ?context.suggestion,
                "Error occurred"
            );
        }
        ErrorSeverity::Warning => {
            warn!(
                error = %error,
                error_type = context.error_type,
                is_retryable = context.is_retryable,
                suggestion = ?context.suggestion,
                "Warning"
            );
        }
        ErrorSeverity::Info => {
            info!(
                error = %error,
                error_type = context.error_type,
                "Info"
            );
        }
    }
}

/// 性能跟踪器
pub struct PerformanceTracker {
    operation: String,
    start_time: Instant,
    metadata: HashMap<String, String>,
}

impl PerformanceTracker {
    pub fn new(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        info!(operation = %operation, "Starting operation");

        Self {
            operation,
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn complete(self) {
        let duration = self.start_time.elapsed();
        info!(
            operation = %self.operation,
            duration_ms = duration.as_millis(),
            metadata = ?self.metadata,
            "Operation completed"
        );
    }

    pub fn complete_with_error(self, error: &HisSrvError) {
        let duration = self.start_time.elapsed();
        error!(
            operation = %self.operation,
            duration_ms = duration.as_millis(),
            metadata = ?self.metadata,
            error = %error,
            error_type = error.context().error_type,
            "Operation failed"
        );
    }
}

/// 批量操作日志
pub fn log_batch_operation(
    operation: &str,
    total_items: usize,
    successful: usize,
    failed: usize,
    duration: Duration,
) {
    let success_rate = if total_items > 0 {
        (successful as f64 / total_items as f64) * 100.0
    } else {
        0.0
    };

    if failed > 0 {
        warn!(
            operation = operation,
            total = total_items,
            successful = successful,
            failed = failed,
            success_rate = format!("{:.2}%", success_rate),
            duration_ms = duration.as_millis(),
            "Batch operation completed with failures"
        );
    } else {
        info!(
            operation = operation,
            total = total_items,
            successful = successful,
            duration_ms = duration.as_millis(),
            "Batch operation completed successfully"
        );
    }
}

/// 存储操作日志
pub fn log_storage_operation(
    backend: &str,
    operation: &str,
    measurement: &str,
    point_count: usize,
    duration: Duration,
    success: bool,
) {
    if success {
        info!(
            backend = backend,
            operation = operation,
            measurement = measurement,
            point_count = point_count,
            duration_ms = duration.as_millis(),
            "Storage operation completed"
        );
    } else {
        error!(
            backend = backend,
            operation = operation,
            measurement = measurement,
            point_count = point_count,
            duration_ms = duration.as_millis(),
            "Storage operation failed"
        );
    }
}

/// API请求日志
pub fn log_api_request(
    method: &str,
    path: &str,
    status: u16,
    duration: Duration,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
) {
    info!(
        method = method,
        path = path,
        status = status,
        duration_ms = duration.as_millis(),
        client_ip = client_ip.unwrap_or("unknown"),
        user_agent = user_agent.unwrap_or("unknown"),
        "API request"
    );
}

/// 查询性能日志
pub fn log_query_performance(
    query_type: &str,
    time_range_hours: u64,
    point_count: usize,
    duration: Duration,
    cache_hit: bool,
) {
    info!(
        query_type = query_type,
        time_range_hours = time_range_hours,
        point_count = point_count,
        duration_ms = duration.as_millis(),
        cache_hit = cache_hit,
        "Query executed"
    );
}
