//! # Simplified Alarm Service
//!
//! A streamlined alarm service that uses Redis Functions directly for all operations.
//! This replaces the complex multi-layer abstraction with simple, direct Redis operations.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use voltage_libs::redis::RedisClient;

/// Alarm level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmLevel {
    Critical,
    Major,
    Minor,
    Warning,
    Info,
}

/// Alarm status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmStatus {
    New,
    Acknowledged,
    Resolved,
}

/// Simple alarm structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub level: AlarmLevel,
    pub status: AlarmStatus,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub priority: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

impl Alarm {
    /// Create new alarm
    pub fn new(title: String, description: String, level: AlarmLevel) -> Self {
        let now = Utc::now();
        let priority = Self::level_to_priority(level);

        Self {
            id: Uuid::new_v4(),
            title,
            description,
            level,
            status: AlarmStatus::New,
            source: None,
            tags: Vec::new(),
            priority,
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            resolved_by: None,
        }
    }

    /// Create alarm with source
    pub fn with_source(
        title: String,
        description: String,
        level: AlarmLevel,
        source: String,
    ) -> Self {
        let mut alarm = Self::new(title, description, level);
        alarm.source = Some(source);
        alarm
    }

    /// Convert alarm level to priority score
    fn level_to_priority(level: AlarmLevel) -> u32 {
        match level {
            AlarmLevel::Critical => 90,
            AlarmLevel::Major => 70,
            AlarmLevel::Minor => 50,
            AlarmLevel::Warning => 30,
            AlarmLevel::Info => 10,
        }
    }

    /// Check if alarm is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, AlarmStatus::New | AlarmStatus::Acknowledged)
    }
}

/// Alarm statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmStatistics {
    pub total: usize,
    pub new: usize,
    pub acknowledged: usize,
    pub resolved: usize,
    pub critical: usize,
    pub major: usize,
    pub minor: usize,
    pub warning: usize,
    pub info: usize,
    pub today_handled: usize,
    pub active: usize,
}

/// Query parameters for alarm search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmQuery {
    pub status: Option<AlarmStatus>,
    pub level: Option<AlarmLevel>,
    pub source: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for AlarmQuery {
    fn default() -> Self {
        Self {
            status: None,
            level: None,
            source: None,
            start_time: None,
            end_time: None,
            limit: Some(100),
            offset: Some(0),
        }
    }
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmQueryResult {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub data: Vec<Alarm>,
}

/// Simplified Alarm Service using Redis Functions
pub struct AlarmService {
    redis: Arc<RwLock<RedisClient>>,
    key_prefix: String,
}

impl AlarmService {
    /// Create new alarm service
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = RedisClient::new(redis_url).await?;

        info!("Connected to Redis for AlarmService");

        Ok(Self {
            redis: Arc::new(RwLock::new(client)),
            key_prefix: "alarmsrv".to_string(),
        })
    }

    /// Store alarm using Redis Functions
    pub async fn store_alarm(&self, alarm: &Alarm) -> Result<()> {
        let alarm_json = serde_json::to_string(alarm)?;

        debug!("Storing alarm: {} with title: {}", alarm.id, alarm.title);

        // Call Redis Function to store alarm
        let result: String = {
            let mut conn = self.redis.write().await;
            conn.fcall("store_alarm", &[&alarm.id.to_string()], &[&alarm_json])
                .await?
        }; // Lock released here

        if result != "OK" {
            return Err(anyhow!("Failed to store alarm: {}", result));
        }

        info!("Alarm {} stored successfully", alarm.id);
        Ok(())
    }

    /// Get alarm by ID
    pub async fn get_alarm(&self, alarm_id: &Uuid) -> Result<Option<Alarm>> {
        let alarm_key = format!("{}:{}", self.key_prefix, alarm_id);

        debug!("Getting alarm: {}", alarm_id);

        // Get alarm data directly from Redis hash
        let alarm_data: Option<String> = {
            let mut conn = self.redis.write().await;
            conn.hget(&alarm_key, "data").await?
        }; // Lock released here

        match alarm_data {
            Some(data) => {
                let alarm: Alarm = serde_json::from_str(&data)?;
                debug!("Found alarm: {}", alarm_id);
                Ok(Some(alarm))
            },
            None => {
                debug!("Alarm not found: {}", alarm_id);
                Ok(None)
            },
        }
    }

    /// Acknowledge alarm using Redis Functions
    pub async fn acknowledge_alarm(&self, alarm_id: &Uuid, user: &str) -> Result<Alarm> {
        debug!("Acknowledging alarm: {} by user: {}", alarm_id, user);

        let now = chrono::Utc::now().to_rfc3339();

        // Call Redis Function to acknowledge alarm
        let result: String = {
            let mut conn = self.redis.write().await;
            conn.fcall("acknowledge_alarm", &[&alarm_id.to_string()], &[user, &now])
                .await?
        }; // Lock released here

        let alarm: Alarm = serde_json::from_str(&result)?;
        info!("Alarm {} acknowledged by {}", alarm_id, user);
        Ok(alarm)
    }

    /// Resolve alarm using Redis Functions
    pub async fn resolve_alarm(&self, alarm_id: &Uuid, user: &str) -> Result<Alarm> {
        debug!("Resolving alarm: {} by user: {}", alarm_id, user);

        let now = chrono::Utc::now().to_rfc3339();

        // Call Redis Function to resolve alarm
        let result: String = {
            let mut conn = self.redis.write().await;
            conn.fcall("resolve_alarm", &[&alarm_id.to_string()], &[user, &now])
                .await?
        }; // Lock released here

        let alarm: Alarm = serde_json::from_str(&result)?;
        info!("Alarm {} resolved by {}", alarm_id, user);
        Ok(alarm)
    }

    /// Query alarms using Redis Functions
    pub async fn query_alarms(&self, query: &AlarmQuery) -> Result<AlarmQueryResult> {
        let mut conn = self.redis.write().await;

        // Convert query to JSON format for Redis Function
        let query_config = serde_json::json!({
            "status": query.status.map(|s| format!("{:?}", s)),
            "level": query.level.map(|l| format!("{:?}", l)),
            "source": query.source,
            "start_time": query.start_time.map(|t| t.to_rfc3339()),
            "end_time": query.end_time.map(|t| t.to_rfc3339()),
            "limit": query.limit.unwrap_or(100),
            "offset": query.offset.unwrap_or(0),
        });

        debug!("Querying alarms with config: {}", query_config);

        // Call Redis Function to query alarms
        let result: String = conn
            .fcall("query_alarms", &[], &[&query_config.to_string()])
            .await?;

        let query_result: AlarmQueryResult = serde_json::from_str(&result)?;
        debug!(
            "Query returned {} alarms out of {} total",
            query_result.data.len(),
            query_result.total
        );
        Ok(query_result)
    }

    /// Get alarm statistics using Redis Functions
    pub async fn get_statistics(&self) -> Result<AlarmStatistics> {
        debug!("Getting alarm statistics");

        // Call Redis Function to get statistics
        let result: String = {
            let mut conn = self.redis.write().await;
            conn.fcall("get_alarm_stats", &[], &["summary"]).await?
        }; // Lock released here

        // Parse the statistics result
        let stats_data: serde_json::Value = serde_json::from_str(&result)?;

        let stats = AlarmStatistics {
            total: stats_data["total"].as_u64().unwrap_or(0) as usize,
            new: stats_data["new"].as_u64().unwrap_or(0) as usize,
            acknowledged: stats_data["acknowledged"].as_u64().unwrap_or(0) as usize,
            resolved: stats_data["resolved"].as_u64().unwrap_or(0) as usize,
            critical: stats_data["critical"].as_u64().unwrap_or(0) as usize,
            major: stats_data["major"].as_u64().unwrap_or(0) as usize,
            minor: stats_data["minor"].as_u64().unwrap_or(0) as usize,
            warning: stats_data["warning"].as_u64().unwrap_or(0) as usize,
            info: stats_data["info"].as_u64().unwrap_or(0) as usize,
            today_handled: 0, // This would need additional logic in Redis Function
            active: stats_data["new"].as_u64().unwrap_or(0) as usize
                + stats_data["acknowledged"].as_u64().unwrap_or(0) as usize,
        };

        debug!("Statistics: total={}, active={}", stats.total, stats.active);
        Ok(stats)
    }

    /// Create alarm from threshold check
    pub async fn check_threshold(
        &self,
        channel: &str,
        conditions: &serde_json::Value,
        values: &serde_json::Value,
    ) -> Result<Option<Alarm>> {
        debug!("Checking threshold for channel: {}", channel);

        // Call Redis Function to check threshold
        let result: String = {
            let mut conn = self.redis.write().await;
            conn.fcall(
                "check_alarm_threshold",
                &[channel],
                &[&conditions.to_string(), &values.to_string()],
            )
            .await?
        }; // Lock released here

        if result == "no_alarm" {
            debug!("No threshold exceeded for channel: {}", channel);
            return Ok(None);
        }

        if result == "OK" {
            info!("Threshold alarm created for channel: {}", channel);
            // The Redis Function already stored the alarm, we could fetch it back if needed
            // For now, we'll return None as the alarm is stored but not returned directly
            return Ok(None);
        }

        // If result is not "no_alarm" or "OK", it might be an error
        warn!("Unexpected threshold check result: {}", result);
        Ok(None)
    }

    /// Subscribe to alarm events
    pub async fn subscribe_to_events(&self, redis_url: &str) -> Result<redis::aio::PubSub> {
        let mut client = RedisClient::new(redis_url).await?;
        let pubsub = client.subscribe(&["alarmsrv:events"]).await?;
        info!("Subscribed to alarm events");
        Ok(pubsub)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.redis.write().await;
        let _pong = conn.ping().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alarm_creation() {
        let alarm = Alarm::new(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Warning,
        );

        assert_eq!(alarm.title, "Test Alarm");
        assert_eq!(alarm.level, AlarmLevel::Warning);
        assert_eq!(alarm.status, AlarmStatus::New);
        assert!(alarm.is_active());
        assert_eq!(alarm.priority, 30);
    }

    #[test]
    fn test_alarm_with_source() {
        let alarm = Alarm::with_source(
            "Test Alarm".to_string(),
            "This is a test alarm".to_string(),
            AlarmLevel::Critical,
            "test_source".to_string(),
        );

        assert_eq!(alarm.source, Some("test_source".to_string()));
        assert_eq!(alarm.priority, 90);
    }

    #[test]
    fn test_default_query() {
        let query = AlarmQuery::default();
        assert_eq!(query.limit, Some(100));
        assert_eq!(query.offset, Some(0));
    }
}
