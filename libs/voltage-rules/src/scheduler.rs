//! Rule Scheduler - Periodic rule execution scheduler
//!
//! Manages rule execution based on trigger configurations:
//! - Interval: Execute rules at fixed intervals
//! - OnChange: Execute rules when watched instance values change (TODO: future)
//!
//! Current implementation uses a simple tick-based approach with 100ms granularity.

use crate::error::Result;
use crate::executor::{RuleExecutionResult, RuleExecutor};
use crate::repository;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use voltage_config::rulesrv::Rule;
use voltage_config::RoutingCache;
use voltage_rtdb::traits::Rtdb;

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
}

impl<R: Rtdb + ?Sized + 'static> RuleScheduler<R> {
    /// Create a new rule scheduler with configurable tick interval
    pub fn new(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
    ) -> Self {
        Self {
            executor: Arc::new(RuleExecutor::new(rtdb, routing_cache)),
            pool,
            rules: Arc::new(RwLock::new(Vec::new())),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tick_ms,
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

        info!("Loaded {} rules into scheduler", count);
        Ok(count)
    }

    /// Reload rules from database (hot reload)
    pub async fn reload_rules(&self) -> Result<usize> {
        info!("Reloading rules...");
        self.load_rules().await
    }

    /// Start the scheduler loop
    pub async fn start(&self) {
        use std::sync::atomic::Ordering;

        if self.running.load(Ordering::Relaxed) {
            warn!("Scheduler already running");
            return;
        }

        self.running.store(true, Ordering::Relaxed);
        info!("Starting rule scheduler with {}ms tick", self.tick_ms);

        let mut tick_interval = interval(Duration::from_millis(self.tick_ms));

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if let Err(e) = self.tick().await {
                        error!("Scheduler tick error: {}", e);
                    }
                }
                _ = self.shutdown.notified() => {
                    info!("Scheduler received shutdown signal");
                    break;
                }
            }
        }

        self.running.store(false, Ordering::Relaxed);
        info!("Rule scheduler stopped");
    }

    /// Stop the scheduler
    pub fn stop(&self) {
        info!("Stopping rule scheduler...");
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
                            warn!(
                                "Rule {} execution failed: {:?}",
                                result.rule_id, result.error
                            );
                        }
                    },
                    Err(e) => {
                        error!("Rule {} execution error: {}", scheduled.rule.id, e);
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
    pub async fn execute_rule(&self, rule_id: &str) -> Result<RuleExecutionResult> {
        // Load the rule from database
        let rule = repository::get_rule_for_execution(&self.pool, rule_id).await?;

        // Execute it
        self.executor.execute(&rule).await
    }

    /// Get execution results for a rule (if cached)
    /// Note: This is a placeholder for future implementation
    pub async fn get_last_results(&self, _rule_id: &str) -> Option<RuleExecutionResult> {
        // TODO: Implement result caching if needed
        None
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
