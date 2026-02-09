//! Rule Scheduler - Periodic rule execution scheduler
//!
//! Manages rule execution based on trigger configurations:
//! - Interval: Execute rules at fixed intervals
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
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use voltage_calc::StateStore;
use voltage_routing::RoutingCache;
use voltage_rtdb::traits::Rtdb;
use voltage_rtdb_shm::{ShmNotifier, UnifiedReader, UnifiedWriter};

/// Default scheduler tick interval (100ms)
pub const DEFAULT_TICK_MS: u64 = 100;

/// Rule trigger configuration
///
/// Supports JSON deserialization for database storage:
/// - `{"type": "interval", "interval_ms": 1000}`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    /// Execute rule at fixed intervals
    Interval {
        /// Interval in milliseconds
        interval_ms: u64,
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
    /// Rule wrapped in Arc to avoid cloning during execution
    rule: Arc<Rule>,
    trigger: TriggerConfig,
    last_execution: Option<Instant>,
    /// Track last cooldown trigger time
    last_cooldown_start: Option<Instant>,
}

/// Rule Scheduler - manages periodic rule execution
///
/// Generic over `S: StateStore` for stateful function persistence:
/// - `MemoryStateStore` (default): In-memory, lost on restart
/// - `RtdbStateStore`: Redis-backed, persistent across restarts
pub struct RuleScheduler<R: Rtdb, S: StateStore = voltage_calc::MemoryStateStore> {
    /// RTDB instance for reading/writing data
    rtdb: Arc<R>,
    /// Rule executor instance with configurable state store
    executor: Arc<RuleExecutor<R, S>>,
    /// SQLite pool for rule persistence
    pool: SqlitePool,
    /// Cached rules with their trigger configs
    rules: Arc<RwLock<Vec<ScheduledRule>>>,
    /// Shutdown token (unified stop signal + running state)
    shutdown: CancellationToken,
    /// Scheduler tick interval in milliseconds
    tick_ms: u64,
    /// Rule logger manager for independent rule log files
    logger_manager: RuleLoggerManager,
}

impl<R: Rtdb + 'static> RuleScheduler<R, voltage_calc::MemoryStateStore> {
    /// Create a new rule scheduler with configurable tick interval (uses MemoryStateStore)
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
            shutdown: CancellationToken::new(),
            tick_ms,
            logger_manager: RuleLoggerManager::new(log_root),
        }
    }

    /// Create with UnifiedReader for two-tier priority reads (uses MemoryStateStore)
    ///
    /// Enables SharedMemory layer in the executor:
    /// 1. SharedMemory (~5μs) - cross-process mmap, highest priority
    /// 2. Redis (~1ms) - remote fallback
    ///
    /// SharedMemory is populated by comsrv and works on any filesystem.
    /// Removed VecRtdb - using SharedMemory + Redis two-tier architecture
    pub fn with_shared_reader(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
        log_root: PathBuf,
        shared_reader: Option<Arc<UnifiedReader>>,
    ) -> Self {
        Self::with_shm(
            rtdb,
            routing_cache,
            pool,
            tick_ms,
            log_root,
            shared_reader,
            None,
        )
    }

    /// Create with both UnifiedReader (for reads) and UnifiedWriter (for M2C actions)
    /// (uses MemoryStateStore - state lost on restart)
    ///
    /// Enables full SHM two-tier architecture:
    /// - Reads: SharedMemory (~5μs) > Redis (~1ms)
    /// - Writes: SHM (primary) + Redis TODO (fallback)
    pub fn with_shm(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
        log_root: PathBuf,
        shared_reader: Option<Arc<UnifiedReader>>,
        shm_action_writer: Option<Arc<UnifiedWriter>>,
    ) -> Self {
        Self::with_shm_full(
            rtdb,
            routing_cache,
            pool,
            tick_ms,
            log_root,
            shared_reader,
            shm_action_writer,
            None,
        )
    }

    /// Create with full SHM support including UDS notifier (uses MemoryStateStore)
    ///
    /// Enables complete M2C path:
    /// - SHM write (UnifiedWriter) for data
    /// - UDS notification (ShmNotifier) for immediate dispatch (~1-2ms)
    #[allow(clippy::too_many_arguments)]
    pub fn with_shm_full(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
        log_root: PathBuf,
        shared_reader: Option<Arc<UnifiedReader>>,
        shm_action_writer: Option<Arc<UnifiedWriter>>,
        shm_notifier: Option<Arc<tokio::sync::Mutex<ShmNotifier>>>,
    ) -> Self {
        let mut executor = RuleExecutor::new(Arc::clone(&rtdb), routing_cache);
        if let Some(reader) = shared_reader {
            executor = executor.with_shared_reader(reader);
        }
        if let Some(writer) = shm_action_writer {
            executor = executor.with_shm_action_writer(writer);
        }
        if let Some(notifier) = shm_notifier {
            executor = executor.with_shm_notifier(notifier);
        }
        Self {
            rtdb,
            executor: Arc::new(executor),
            pool,
            rules: Arc::new(RwLock::new(Vec::new())),
            shutdown: CancellationToken::new(),
            tick_ms,
            logger_manager: RuleLoggerManager::new(log_root),
        }
    }
}

impl<R: Rtdb + 'static, S: StateStore + 'static> RuleScheduler<R, S> {
    /// Create with custom StateStore and full SHM support
    ///
    /// Use this constructor for persistent state storage (e.g., RtdbStateStore).
    /// This ensures stateful functions like `period_delta()` retain their state
    /// across service restarts.
    ///
    /// # Example
    /// ```ignore
    /// let state_store = Arc::new(RtdbStateStore::new(rtdb.clone()));
    /// let scheduler = RuleScheduler::with_state_store(
    ///     rtdb, routing_cache, pool, tick_ms, log_root, state_store,
    ///     shared_reader, shm_action_writer, shm_notifier,
    /// );
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn with_state_store(
        rtdb: Arc<R>,
        routing_cache: Arc<RoutingCache>,
        pool: SqlitePool,
        tick_ms: u64,
        log_root: PathBuf,
        state_store: Arc<S>,
        shared_reader: Option<Arc<UnifiedReader>>,
        shm_action_writer: Option<Arc<UnifiedWriter>>,
        shm_notifier: Option<Arc<tokio::sync::Mutex<ShmNotifier>>>,
    ) -> Self {
        let mut executor =
            RuleExecutor::with_state_store(Arc::clone(&rtdb), routing_cache, state_store);
        if let Some(reader) = shared_reader {
            executor = executor.with_shared_reader(reader);
        }
        if let Some(writer) = shm_action_writer {
            executor = executor.with_shm_action_writer(writer);
        }
        if let Some(notifier) = shm_notifier {
            executor = executor.with_shm_notifier(notifier);
        }
        Self {
            rtdb,
            executor: Arc::new(executor),
            pool,
            rules: Arc::new(RwLock::new(Vec::new())),
            shutdown: CancellationToken::new(),
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
                // Parse trigger_config from database, fallback to cooldown_ms
                let trigger = rule
                    .trigger_config
                    .as_ref()
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_else(|| {
                        // Fallback: use cooldown_ms as interval (minimum 1000ms)
                        let interval_ms = if rule.cooldown_ms > 0 {
                            rule.cooldown_ms
                        } else {
                            1000
                        };
                        TriggerConfig::Interval { interval_ms }
                    });

                ScheduledRule {
                    rule: Arc::new(rule),
                    trigger,
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
        if self.shutdown.is_cancelled() {
            warn!("Scheduler already stopped");
            return;
        }

        info!("Scheduler start ({}ms)", self.tick_ms);

        let mut tick_interval = interval(Duration::from_millis(self.tick_ms));

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if let Err(e) = self.tick().await {
                        error!("Tick err: {}", e);
                    }
                }
                _ = self.shutdown.cancelled() => {
                    info!("Scheduler shutdown");
                    break;
                }
            }
        }

        info!("Scheduler stopped");
    }

    /// Stop the scheduler
    pub fn stop(&self) {
        info!("Scheduler stopping");
        self.shutdown.cancel();
    }

    /// Check if scheduler is running
    ///
    /// Returns true if the scheduler has not been stopped yet.
    /// Note: This only indicates whether stop() was called, not whether
    /// the scheduler loop has actually exited.
    pub fn is_running(&self) -> bool {
        !self.shutdown.is_cancelled()
    }

    /// Single scheduler tick - check all rules and execute if due
    ///
    /// Snapshot execution pattern for minimal lock hold time
    /// - Phase 1: Read lock to collect rules due for execution (~10μs)
    /// - Phase 2: Execute rules without holding any lock (bulk of time)
    /// - Phase 3: Write lock to update timestamps (~100μs)
    ///
    /// This reduces write lock hold time from 100ms+ to ~100μs.
    async fn tick(&self) -> Result<()> {
        let now = Instant::now();

        // Phase 1: Read lock to collect rules that need execution (fast)
        // Use Arc<Rule> to avoid cloning entire rule structure (~2KB → 8B pointer copy)
        let rules_to_execute: Vec<(usize, Arc<Rule>)> = {
            let rules = self.rules.read().await;
            rules
                .iter()
                .enumerate()
                .filter_map(|(idx, scheduled)| {
                    if !scheduled.rule.enabled {
                        return None;
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
                        Some((idx, Arc::clone(&scheduled.rule)))
                    } else {
                        None
                    }
                })
                .collect()
        }; // Read lock released here (~10μs)

        if rules_to_execute.is_empty() {
            return Ok(());
        }

        // Phase 2: Execute rules in parallel without holding any lock
        // Use buffer_unordered for concurrent execution with bounded parallelism
        use futures::stream::{self, StreamExt};

        struct ExecutionOutcome {
            idx: usize,
            rule_id: i64,
            rule_name: String,
            result: Result<RuleExecutionResult>,
        }

        // Execute rules concurrently (max 4 parallel)
        let executor = Arc::clone(&self.executor);
        let execution_futures = rules_to_execute.into_iter().map(|(idx, rule)| {
            let executor = Arc::clone(&executor);
            async move {
                debug!("Executing rule: {}", rule.id);
                let rule_id = rule.id;
                let rule_name = rule.name.clone();
                let result = executor.execute(&rule).await;
                ExecutionOutcome {
                    idx,
                    rule_id,
                    rule_name,
                    result,
                }
            }
        });

        let execution_results: Vec<ExecutionOutcome> = stream::iter(execution_futures)
            .buffer_unordered(4)
            .collect()
            .await;

        // Process results sequentially (logging and Redis writes)
        struct TimestampUpdate {
            idx: usize,
            rule_id: i64,
            start_cooldown: bool,
        }
        let mut updates: Vec<TimestampUpdate> = Vec::with_capacity(execution_results.len());

        for outcome in execution_results {
            match outcome.result {
                Ok(result) => {
                    // Log rule execution to independent rule log file
                    let logger = self
                        .logger_manager
                        .get_logger(outcome.rule_id, &outcome.rule_name);
                    logger.log_execution(&result, &result.variable_values);

                    // Write rule execution result to Redis for WebSocket monitoring
                    self.write_rule_exec_to_redis(outcome.rule_id, &result)
                        .await;

                    let start_cooldown = result.success && !result.actions_executed.is_empty();

                    if result.success {
                        debug!(
                            "Rule {} executed successfully, {} actions",
                            result.rule_id,
                            result.actions_executed.len()
                        );
                    } else {
                        warn!("Rule {} fail: {:?}", result.rule_id, result.error);
                    }

                    updates.push(TimestampUpdate {
                        idx: outcome.idx,
                        rule_id: outcome.rule_id,
                        start_cooldown,
                    });
                },
                Err(e) => {
                    error!("Rule {} err: {}", outcome.rule_id, e);
                    // Still update last_execution to prevent retry spam
                    updates.push(TimestampUpdate {
                        idx: outcome.idx,
                        rule_id: outcome.rule_id,
                        start_cooldown: false,
                    });
                },
            }
        }

        // Phase 3: Write lock to update timestamps (fast)
        if !updates.is_empty() {
            let mut rules = self.rules.write().await;
            for update in updates {
                if let Some(scheduled) = rules.get_mut(update.idx) {
                    // Verify rule ID matches (safety check against concurrent modifications)
                    if scheduled.rule.id == update.rule_id {
                        scheduled.last_execution = Some(now);
                        if update.start_cooldown {
                            scheduled.last_cooldown_start = Some(now);
                        }
                    }
                }
            }
        } // Write lock released here (~100μs)

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
    ///
    /// Note: Results are persisted to Redis via `write_rule_exec_to_redis()`.
    /// In-memory caching is not implemented - read from Redis if needed.
    pub async fn get_last_results(&self, _rule_id: i64) -> Option<RuleExecutionResult> {
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
            .expect("System time should be after UNIX epoch")
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
    use crate::types::{RuleFlow, RuleNode, RuleWires};
    use std::collections::HashMap;

    #[test]
    fn test_trigger_config_default() {
        let config = TriggerConfig::default();
        let TriggerConfig::Interval { interval_ms } = config;
        assert_eq!(interval_ms, 1000);
    }

    /// Helper to create a minimal test rule
    fn create_test_rule(id: i64, name: &str, cooldown_ms: u64) -> Rule {
        let mut nodes = HashMap::new();
        nodes.insert(
            "start".to_string(),
            RuleNode::Start {
                wires: RuleWires {
                    default: vec!["end".to_string()],
                },
            },
        );
        nodes.insert("end".to_string(), RuleNode::End);

        Rule {
            id,
            name: name.to_string(),
            description: None,
            enabled: true,
            priority: 100,
            cooldown_ms,
            trigger_config: None,
            flow: RuleFlow {
                start_node: "start".to_string(),
                nodes,
            },
        }
    }

    #[test]
    fn test_scheduled_rule_uses_arc() {
        // Create a rule and wrap it in Arc
        let rule = create_test_rule(1, "Test Rule", 1000);
        let arc_rule = Arc::new(rule);

        // Create ScheduledRule with Arc<Rule>
        let scheduled = ScheduledRule {
            rule: Arc::clone(&arc_rule),
            trigger: TriggerConfig::default(),
            last_execution: None,
            last_cooldown_start: None,
        };

        // Verify Arc works correctly
        assert_eq!(scheduled.rule.id, 1);
        assert_eq!(scheduled.rule.name, "Test Rule");
        assert_eq!(scheduled.rule.cooldown_ms, 1000);

        // Verify Arc::clone is cheap (same underlying data)
        let cloned_arc = Arc::clone(&scheduled.rule);
        assert!(Arc::ptr_eq(&scheduled.rule, &cloned_arc));
    }

    #[test]
    fn test_arc_clone_is_pointer_copy() {
        let rule = create_test_rule(42, "Arc Test", 5000);
        let arc1 = Arc::new(rule);

        // Clone multiple times
        let arc2 = Arc::clone(&arc1);
        let arc3 = Arc::clone(&arc1);
        let arc4 = Arc::clone(&arc2);

        // All point to the same data
        assert!(Arc::ptr_eq(&arc1, &arc2));
        assert!(Arc::ptr_eq(&arc2, &arc3));
        assert!(Arc::ptr_eq(&arc3, &arc4));

        // Strong count should be 4
        assert_eq!(Arc::strong_count(&arc1), 4);

        // Data is shared, not copied
        assert_eq!(arc1.id, arc4.id);
        assert_eq!(arc1.name, arc4.name);
    }

    #[test]
    fn test_scheduled_rule_trigger_interval() {
        let rule = Arc::new(create_test_rule(1, "Interval Test", 0));

        let scheduled = ScheduledRule {
            rule,
            trigger: TriggerConfig::Interval { interval_ms: 500 },
            last_execution: None,
            last_cooldown_start: None,
        };

        // Verify trigger config
        match scheduled.trigger {
            TriggerConfig::Interval { interval_ms } => {
                assert_eq!(interval_ms, 500);
            },
        }
    }

    #[test]
    fn test_multiple_scheduled_rules_share_nothing() {
        // Create two independent rules
        let rule1 = Arc::new(create_test_rule(1, "Rule 1", 1000));
        let rule2 = Arc::new(create_test_rule(2, "Rule 2", 2000));

        let scheduled1 = ScheduledRule {
            rule: Arc::clone(&rule1),
            trigger: TriggerConfig::Interval { interval_ms: 100 },
            last_execution: None,
            last_cooldown_start: None,
        };

        let scheduled2 = ScheduledRule {
            rule: Arc::clone(&rule2),
            trigger: TriggerConfig::Interval { interval_ms: 200 },
            last_execution: None,
            last_cooldown_start: None,
        };

        // Verify they are independent
        assert!(!Arc::ptr_eq(&scheduled1.rule, &scheduled2.rule));
        assert_eq!(scheduled1.rule.id, 1);
        assert_eq!(scheduled2.rule.id, 2);
    }

    #[tokio::test]
    async fn test_parallel_execution_collects_all_results() {
        use futures::stream::{self, StreamExt};

        // Simulate parallel execution pattern used in tick()
        let items = vec![(0, 10), (1, 20), (2, 30), (3, 40)];

        let results: Vec<(usize, i32)> =
            stream::iter(items.into_iter().map(|(idx, val)| async move {
                // Simulate async work
                tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;
                (idx, val * 2)
            }))
            .buffer_unordered(4)
            .collect()
            .await;

        // All results should be collected (order may vary due to unordered)
        assert_eq!(results.len(), 4);

        // Verify all values are processed correctly
        let sum: i32 = results.iter().map(|(_, v)| v).sum();
        assert_eq!(sum, 200); // (10+20+30+40) * 2 = 200
    }
}
