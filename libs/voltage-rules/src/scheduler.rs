//! Rule Scheduler - Periodic rule execution scheduler
//!
//! Manages rule execution based on trigger configurations:
//! - Interval: Execute rules at fixed intervals
//! - OnChange: Execute rules when watched instance values change (TODO: future)
//!
//! Current implementation uses a simple tick-based approach with 100ms granularity.

use crate::error::Result;
use crate::executor::{RuleExecutionResult, RuleExecutor};
use crate::logger::RuleLoggerManager;
use crate::repository;
use crate::types::Rule;
use bytes::Bytes;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use voltage_rtdb::traits::Rtdb;
use voltage_rtdb::RoutingCache;

/// Default scheduler tick interval (100ms)
pub const DEFAULT_TICK_MS: u64 = 100;

/// Rule trigger configuration
#[derive(Debug, Clone)]
pub enum TriggerConfig {
    /// Execute rule at fixed intervals
    Interval {
        /// Interval in milliseconds
        interval_ms: u64,
    },
    /// Execute rule when watched instance values change (future implementation)
    #[allow(dead_code)]
    OnChange {
        /// List of instance IDs to watch
        watch_instances: Vec<u16>,
        /// Debounce time in milliseconds
        debounce_ms: u64,
    },
}

impl Default for TriggerConfig {
    fn default() -> Self {
        // Default to 1 second interval
        TriggerConfig::Interval { interval_ms: 1000 }
    }
}

/// Runtime state for a scheduled rule
struct ScheduledRule {
    rule: Rule,
    trigger: TriggerConfig,
    last_execution: Option<Instant>,
    /// Track last cooldown trigger time
    last_cooldown_start: Option<Instant>,
}

/// Rule Scheduler - manages periodic rule execution
pub struct RuleScheduler<R: Rtdb + ?Sized> {
    /// RTDB instance for reading/writing data
    rtdb: Arc<R>,
    /// Rule executor instance
    executor: Arc<RuleExecutor<R>>,
    /// SQLite pool for rule persistence
    pool: SqlitePool,
    /// Cached rules with their trigger configs
    rules: Arc<RwLock<Vec<ScheduledRule>>>,
    /// Shutdown signal
    shutdown: Arc<tokio::sync::Notify>,
    /// Running state
    running: Arc<std::sync::atomic::AtomicBool>,
    /// Scheduler tick interval in milliseconds
    tick_ms: u64,
    /// Rule logger manager for independent rule log files
    logger_manager: RuleLoggerManager,
}

impl<R: Rtdb + ?Sized + 'static> RuleScheduler<R> {
    /// Create a new rule scheduler with configurable tick interval
    ///
    /// # Arguments
    /// * `rtdb` - RTDB instance for reading/writing data
    /// * `routing_cache` - Routing cache for M2C route lookups
    /// * `pool` - SQLite pool for rule persistence
    /// * `tick_ms` - Scheduler tick interval in milliseconds
    /// * `log_root` - Root directory for rule log files (e.g., "logs/modsrv")
    pub fn new(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
        log_root: PathBuf,
    ) -> Self {
        Self {
            rtdb: Arc::clone(&rtdb),
            executor: Arc::new(RuleExecutor::new(rtdb, routing_cache)),
            pool,
            rules: Arc::new(RwLock::new(Vec::new())),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tick_ms,
            logger_manager: RuleLoggerManager::new(log_root),
        }
    }

    /// Load rules from database and initialize scheduler state
    pub async fn load_rules(&self) -> Result<usize> {
        let db_rules = repository::load_enabled_rules(&self.pool).await?;
        let count = db_rules.len();

        let scheduled: Vec<ScheduledRule> = db_rules
            .into_iter()
            .map(|rule| {
                // For now, use default interval trigger
                // TODO: Read trigger config from database
                let interval_ms = if rule.cooldown_ms > 0 {
                    rule.cooldown_ms
                } else {
                    1000 // Default 1 second
                };

                ScheduledRule {
                    rule,
                    trigger: TriggerConfig::Interval { interval_ms },
                    last_execution: None,
                    last_cooldown_start: None,
                }
            })
            .collect();

        let mut rules = self.rules.write().await;
        *rules = scheduled;

        info!("Rules: {} loaded", count);
        Ok(count)
    }

    /// Reload rules from database (hot reload)
    pub async fn reload_rules(&self) -> Result<usize> {
        info!("Rules reloading");
        self.load_rules().await
    }

    /// Start the scheduler loop
    pub async fn start(&self) {
        use std::sync::atomic::Ordering;

        if self.running.load(Ordering::Relaxed) {
            warn!("Scheduler running");
            return;
        }

        self.running.store(true, Ordering::Relaxed);
        info!("Scheduler start ({}ms)", self.tick_ms);

        let mut tick_interval = interval(Duration::from_millis(self.tick_ms));

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if let Err(e) = self.tick().await {
                        error!("Tick err: {}", e);
                    }
                }
                _ = self.shutdown.notified() => {
                    info!("Scheduler shutdown");
                    break;
                }
            }
        }

        self.running.store(false, Ordering::Relaxed);
        info!("Scheduler stopped");
    }

    /// Stop the scheduler
    pub fn stop(&self) {
        info!("Scheduler stopping");
        self.shutdown.notify_one();
    }

    /// Check if scheduler is running
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Single scheduler tick - check all rules and execute if due
    async fn tick(&self) -> Result<()> {
        let now = Instant::now();
        let mut rules = self.rules.write().await;

        for scheduled in rules.iter_mut() {
            if !scheduled.rule.enabled {
                continue;
            }

            let should_execute = match &scheduled.trigger {
                TriggerConfig::Interval { interval_ms } => {
                    match scheduled.last_execution {
                        None => true, // First execution
                        Some(last) => {
                            let elapsed = now.duration_since(last).as_millis() as u64;
                            elapsed >= *interval_ms
                        },
                    }
                },
                TriggerConfig::OnChange { .. } => {
                    // TODO: Implement change detection
                    false
                },
            };

            // Check cooldown
            let cooldown_ok = if scheduled.rule.cooldown_ms > 0 {
                match scheduled.last_cooldown_start {
                    None => true,
                    Some(start) => {
                        let elapsed = now.duration_since(start).as_millis() as u64;
                        elapsed >= scheduled.rule.cooldown_ms
                    },
                }
            } else {
                true
            };

            if should_execute && cooldown_ok {
                debug!("Executing rule: {}", scheduled.rule.id);

                match self.executor.execute(&scheduled.rule).await {
                    Ok(result) => {
                        // Log rule execution to independent rule log file
                        let logger = self
                            .logger_manager
                            .get_logger(scheduled.rule.id, &scheduled.rule.name);
                        logger.log_execution(&result, &result.variable_values);

                        // Write rule execution result to Redis for WebSocket monitoring
                        // Note: node_details already contains input_values for each node
                        self.write_rule_exec_to_redis(scheduled.rule.id, &result)
                            .await;

                        scheduled.last_execution = Some(now);
                        if result.success {
                            debug!(
                                "Rule {} executed successfully, {} actions",
                                result.rule_id,
                                result.actions_executed.len()
                            );
                            // Start cooldown on successful execution with actions
                            if !result.actions_executed.is_empty() {
                                scheduled.last_cooldown_start = Some(now);
                            }
                        } else {
                            warn!("Rule {} fail: {:?}", result.rule_id, result.error);
                        }
                    },
                    Err(e) => {
                        error!("Rule {} err: {}", scheduled.rule.id, e);
                        scheduled.last_execution = Some(now);
                    },
                }
            }
        }

        Ok(())
    }

    /// Get current rules count
    pub async fn rules_count(&self) -> usize {
        self.rules.read().await.len()
    }

    /// Get scheduler status
    pub async fn status(&self) -> SchedulerStatus {
        let rules = self.rules.read().await;
        let enabled_count = rules.iter().filter(|r| r.rule.enabled).count();

        SchedulerStatus {
            running: self.is_running(),
            total_rules: rules.len(),
            enabled_rules: enabled_count,
            tick_interval_ms: DEFAULT_TICK_MS,
        }
    }

    /// Execute a specific rule by ID (manual trigger)
    pub async fn execute_rule(&self, rule_id: i64) -> Result<RuleExecutionResult> {
        // Load the rule from database
        let rule = repository::get_rule_for_execution(&self.pool, rule_id).await?;

        // Execute it
        self.executor.execute(&rule).await
    }

    /// Get execution results for a rule (if cached)
    /// Note: This is a placeholder for future implementation
    pub async fn get_last_results(&self, _rule_id: i64) -> Option<RuleExecutionResult> {
        // TODO: Implement result caching if needed
        None
    }

    /// Write rule execution result to Redis
    ///
    /// Stores result in `rule:{rule_id}:exec` Hash with fields:
    /// - `timestamp` → execution timestamp
    /// - `success` → "true" or "false"
    /// - `execution_path` → JSON array of node IDs
    /// - `variable_values` → JSON object of variable values
    /// - `node_details` → JSON object of node execution details
    /// - `error` → error message if any
    async fn write_rule_exec_to_redis(&self, rule_id: i64, result: &RuleExecutionResult) {
        let exec_key = format!("rule:{}:exec", rule_id);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Write timestamp
        let _ = self
            .rtdb
            .hash_set(&exec_key, "timestamp", Bytes::from(ts.to_string()))
            .await;

        // Write success flag
        let _ = self
            .rtdb
            .hash_set(
                &exec_key,
                "success",
                Bytes::from(result.success.to_string()),
            )
            .await;

        // Write execution path as JSON
        if let Ok(path_json) = serde_json::to_string(&result.execution_path) {
            let _ = self
                .rtdb
                .hash_set(&exec_key, "execution_path", Bytes::from(path_json))
                .await;
        }

        // Write variable values as JSON
        if let Ok(vars_json) = serde_json::to_string(&result.variable_values) {
            let _ = self
                .rtdb
                .hash_set(&exec_key, "variable_values", Bytes::from(vars_json))
                .await;
        }

        // Write node details as JSON
        if let Ok(details_json) = serde_json::to_string(&result.node_details) {
            let _ = self
                .rtdb
                .hash_set(&exec_key, "node_details", Bytes::from(details_json))
                .await;
        }

        // Write error if present
        let error_str = result.error.clone().unwrap_or_default();
        let _ = self
            .rtdb
            .hash_set(&exec_key, "error", Bytes::from(error_str))
            .await;

        debug!("Written rule execution result to Redis: {}", rule_id);
    }
}

/// Scheduler status information
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    pub running: bool,
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub tick_interval_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_config_default() {
        let config = TriggerConfig::default();
        match config {
            TriggerConfig::Interval { interval_ms } => {
                assert_eq!(interval_ms, 1000);
            },
            _ => panic!("Expected Interval trigger"),
        }
    }
}
