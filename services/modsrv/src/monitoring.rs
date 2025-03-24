use crate::error::{Result, ModelSrvError};
use crate::storage::hybrid_store::HybridStore;
use crate::storage::DataStore;

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use serde::{Serialize, Deserialize};
use log::{info, error, warn, debug};
use serde_json::Value;
use redis::Commands;
use crate::redis_handler::RedisConnection;

/// Rule execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetrics {
    /// Total number of rule executions
    pub total_executions: u64,
    /// Number of successful executions
    pub successful_executions: u64,
    /// Number of failed executions
    pub failed_executions: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: u64,
    /// Minimum execution time in milliseconds
    pub min_execution_time_ms: u64,
    /// Last execution timestamp
    pub last_execution: u64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
}

impl Default for RuleMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            total_execution_time_ms: 0,
            avg_execution_time_ms: 0.0,
            max_execution_time_ms: 0,
            min_execution_time_ms: u64::MAX,
            last_execution: 0,
            success_rate: 0.0,
        }
    }
}

/// Rule execution history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionEntry {
    /// Rule ID
    pub rule_id: String,
    /// Execution timestamp
    pub timestamp: u64,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Success status
    pub success: bool,
    /// Context data (input)
    pub context: Option<Value>,
    /// Result (output)
    pub result: Option<Value>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Health status of the rule engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// System is healthy
    Healthy,
    /// System is degraded but operational
    Degraded,
    /// System is unhealthy
    Unhealthy,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Healthy
    }
}

/// Health check information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    /// Overall health status
    pub status: HealthStatus,
    /// System uptime in seconds
    pub uptime: u64,
    /// Current memory usage in bytes
    pub memory_usage: u64,
    /// Number of rules loaded
    pub rules_count: usize,
    /// Redis connection status
    pub redis_connected: bool,
    /// Health check timestamp
    pub timestamp: u64,
    /// Detailed health checks
    pub checks: HashMap<String, HealthCheckResult>,
}

/// Result of a specific health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Check name
    pub name: String,
    /// Check status
    pub status: HealthStatus,
    /// Additional details
    pub details: Option<String>,
    /// Last success timestamp
    pub last_success: Option<u64>,
}

/// Monitoring service for rule execution and system health
pub struct MonitoringService {
    /// Data store for persisting metrics and history
    store: Arc<HybridStore>,
    /// In-memory metrics for each rule
    metrics: Arc<RwLock<HashMap<String, RuleMetrics>>>,
    /// History of rule executions (limited to most recent)
    history: Arc<Mutex<Vec<RuleExecutionEntry>>>,
    /// History retention limit
    history_limit: usize,
    /// Health information
    health: Arc<RwLock<HealthInfo>>,
    /// Start time of the service
    start_time: SystemTime,
    /// Redis connection
    redis: Mutex<RedisConnection>,
}

impl MonitoringService {
    /// Create a new monitoring service
    pub fn new(store: Arc<HybridStore>, history_limit: usize) -> Self {
        let default_health = HealthInfo {
            status: HealthStatus::Healthy,
            uptime: 0,
            memory_usage: 0,
            rules_count: 0,
            redis_connected: false,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            checks: HashMap::new(),
        };
        
        Self {
            store,
            metrics: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(Mutex::new(Vec::with_capacity(history_limit))),
            history_limit,
            health: Arc::new(RwLock::new(default_health)),
            start_time: SystemTime::now(),
            redis: Mutex::new(RedisConnection::new()),
        }
    }
    
    /// Record metrics for a rule execution
    pub fn record_execution(&self, rule_id: &str, duration: Duration, success: bool, context: Option<Value>, result: Option<Value>, error: Option<String>) -> Result<()> {
        // Record metrics
        {
            let mut metrics_map = self.metrics.write().map_err(|_| ModelSrvError::LockError)?;
            let metrics = metrics_map.entry(rule_id.to_string()).or_default();
            
            metrics.total_executions += 1;
            if success {
                metrics.successful_executions += 1;
            } else {
                metrics.failed_executions += 1;
            }
            
            let duration_ms = duration.as_millis() as u64;
            metrics.total_execution_time_ms += duration_ms;
            metrics.avg_execution_time_ms = metrics.total_execution_time_ms as f64 / metrics.total_executions as f64;
            metrics.max_execution_time_ms = metrics.max_execution_time_ms.max(duration_ms);
            metrics.min_execution_time_ms = metrics.min_execution_time_ms.min(duration_ms);
            metrics.last_execution = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            metrics.success_rate = metrics.successful_executions as f64 / metrics.total_executions as f64;
            
            // Persist metrics to Redis
            if let Ok(metrics_json) = serde_json::to_string(metrics) {
                if let Err(e) = self.store.set_string(&format!("metrics:rule:{}", rule_id), &metrics_json) {
                    error!("Failed to persist metrics for rule {}: {}", rule_id, e);
                }
            }
        }
        
        // Record execution history
        {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let entry = RuleExecutionEntry {
                rule_id: rule_id.to_string(),
                timestamp,
                duration_ms: duration.as_millis() as u64,
                success,
                context,
                result,
                error,
            };
            
            let mut history = self.history.lock().map_err(|_| ModelSrvError::LockError)?;
            
            // Add new entry and ensure history limit is maintained
            history.push(entry.clone());
            if history.len() > self.history_limit {
                history.remove(0);
            }
            
            // Persist history entry to Redis
            if let Ok(entry_json) = serde_json::to_string(&entry) {
                let key = format!("history:rule:{}:{}", rule_id, timestamp);
                if let Err(e) = self.store.set_string(&key, &entry_json) {
                    error!("Failed to persist history entry for rule {}: {}", rule_id, e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get metrics for a specific rule
    pub fn get_rule_metrics(&self, rule_id: &str) -> Result<Option<RuleMetrics>> {
        let metrics_map = self.metrics.read().map_err(|_| ModelSrvError::LockError)?;
        Ok(metrics_map.get(rule_id).cloned())
    }
    
    /// Get metrics for all rules
    pub fn get_all_metrics(&self) -> Result<HashMap<String, RuleMetrics>> {
        let metrics_map = self.metrics.read().map_err(|_| ModelSrvError::LockError)?;
        Ok(metrics_map.clone())
    }
    
    /// Get execution history for a specific rule
    pub fn get_rule_history(&self, rule_id: &str, limit: Option<usize>) -> Result<Vec<RuleExecutionEntry>> {
        let history = self.history.lock().map_err(|_| ModelSrvError::LockError)?;
        let entries: Vec<_> = history.iter()
            .filter(|entry| entry.rule_id == rule_id)
            .cloned()
            .collect();
            
        let limit = limit.unwrap_or(entries.len());
        Ok(entries.into_iter().rev().take(limit).collect())
    }
    
    /// Run a health check and update health status
    pub async fn run_health_check(&self) -> Result<HealthInfo> {
        let mut health = self.health.write().map_err(|_| ModelSrvError::LockError)?;
        
        // Update uptime
        let uptime = SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();
        health.uptime = uptime;
        
        // Check Redis connection
        let redis_connected = if let Some(mut conn) = self.store.get_redis_connection() {
            match redis::cmd("PING").query::<String>(&mut conn) {
                Ok(response) => response == "PONG",
                Err(_) => false,
            }
        } else {
            false
        };
        health.redis_connected = redis_connected;
        
        // Count loaded rules
        if let Ok(keys) = self.store.get_keys("rule:*") {
            health.rules_count = keys.len();
        }
        
        // Perform specific health checks
        let mut checks = HashMap::new();
        
        // Check 1: Redis health
        let redis_health = HealthCheckResult {
            name: "Redis Connection".to_string(),
            status: if redis_connected { HealthStatus::Healthy } else { HealthStatus::Unhealthy },
            details: Some(if redis_connected { "Connected".to_string() } else { "Disconnected".to_string() }),
            last_success: if redis_connected { 
                Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()) 
            } else { 
                None 
            },
        };
        checks.insert("redis".to_string(), redis_health);
        
        // Check 2: Rules data health
        let rules_health = if health.rules_count > 0 {
            HealthCheckResult {
                name: "Rules Data".to_string(),
                status: HealthStatus::Healthy,
                details: Some(format!("{} rules loaded", health.rules_count)),
                last_success: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
            }
        } else {
            HealthCheckResult {
                name: "Rules Data".to_string(),
                status: HealthStatus::Degraded,
                details: Some("No rules loaded".to_string()),
                last_success: None,
            }
        };
        checks.insert("rules_data".to_string(), rules_health);
        
        // TODO: Add more health checks as needed
        
        // Update overall health status
        let any_unhealthy = checks.values().any(|check| check.status == HealthStatus::Unhealthy);
        let any_degraded = checks.values().any(|check| check.status == HealthStatus::Degraded);
        
        health.status = if any_unhealthy {
            HealthStatus::Unhealthy
        } else if any_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        health.checks = checks;
        health.timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // TODO: Implement automatic recovery actions based on health status
        if health.status == HealthStatus::Unhealthy {
            warn!("System health is UNHEALTHY - consider manual intervention");
        } else if health.status == HealthStatus::Degraded {
            warn!("System health is DEGRADED - monitoring for improvement");
        }
        
        Ok(health.clone())
    }
    
    /// Start the monitoring service background tasks
    pub fn start_background_tasks(&self) -> Result<()> {
        // Clone required data for the background task
        let health_lock = self.health.clone();
        let store = self.store.clone();
        
        // Spawn health check task
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(60));
            
            loop {
                interval_timer.tick().await;
                
                // Run health check
                let mut health = match health_lock.write() {
                    Ok(health) => health,
                    Err(e) => {
                        error!("Failed to acquire health lock: {}", e);
                        continue;
                    }
                };
                
                // Update uptime
                let uptime = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_else(|_| Duration::from_secs(0))
                    .as_secs();
                health.uptime = uptime;
                
                // Check Redis connection
                let redis_connected = if let Some(mut conn) = store.get_redis_connection() {
                    match redis::cmd("PING").query::<String>(&mut conn) {
                        Ok(response) => response == "PONG",
                        Err(_) => false,
                    }
                } else {
                    false
                };
                health.redis_connected = redis_connected;
                
                // Update health status based on checks
                if !redis_connected {
                    health.status = HealthStatus::Degraded;
                    warn!("Redis connection is down - system health degraded");
                } else {
                    health.status = HealthStatus::Healthy;
                    debug!("Health check passed - system is healthy");
                }
                
                health.timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                
                // Attempt recovery if needed
                if health.status != HealthStatus::Healthy {
                    info!("Attempting recovery actions");
                    // TODO: Implement recovery actions
                }
            }
        });
        
        Ok(())
    }
    
    /// Load metrics from persistent storage
    pub async fn load_metrics(&self) -> Result<HashMap<String, RuleMetrics>> {
        let mut metrics_map = HashMap::new();
        let mut redis = self.redis.lock().unwrap();

        let keys: Vec<String> = redis.keys("metrics:rule:*")?;
        for key in keys {
            if let Ok(metrics_data) = redis.get::<_, String>(&key) {
                if let Ok(metrics) = serde_json::from_str(&metrics_data) {
                    let rule_id = key.strip_prefix("metrics:rule:").unwrap_or(&key).to_string();
                    debug!("Loading metrics for rule {}", &rule_id);
                    metrics_map.insert(rule_id.clone(), metrics);
                    debug!("Loaded metrics for rule {}", rule_id);
                }
            }
        }
        
        Ok(metrics_map)
    }
    
    /// Load recent history from persistent storage
    pub async fn load_history(&self) -> Result<()> {
        // Get history keys from Redis
        if let Ok(keys) = self.store.get_keys("history:rule:*") {
            let mut history = self.history.lock().map_err(|_| ModelSrvError::LockError)?;
            
            // Sort keys by timestamp (newest first)
            let mut sorted_keys = keys;
            sorted_keys.sort_by(|a, b| b.cmp(a));
            
            // Take only the most recent entries up to history_limit
            for key in sorted_keys.iter().take(self.history_limit) {
                // Load history entry from Redis
                if let Ok(json) = self.store.get_string(key) {
                    if let Ok(entry) = serde_json::from_str::<RuleExecutionEntry>(&json) {
                        history.push(entry);
                    }
                }
            }
            
            // Sort history by timestamp (newest first)
            history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            
            // Ensure history doesn't exceed limit
            if history.len() > self.history_limit {
                history.truncate(self.history_limit);
            }
            
            info!("Loaded {} history entries", history.len());
        }
        
        Ok(())
    }
} 