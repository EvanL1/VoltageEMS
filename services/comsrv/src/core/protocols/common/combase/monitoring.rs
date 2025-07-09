//! 基础监控和诊断功能
//!
//! 这个模块提供了基础的监控和诊断功能，包含：
//! - 实时性能指标收集
//! - 健康检查机制
//! - 诊断报告生成
//! - 异常检测和告警

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// 健康状态等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

impl std::fmt::Display for HealthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthLevel::Healthy => write!(f, "healthy"),
            HealthLevel::Warning => write!(f, "warning"),
            HealthLevel::Critical => write!(f, "critical"),
            HealthLevel::Unknown => write!(f, "unknown"),
        }
    }
}

/// 健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub component: String,
    pub level: HealthLevel,
    pub message: String,
    pub details: HashMap<String, String>,
    pub timestamp: SystemTime,
    pub check_duration_ms: u64,
}

/// 性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// 请求速率（每秒）
    pub request_rate: f64,
    /// 成功率（百分比）
    pub success_rate: f64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 95百分位响应时间（毫秒）
    pub p95_response_time_ms: f64,
    /// 99百分位响应时间（毫秒）
    pub p99_response_time_ms: f64,
    /// 错误率（百分比）
    pub error_rate: f64,
    /// 连接正常运行时间（秒）
    pub uptime_seconds: u64,
    /// 活跃连接数
    pub active_connections: u64,
    /// 内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// CPU使用率（百分比）
    pub cpu_usage_percent: f64,
}

/// 响应时间统计
#[derive(Debug, Clone)]
struct ResponseTimeStats {
    samples: Vec<u64>,
    max_samples: usize,
}

impl ResponseTimeStats {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::new(),
            max_samples,
        }
    }

    fn add_sample(&mut self, response_time_ms: u64) {
        self.samples.push(response_time_ms);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    fn get_percentile(&self, percentile: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let mut sorted = self.samples.clone();
        sorted.sort_unstable();

        let index = ((percentile / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[index.min(sorted.len() - 1)] as f64
    }

    fn get_average(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }

        self.samples.iter().sum::<u64>() as f64 / self.samples.len() as f64
    }
}

/// 基础监控收集器
#[derive(Debug)]
pub struct BasicMonitoring {
    /// 组件名称
    component_name: String,
    /// 开始时间
    start_time: Instant,
    /// 请求统计
    request_stats: Arc<RwLock<RequestStats>>,
    /// 响应时间统计
    response_time_stats: Arc<RwLock<ResponseTimeStats>>,
    /// 健康检查器
    health_checkers: Arc<RwLock<Vec<Box<dyn HealthChecker>>>>,
    /// 告警管理器
    alert_manager: Arc<AlertManager>,
}

/// 请求统计
#[derive(Debug, Clone, Default)]
struct RequestStats {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    last_request_time: Option<Instant>,
    request_times: Vec<Instant>, // 用于计算请求速率
}

impl RequestStats {
    fn add_request(&mut self, success: bool) {
        self.total_requests += 1;
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }

        let now = Instant::now();
        self.last_request_time = Some(now);
        self.request_times.push(now);

        // 保留最近1分钟的请求记录
        let cutoff = now - Duration::from_secs(60);
        self.request_times.retain(|&time| time > cutoff);
    }

    fn get_request_rate(&self) -> f64 {
        if self.request_times.len() < 2 {
            return 0.0;
        }

        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        let recent_requests = self
            .request_times
            .iter()
            .filter(|&&time| time > one_minute_ago)
            .count();

        recent_requests as f64 / 60.0 // 每秒请求数
    }

    fn get_success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }

    fn get_error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.failed_requests as f64 / self.total_requests as f64) * 100.0
    }
}

/// 健康检查器trait
#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync + std::fmt::Debug {
    /// 执行健康检查
    async fn check_health(&self) -> HealthCheckResult;

    /// 获取检查器名称
    fn name(&self) -> &str;
}

/// 连接健康检查器
pub struct ConnectionHealthChecker {
    name: String,
    check_connection: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl std::fmt::Debug for ConnectionHealthChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHealthChecker")
            .field("name", &self.name)
            .field("check_connection", &"<function>")
            .finish()
    }
}

impl ConnectionHealthChecker {
    pub fn new<F>(name: String, check_fn: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        Self {
            name,
            check_connection: Arc::new(check_fn),
        }
    }
}

#[async_trait::async_trait]
impl HealthChecker for ConnectionHealthChecker {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let is_connected = (self.check_connection)();
        let check_duration = start_time.elapsed();

        let (level, message) = if is_connected {
            (HealthLevel::Healthy, "连接正常".to_string())
        } else {
            (HealthLevel::Critical, "连接断开".to_string())
        };

        let mut details = HashMap::new();
        details.insert("connected".to_string(), is_connected.to_string());

        HealthCheckResult {
            component: self.name.clone(),
            level,
            message,
            details,
            timestamp: SystemTime::now(),
            check_duration_ms: check_duration.as_millis() as u64,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 性能健康检查器
#[derive(Debug)]
pub struct PerformanceHealthChecker {
    name: String,
    monitoring: Arc<BasicMonitoring>,
    thresholds: PerformanceThresholds,
}

/// 性能阈值配置
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub max_error_rate: f64,       // 最大错误率（百分比）
    pub max_response_time_ms: f64, // 最大响应时间（毫秒）
    pub min_success_rate: f64,     // 最小成功率（百分比）
    pub max_memory_usage_mb: f64,  // 最大内存使用（MB）
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_error_rate: 5.0,          // 5%
            max_response_time_ms: 5000.0, // 5秒
            min_success_rate: 95.0,       // 95%
            max_memory_usage_mb: 1024.0,  // 1GB
        }
    }
}

impl PerformanceHealthChecker {
    pub fn new(
        name: String,
        monitoring: Arc<BasicMonitoring>,
        thresholds: PerformanceThresholds,
    ) -> Self {
        Self {
            name,
            monitoring,
            thresholds,
        }
    }
}

#[async_trait::async_trait]
impl HealthChecker for PerformanceHealthChecker {
    async fn check_health(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let metrics = self.monitoring.get_performance_metrics().await;
        let check_duration = start_time.elapsed();

        let mut level = HealthLevel::Healthy;
        let mut messages = Vec::new();
        let mut details = HashMap::new();

        // 检查错误率
        if metrics.error_rate > self.thresholds.max_error_rate {
            level = HealthLevel::Warning;
            messages.push(format!("错误率过高: {:.1}%", metrics.error_rate));
        }
        details.insert(
            "error_rate".to_string(),
            format!("{:.1}%", metrics.error_rate),
        );

        // 检查响应时间
        if metrics.avg_response_time_ms > self.thresholds.max_response_time_ms {
            level = HealthLevel::Warning;
            messages.push(format!(
                "响应时间过长: {:.1}ms",
                metrics.avg_response_time_ms
            ));
        }
        details.insert(
            "avg_response_time_ms".to_string(),
            format!("{:.1}", metrics.avg_response_time_ms),
        );

        // 检查成功率
        if metrics.success_rate < self.thresholds.min_success_rate {
            level = HealthLevel::Critical;
            messages.push(format!("成功率过低: {:.1}%", metrics.success_rate));
        }
        details.insert(
            "success_rate".to_string(),
            format!("{:.1}%", metrics.success_rate),
        );

        // 检查内存使用
        let memory_mb = metrics.memory_usage_bytes as f64 / 1024.0 / 1024.0;
        if memory_mb > self.thresholds.max_memory_usage_mb {
            level = HealthLevel::Warning;
            messages.push(format!("内存使用过高: {:.1}MB", memory_mb));
        }
        details.insert("memory_usage_mb".to_string(), format!("{:.1}", memory_mb));

        let message = if messages.is_empty() {
            "性能指标正常".to_string()
        } else {
            messages.join("; ")
        };

        HealthCheckResult {
            component: self.name.clone(),
            level,
            message,
            details,
            timestamp: SystemTime::now(),
            check_duration_ms: check_duration.as_millis() as u64,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 告警规则
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub cooldown: Duration,
    pub last_triggered: Option<Instant>,
}

/// 告警条件
#[derive(Debug, Clone)]
pub enum AlertCondition {
    ErrorRateAbove(f64),
    ResponseTimeAbove(f64),
    SuccessRateBelow(f64),
    ConnectionDown,
}

/// 告警严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// 告警事件
#[derive(Debug, Clone)]
pub struct AlertEvent {
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub details: HashMap<String, String>,
    pub timestamp: SystemTime,
}

/// 告警管理器
#[derive(Debug)]
pub struct AlertManager {
    rules: Arc<RwLock<Vec<AlertRule>>>,
    events: Arc<RwLock<Vec<AlertEvent>>>,
    max_events: usize,
}

impl AlertManager {
    pub fn new(max_events: usize) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            max_events,
        }
    }

    /// 添加告警规则
    pub async fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
    }

    /// 检查告警条件
    pub async fn check_alerts(&self, metrics: &PerformanceMetrics) {
        let mut rules = self.rules.write().await;
        let mut events = self.events.write().await;
        let now = Instant::now();

        for rule in rules.iter_mut() {
            // 检查冷却时间
            if let Some(last_triggered) = rule.last_triggered {
                if now.duration_since(last_triggered) < rule.cooldown {
                    continue;
                }
            }

            // 检查条件
            let should_trigger = match &rule.condition {
                AlertCondition::ErrorRateAbove(threshold) => metrics.error_rate > *threshold,
                AlertCondition::ResponseTimeAbove(threshold) => {
                    metrics.avg_response_time_ms > *threshold
                }
                AlertCondition::SuccessRateBelow(threshold) => metrics.success_rate < *threshold,
                AlertCondition::ConnectionDown => metrics.active_connections == 0,
            };

            if should_trigger {
                rule.last_triggered = Some(now);

                let message = match &rule.condition {
                    AlertCondition::ErrorRateAbove(threshold) => format!(
                        "错误率 {:.1}% 超过阈值 {:.1}%",
                        metrics.error_rate, threshold
                    ),
                    AlertCondition::ResponseTimeAbove(threshold) => format!(
                        "响应时间 {:.1}ms 超过阈值 {:.1}ms",
                        metrics.avg_response_time_ms, threshold
                    ),
                    AlertCondition::SuccessRateBelow(threshold) => format!(
                        "成功率 {:.1}% 低于阈值 {:.1}%",
                        metrics.success_rate, threshold
                    ),
                    AlertCondition::ConnectionDown => "连接已断开".to_string(),
                };

                let mut details = HashMap::new();
                details.insert(
                    "error_rate".to_string(),
                    format!("{:.1}%", metrics.error_rate),
                );
                details.insert(
                    "response_time_ms".to_string(),
                    format!("{:.1}", metrics.avg_response_time_ms),
                );
                details.insert(
                    "success_rate".to_string(),
                    format!("{:.1}%", metrics.success_rate),
                );

                let event = AlertEvent {
                    rule_name: rule.name.clone(),
                    severity: rule.severity,
                    message,
                    details,
                    timestamp: SystemTime::now(),
                };

                events.push(event.clone());

                // 限制事件数量
                if events.len() > self.max_events {
                    events.remove(0);
                }

                // 记录告警
                match rule.severity {
                    AlertSeverity::Info => info!("告警: {}", event.message),
                    AlertSeverity::Warning => warn!("告警: {}", event.message),
                    AlertSeverity::Critical => error!("告警: {}", event.message),
                }
            }
        }
    }

    /// 获取最近的告警事件
    pub async fn get_recent_events(&self, limit: usize) -> Vec<AlertEvent> {
        let events = self.events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }
}

impl BasicMonitoring {
    /// 创建新的监控实例
    pub fn new(component_name: String) -> Self {
        Self {
            component_name,
            start_time: Instant::now(),
            request_stats: Arc::new(RwLock::new(RequestStats::default())),
            response_time_stats: Arc::new(RwLock::new(ResponseTimeStats::new(1000))),
            health_checkers: Arc::new(RwLock::new(Vec::new())),
            alert_manager: Arc::new(AlertManager::new(1000)),
        }
    }

    /// 记录请求
    pub async fn record_request(&self, success: bool, response_time_ms: u64) {
        {
            let mut stats = self.request_stats.write().await;
            stats.add_request(success);
        }

        {
            let mut response_stats = self.response_time_stats.write().await;
            response_stats.add_sample(response_time_ms);
        }
    }

    /// 添加健康检查器
    pub async fn add_health_checker(&self, checker: Box<dyn HealthChecker>) {
        let mut checkers = self.health_checkers.write().await;
        checkers.push(checker);
    }

    /// 获取性能指标
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        let request_stats = self.request_stats.read().await;
        let response_stats = self.response_time_stats.read().await;

        PerformanceMetrics {
            request_rate: request_stats.get_request_rate(),
            success_rate: request_stats.get_success_rate(),
            avg_response_time_ms: response_stats.get_average(),
            p95_response_time_ms: response_stats.get_percentile(95.0),
            p99_response_time_ms: response_stats.get_percentile(99.0),
            error_rate: request_stats.get_error_rate(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections: 1,  // 简化实现
            memory_usage_bytes: 0,  // 需要具体实现
            cpu_usage_percent: 0.0, // 需要具体实现
        }
    }

    /// 执行健康检查
    pub async fn health_check(&self) -> Vec<HealthCheckResult> {
        let checkers = self.health_checkers.read().await;
        let mut results = Vec::new();

        for checker in checkers.iter() {
            let result = checker.check_health().await;
            results.push(result);
        }

        results
    }

    /// 获取系统状态
    pub async fn get_system_status(&self) -> HashMap<String, String> {
        let mut status = HashMap::new();

        status.insert("component".to_string(), self.component_name.clone());
        status.insert(
            "uptime_seconds".to_string(),
            self.start_time.elapsed().as_secs().to_string(),
        );

        let metrics = self.get_performance_metrics().await;
        status.insert(
            "request_rate".to_string(),
            format!("{:.2}", metrics.request_rate),
        );
        status.insert(
            "success_rate".to_string(),
            format!("{:.1}%", metrics.success_rate),
        );
        status.insert(
            "avg_response_time_ms".to_string(),
            format!("{:.1}", metrics.avg_response_time_ms),
        );
        status.insert(
            "error_rate".to_string(),
            format!("{:.1}%", metrics.error_rate),
        );

        let health_results = self.health_check().await;
        let healthy_count = health_results
            .iter()
            .filter(|r| r.level == HealthLevel::Healthy)
            .count();
        status.insert(
            "health_checks_passed".to_string(),
            format!("{}/{}", healthy_count, health_results.len()),
        );

        status
    }

    /// 获取告警管理器
    pub fn alert_manager(&self) -> &Arc<AlertManager> {
        &self.alert_manager
    }

    /// 启动监控任务
    pub async fn start_monitoring_task(&self) {
        let alert_manager = self.alert_manager.clone();
        let request_stats = self.request_stats.clone();
        let response_stats = self.response_time_stats.clone();
        let start_time = self.start_time;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                // 计算当前指标
                let metrics = {
                    let req_stats = request_stats.read().await;
                    let resp_stats = response_stats.read().await;

                    PerformanceMetrics {
                        request_rate: req_stats.get_request_rate(),
                        success_rate: req_stats.get_success_rate(),
                        avg_response_time_ms: resp_stats.get_average(),
                        p95_response_time_ms: resp_stats.get_percentile(95.0),
                        p99_response_time_ms: resp_stats.get_percentile(99.0),
                        error_rate: req_stats.get_error_rate(),
                        uptime_seconds: start_time.elapsed().as_secs(),
                        active_connections: 1,
                        memory_usage_bytes: 0,
                        cpu_usage_percent: 0.0,
                    }
                };

                // 检查告警
                alert_manager.check_alerts(&metrics).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_monitoring() {
        let monitoring = BasicMonitoring::new("test_component".to_string());

        // 记录一些请求
        monitoring.record_request(true, 100).await;
        monitoring.record_request(true, 200).await;
        monitoring.record_request(false, 500).await;

        let metrics = monitoring.get_performance_metrics().await;
        assert_eq!(metrics.success_rate, 66.66666666666667); // 2/3 * 100
        assert_eq!(metrics.error_rate, 33.333333333333336); // 1/3 * 100
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = ConnectionHealthChecker::new("test_connection".to_string(), || true);

        let result = checker.check_health().await;
        assert_eq!(result.level, HealthLevel::Healthy);
        assert_eq!(result.component, "test_connection");
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let alert_manager = AlertManager::new(100);

        let rule = AlertRule {
            name: "high_error_rate".to_string(),
            condition: AlertCondition::ErrorRateAbove(10.0),
            severity: AlertSeverity::Warning,
            cooldown: Duration::from_secs(60),
            last_triggered: None,
        };

        alert_manager.add_rule(rule).await;

        let metrics = PerformanceMetrics {
            error_rate: 15.0, // 超过阈值
            ..Default::default()
        };

        alert_manager.check_alerts(&metrics).await;

        let events = alert_manager.get_recent_events(10).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].rule_name, "high_error_rate");
    }
}
